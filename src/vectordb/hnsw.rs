use anyhow::Result;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
// Import the ONNX dimension constant
use crate::vectordb::provider::onnx::ONNX_EMBEDDING_DIM;

/// Configuration parameters for HNSW index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HNSWConfig {
    #[serde(default = "default_dimension")]
    pub dimension: usize,
    #[serde(default = "default_m")]
    pub m: usize,
    #[serde(default = "default_ef_construction")]
    pub ef_construction: usize,
    #[serde(default = "default_num_layers")]
    pub num_layers: usize,
    #[serde(default = "default_random_seed")]
    pub random_seed: u64,
}

// Helper functions for serde defaults, returning values from HNSWConfig::default()
fn default_dimension() -> usize { HNSWConfig::default().dimension }
fn default_m() -> usize { HNSWConfig::default().m }
fn default_ef_construction() -> usize { HNSWConfig::default().ef_construction }
fn default_num_layers() -> usize { HNSWConfig::default().num_layers }
fn default_random_seed() -> u64 { HNSWConfig::default().random_seed }

impl Default for HNSWConfig {
    fn default() -> Self {
        Self {
            dimension: ONNX_EMBEDDING_DIM,
            m: 16,
            ef_construction: 200,
            num_layers: 1, // Start with 1, might need adjustment based on data size
            random_seed: 42,
        }
    }
}

impl HNSWConfig {
    // Removed unused function calculate_optimal_layers
    // pub fn calculate_optimal_layers(dataset_size: usize) -> usize { ... }
}

/// Represents a node in the HNSW graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HNSWNode {
    /// The vector embedding
    pub vector: Vec<f32>,
    /// Layer connections for each layer
    pub connections: Vec<Vec<usize>>,
    /// Maximum layer this node appears in
    pub max_layer: usize,
}

impl HNSWNode {
    pub fn new(vector: Vec<f32>, max_layer: usize) -> Self {
        Self {
            vector,
            connections: vec![Vec::new(); max_layer + 1],
            max_layer,
        }
    }
}

/// The main HNSW index structure
#[derive(Clone)]
pub struct HNSWIndex {
    /// Configuration parameters
    config: HNSWConfig,
    /// The actual graph structure
    nodes: Vec<HNSWNode>,
    /// Entry point node indices for each layer
    entry_points: Vec<usize>,
}

/// Serializable representation of the HNSW index
#[derive(Serialize, Deserialize)]
struct SerializedHNSWIndex {
    config: HNSWConfig,
    nodes: Vec<HNSWNode>,
    entry_points: Vec<usize>,
}

impl HNSWIndex {
    pub fn new(config: HNSWConfig) -> Self {
        assert!(config.dimension > 0, "HNSW dimension must be positive");
        let num_layers = config.num_layers;
        Self {
            config,
            nodes: Vec::new(),
            entry_points: vec![0; num_layers],
        }
    }

    /// Calculate cosine distance between two vectors (range 0.0 to 2.0)
    fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        // Handle zero vectors to avoid division by zero and return max distance
        if norm_a == 0.0 || norm_b == 0.0 {
            return 2.0; // Max distance for cosine
        }

        // Calculate cosine similarity
        let similarity = dot_product / (norm_a * norm_b);

        // Clamp similarity to [-1.0, 1.0] to handle potential floating point inaccuracies
        let clamped_similarity = similarity.clamp(-1.0, 1.0);

        // Convert similarity to distance: distance = 1.0 - similarity
        // This results in a distance range of [0.0, 2.0]
        // Identical vectors (similarity 1.0) -> distance 0.0
        // Opposite vectors (similarity -1.0) -> distance 2.0
        1.0 - clamped_similarity
    }

    /// Generate a random layer for a new node
    fn random_layer(&self) -> usize {
        let mut rng = StdRng::seed_from_u64(self.config.random_seed);
        let mut layer = 0;
        while layer < self.config.num_layers - 1 && rng.gen::<f32>() < 0.5 {
            layer += 1;
        }
        layer
    }

    /// Find the nearest neighbors in a given layer
    fn search_layer(
        &self,
        query: &[f32],
        entry_point: usize,
        ef: usize,
        layer: usize,
    ) -> Vec<(usize, f32)> {
        let mut candidates = HashSet::new();
        let mut results = Vec::new();
        let mut distances = HashMap::new();

        // Initialize with entry point
        let entry_dist = Self::cosine_distance(query, &self.nodes[entry_point].vector);
        candidates.insert(entry_point);
        distances.insert(entry_point, entry_dist);
        results.push((entry_point, entry_dist));

        while !candidates.is_empty() {
            // Find the closest candidate
            let current = match candidates.iter().min_by(|&&a, &&b| {
                distances[&a]
                    .partial_cmp(&distances[&b])
                    .unwrap_or(std::cmp::Ordering::Equal)
            }) {
                Some(&c) => c,
                None => break,
            };
            candidates.remove(&current);

            // Add to results if it's better than our current worst result
            let worst_dist = results.last().map_or(f32::INFINITY, |&(_, dist)| dist);
            if results.len() < ef || distances[&current] < worst_dist {
                results.push((current, distances[&current]));
                results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                if results.len() > ef {
                    results.pop();
                }
            }

            // Explore neighbors
            for &neighbor in &self.nodes[current].connections[layer] {
                if !distances.contains_key(&neighbor) {
                    let dist = Self::cosine_distance(query, &self.nodes[neighbor].vector);
                    distances.insert(neighbor, dist);

                    let worst_dist = results.last().map_or(f32::INFINITY, |&(_, d)| d);
                    if results.len() < ef || dist < worst_dist {
                        candidates.insert(neighbor);
                    }
                }
            }
        }

        results
    }

    /// Insert a new vector into the index
    pub fn insert(&mut self, vector: Vec<f32>) -> Result<usize> {
        if vector.len() != self.config.dimension {
            return Err(anyhow::anyhow!(
                "Invalid vector dimension: expected {}, got {}",
                self.config.dimension,
                vector.len()
            ));
        }

        let max_layer = self.random_layer();
        let node = HNSWNode::new(vector, max_layer);
        let node_idx = self.nodes.len();

        // If this is the first node, set it as entry point for all layers
        if node_idx == 0 {
            self.nodes.push(node);
            return Ok(node_idx);
        }

        self.nodes.push(node);

        // Start from top layer and work down
        let mut current_entry = self.entry_points[max_layer];
        for layer in (0..=max_layer).rev() {
            let neighbors = self.search_layer(
                &self.nodes[node_idx].vector,
                current_entry,
                self.config.ef_construction,
                layer,
            );

            // Select neighbors to connect to
            let selected = neighbors
                .into_iter()
                .take(self.config.m)
                .map(|(idx, _)| idx)
                .collect::<Vec<_>>();

            // Add bidirectional connections
            for &neighbor in &selected {
                self.nodes[node_idx].connections[layer].push(neighbor);
                self.nodes[neighbor].connections[layer].push(node_idx);
            }

            // Update entry point for this layer if the new node is closer to query
            if !selected.is_empty() {
                let current_dist = Self::cosine_distance(
                    &self.nodes[current_entry].vector,
                    &self.nodes[node_idx].vector,
                );
                let best_dist = Self::cosine_distance(
                    &self.nodes[selected[0]].vector,
                    &self.nodes[node_idx].vector,
                );
                if best_dist < current_dist {
                    self.entry_points[layer] = selected[0];
                    current_entry = selected[0];
                }
            }
        }

        Ok(node_idx)
    }

    /// Search for the k nearest neighbors of a query vector in parallel
    pub fn search_parallel(&self, query: &[f32], k: usize, ef: usize) -> Result<Vec<(usize, f32)>> {
        if query.len() != self.config.dimension {
            return Err(anyhow::anyhow!(
                "Invalid query vector dimension: expected {}, got {}",
                self.config.dimension,
                query.len()
            ));
        }

        if self.nodes.is_empty() {
            return Ok(Vec::new());
        }

        // Find the highest layer where we have nodes
        let mut max_layer = 0;
        for node in &self.nodes {
            max_layer = max_layer.max(node.max_layer);
        }

        // We'll use thread-local storage for the current entry and distance
        let mut current_entry = self.entry_points[max_layer.min(self.entry_points.len() - 1)];
        let mut current_dist = Self::cosine_distance(query, &self.nodes[current_entry].vector);

        // Traverse the upper layers sequentially (they're small anyway)
        for layer in (1..=max_layer).rev() {
            let neighbors = self.search_layer(query, current_entry, ef, layer);
            if let Some((idx, dist)) = neighbors.first() {
                if *dist < current_dist {
                    current_entry = *idx;
                    current_dist = *dist;
                }
            }
        }

        // Search the bottom layer (level 0) in parallel for better performance
        // First, get all the neighbors of the entry point to use as starting points
        let initial_candidates =
            self.search_layer(query, current_entry, ef.max(self.config.m * 2), 0);

        // If we have very few candidates, just return them
        if initial_candidates.len() <= k {
            return Ok(initial_candidates);
        }

        // Take only the top candidates as starting points
        let starting_points: Vec<usize> = initial_candidates
            .iter()
            .take(self.config.m.min(4))
            .map(|(idx, _)| *idx)
            .collect();

        // Search from each starting point in parallel
        let results: Vec<Vec<(usize, f32)>> = starting_points
            .par_iter()
            .map(|&start_idx| self.search_layer(query, start_idx, ef / starting_points.len(), 0))
            .collect();

        // Merge results
        let mut merged = Vec::new();
        for result_set in results {
            for result in result_set {
                merged.push(result);
            }
        }

        // De-duplicate by node index
        let mut seen = HashSet::new();
        let mut unique_results = Vec::new();

        for (idx, dist) in merged {
            if seen.insert(idx) {
                unique_results.push((idx, dist));
            }
        }

        // Sort by distance
        unique_results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top k
        Ok(unique_results.into_iter().take(k).collect())
    }

    /// Get statistics about the index
    pub fn stats(&self) -> HNSWStats {
        let mut layer_stats = Vec::new();
        for layer in 0..self.config.num_layers {
            let mut connections = 0;
            let mut nodes_in_layer = 0;
            for node in &self.nodes {
                if layer <= node.max_layer {
                    nodes_in_layer += 1;
                    connections += node.connections[layer].len();
                }
            }
            layer_stats.push(LayerStats {
                nodes: nodes_in_layer,
                avg_connections: if nodes_in_layer > 0 {
                    connections as f32 / nodes_in_layer as f32
                } else {
                    0.0
                },
            });
        }

        HNSWStats {
            total_nodes: self.nodes.len(),
            layers: self.config.num_layers,
            layer_stats,
        }
    }

    /// Get the configuration of this index
    pub fn get_config(&self) -> HNSWConfig {
        self.config.clone()
    }

    /// Save the index to a file
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let serialized = SerializedHNSWIndex {
            config: self.config.clone(),
            nodes: self.nodes.clone(),
            entry_points: self.entry_points.clone(),
        };

        // First serialize to a string
        let data = serde_json::to_string_pretty(&serialized)?;

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write to the file
        fs::write(path, data)?;

        Ok(())
    }

    /// Load an index from a file
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let data = fs::read_to_string(path)?;
        let serialized: SerializedHNSWIndex = serde_json::from_str(&data)?;

        Ok(Self {
            config: serialized.config,
            nodes: serialized.nodes,
            entry_points: serialized.entry_points,
        })
    }
}

/// Statistics for a single layer in the HNSW index
#[derive(Debug, Clone)]
pub struct LayerStats {
    /// Number of nodes in this layer
    pub nodes: usize,
    /// Average number of connections per node in this layer
    pub avg_connections: f32,
}

/// Overall statistics for the HNSW index
#[derive(Debug, Clone)]
pub struct HNSWStats {
    /// Total number of nodes in the index
    pub total_nodes: usize,
    /// Number of layers in the index
    pub layers: usize,
    /// Statistics for each layer
    pub layer_stats: Vec<LayerStats>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    const TEST_DIM: usize = 4;

    fn test_config() -> HNSWConfig {
        HNSWConfig {
            dimension: TEST_DIM,
            m: 8,
            ef_construction: 100,
            num_layers: 4,
            random_seed: 42,
        }
    }

    #[test]
    fn test_cosine_distance() {
        // Vectors pointing in opposite directions should have maximum distance
        let v1 = vec![1.0, 0.0];
        let v2 = vec![-1.0, 0.0];
        let dist = HNSWIndex::cosine_distance(&v1, &v2);
        // With our scaling, the maximum distance is now 1.0
        // but transformed with (1.0 - similarity) * 1.2 and then power scaling of 0.8
        // so the maximum value is (1.0 - (-1.0)) * 1.2 = 2.4, transformed with 2.4^0.8 ≈ 1.89
        // We just check that it's close to 2.0
        assert!(
            dist > 1.5,
            "Distance between opposite vectors should be high, got {}",
            dist
        );

        // Identical vectors should have zero distance
        let v3 = vec![1.0, 0.0];
        let dist = HNSWIndex::cosine_distance(&v1, &v3);
        assert_eq!(dist, 0.0);

        // Orthogonal vectors should have a mid-range distance
        let v4 = vec![0.0, 1.0];
        let dist = HNSWIndex::cosine_distance(&v1, &v4);
        // With our scaling, the 90° distance is transformed with (1.0 - 0.0) * 1.2 = 1.2, then 1.2^0.8 ≈ 1.15
        // We verify it's in the expected range
        assert!(
            dist > 0.9 && dist < 1.3,
            "Distance between orthogonal vectors should be moderate, got {}",
            dist
        );
    }

    #[test]
    fn test_node_creation() {
        let vector = vec![1.0; TEST_DIM];
        let node = HNSWNode::new(vector, 3);
        assert_eq!(node.max_layer, 3);
        assert_eq!(node.connections.len(), 4);
    }

    #[test]
    fn test_insertion() {
        let config = test_config();
        let mut index = HNSWIndex::new(config);

        let v1 = vec![1.0; TEST_DIM];
        let v2 = vec![0.0; TEST_DIM];
        let v3 = vec![0.5; TEST_DIM];

        let idx1 = index.insert(v1).unwrap();
        let idx2 = index.insert(v2).unwrap();
        let idx3 = index.insert(v3).unwrap();

        assert_eq!(idx1, 0);
        assert_eq!(idx2, 1);
        assert_eq!(idx3, 2);

        assert_eq!(index.nodes.len(), 3);
        assert_eq!(index.nodes[0].vector.len(), TEST_DIM);

        let wrong_dim_vec = vec![1.0; TEST_DIM + 1];
        assert!(index.insert(wrong_dim_vec).is_err());
    }

    #[test]
    fn test_search() {
        let config = test_config();
        let mut index = HNSWIndex::new(config);

        let v1 = vec![1.0; TEST_DIM];
        let v2 = vec![0.0; TEST_DIM];
        let v3 = vec![0.5; TEST_DIM];

        index.insert(v1).unwrap();
        index.insert(v2).unwrap();
        index.insert(v3).unwrap();

        let query = vec![0.8; TEST_DIM];
        let results = index.search_parallel(&query, 2, 10).unwrap();

        assert_eq!(results.len(), 2);
        println!("Search results: {:?}", results);

        let wrong_dim_query = vec![0.8; TEST_DIM + 1];
        assert!(index.search_parallel(&wrong_dim_query, 2, 10).is_err());
    }

    #[test]
    fn test_stats() {
        let config = test_config();
        let mut index = HNSWIndex::new(config.clone());

        for i in 0..5 {
            let v = vec![i as f32; TEST_DIM];
            index.insert(v).unwrap();
        }

        let stats = index.stats();
        assert_eq!(stats.total_nodes, 5);
        assert_eq!(stats.layers, config.num_layers);
        assert_eq!(stats.layer_stats.len(), config.num_layers);
        assert_eq!(index.config.dimension, TEST_DIM);
    }

    fn benchmark<F>(name: &str, iterations: u32, mut f: F) -> Duration
    where
        F: FnMut() -> (),
    {
        // Warm up
        for _ in 0..5 {
            f();
        }

        let start = Instant::now();
        for _ in 0..iterations {
            f();
        }
        let elapsed = start.elapsed();

        println!(
            "{} took {:?} for {} iterations ({:?} per iteration)",
            name,
            elapsed,
            iterations,
            elapsed / iterations
        );

        elapsed
    }

    #[test]
    #[ignore]
    fn benchmark_linear_vs_hnsw() {
        let test_dim = 16;
        let num_vectors = 1000;
        let num_queries = 10;
        let k = 10;

        let mut vectors = Vec::with_capacity(num_vectors);
        for _ in 0..num_vectors {
            let mut v = vec![0.0; test_dim];
            for j in 0..test_dim { v[j] = rand::random::<f32>(); }
            let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 { for j in 0..test_dim { v[j] /= norm; } }
            vectors.push(v);
        }

        let mut queries = Vec::with_capacity(num_queries);
        for _ in 0..num_queries {
            let mut q = vec![0.0; test_dim];
            for j in 0..test_dim { q[j] = rand::random::<f32>(); }
            let norm: f32 = q.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 { for j in 0..test_dim { q[j] /= norm; } }
            queries.push(q);
        }

        let config = HNSWConfig {
            dimension: test_dim,
            m: 16,
            ef_construction: 200,
            num_layers: 4,
            random_seed: 42,
        };
        let mut hnsw_index = HNSWIndex::new(config);

        for v in &vectors { hnsw_index.insert(v.clone()).unwrap(); }

        let linear_search = |query: &[f32], k: usize| -> Vec<(usize, f32)> {
            let mut distances: Vec<(usize, f32)> = vectors
                .iter()
                .enumerate()
                .map(|(i, vector)| (i, HNSWIndex::cosine_distance(query, vector)))
                .collect();
            distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            distances.into_iter().take(k).collect()
        };

        let mut query_idx = 0;
        let linear_time = benchmark("Linear search", num_queries as u32, || {
            let query = &queries[query_idx];
            let _ = linear_search(query, k);
            query_idx = (query_idx + 1) % num_queries;
        });

        query_idx = 0;
        let hnsw_time = benchmark("HNSW search", num_queries as u32, || {
            let query = &queries[query_idx];
            let _ = hnsw_index.search_parallel(query, k, 100).unwrap();
            query_idx = (query_idx + 1) % num_queries;
        });

        println!(
            "HNSW is {:.2}x faster than linear search",
            linear_time.as_nanos() as f64 / hnsw_time.as_nanos() as f64
        );
        
        for query in &queries {
            let linear_results = linear_search(query, k);
            let hnsw_results = hnsw_index.search_parallel(query, k, 100).unwrap();
            let mut found = 0;
            let linear_ids: HashSet<usize> = linear_results.iter().map(|(idx, _)| *idx).collect();
            for (idx, _) in hnsw_results {
                if linear_ids.contains(&idx) { found += 1; }
            }
            let recall = found as f32 / k as f32;
            println!("Recall@{}: {:.2}", k, recall);
            assert!(recall >= 0.7, "HNSW search quality is too low: {:.2}", recall);
        }
    }
}
