use anyhow::Result;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashSet};
use std::cmp::{Ordering, Reverse};
use std::fs;
use std::path::Path;
use ndarray::{ArrayView1};

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
            dimension: 128,
            m: 16,
            ef_construction: 200,
            num_layers: 1, // Start with 1, might need adjustment based on data size
            random_seed: 42,
        }
    }
}

impl HNSWConfig {
    /// Creates a new HNSWConfig with the specified dimension and default values for other parameters.
    pub fn new(dimension: usize) -> Self {
        assert!(dimension > 0, "Dimension must be positive");
        Self {
            dimension,
            ..Self::default() // Use default values for other fields
        }
    }

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

// --- Added OrderedFloat wrapper for f32 in BinaryHeap ---
#[derive(PartialEq, PartialOrd, Debug, Copy, Clone)]
struct OrderedFloat(f32);

impl Eq for OrderedFloat {}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.partial_cmp(&other.0).unwrap_or(Ordering::Equal)
    }
}
// --- End OrderedFloat ---

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

    /// Calculate cosine distance between two vectors (range 0.0 to 2.0) using ndarray
    /// Requires vectors to have the same dimension.
    #[inline(always)]
    fn cosine_distance(a: ArrayView1<f32>, b: ArrayView1<f32>) -> f32 {
        // ndarray handles dimension check internally in dot product
        let dot_product = a.dot(&b);
        
        // Calculate L2 norm manually: sqrt(sum(x*x))
        let norm_a = a.mapv(|x| x * x).sum().sqrt();
        let norm_b = b.mapv(|x| x * x).sum().sqrt();

        // Handle zero vectors to avoid division by zero and return max distance
        if norm_a == 0.0 || norm_b == 0.0 {
            return 2.0; // Max distance for cosine
        }

        // Calculate cosine similarity
        let similarity = dot_product / (norm_a * norm_b);

        // Clamp similarity to [-1.0, 1.0] to handle potential floating point inaccuracies
        let clamped_similarity = similarity.clamp(-1.0, 1.0);

        // Convert similarity to distance: distance = 1.0 - similarity
        // Range [0.0, 2.0]
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

    /// Find the nearest neighbors in a given layer using BinaryHeap (Phase 1 Opt)
    fn search_layer(
        &self,
        query: &[f32],
        entry_point: usize,
        ef: usize, // Number of candidates to explore
        layer: usize,
    ) -> Vec<(usize, f32)> {
        if self.nodes.is_empty() { return Vec::new(); }
        if entry_point >= self.nodes.len() { return Vec::new(); }
        let query_view = ArrayView1::from(query);
        let mut candidates_heap = BinaryHeap::new();
        let mut results_heap: BinaryHeap<(OrderedFloat, usize)> = BinaryHeap::new();
        let mut visited = HashSet::new();
        let entry_node_view = ArrayView1::from(&self.nodes[entry_point].vector);
        let entry_dist = Self::cosine_distance(query_view, entry_node_view);
        candidates_heap.push(Reverse((OrderedFloat(entry_dist), entry_point)));
        visited.insert(entry_point);
        while let Some(Reverse((dist_wrapped, current))) = candidates_heap.pop() {
             let current_dist = dist_wrapped.0;
             if results_heap.len() >= ef {
                  if let Some(&(farthest_dist_wrapped, _)) = results_heap.peek() {
                      if current_dist > farthest_dist_wrapped.0 { continue; }
                  }
             }
             if results_heap.len() < ef {
                 results_heap.push((OrderedFloat(current_dist), current));
             } else if current_dist < results_heap.peek().unwrap().0.0 {
                 results_heap.pop();
                 results_heap.push((OrderedFloat(current_dist), current));
             }
             if layer >= self.nodes[current].connections.len() { continue; }
             for &neighbor in &self.nodes[current].connections[layer] {
                 if !visited.contains(&neighbor) {
                     visited.insert(neighbor);
                     let neighbor_node_view = ArrayView1::from(&self.nodes[neighbor].vector);
                     let neighbor_dist = Self::cosine_distance(query_view, neighbor_node_view);
                     let mut add_to_candidates = false;
                     if results_heap.len() < ef {
                         add_to_candidates = true;
                     } else if let Some(&(farthest_dist_wrapped, _)) = results_heap.peek() {
                         if neighbor_dist < farthest_dist_wrapped.0 { add_to_candidates = true; }
                     }
                     if add_to_candidates {
                         candidates_heap.push(Reverse((OrderedFloat(neighbor_dist), neighbor)));
                     }
                 }
             }
        }
        results_heap.into_sorted_vec().into_iter().map(|(dist_wrapped, idx)| (idx, dist_wrapped.0)).collect()
    }

    /// Insert a new vector into the index (Restored Sequential Logic)
    pub fn insert(&mut self, vector: Vec<f32>) -> Result<usize> {
        if vector.len() != self.config.dimension {
            return Err(anyhow::anyhow!(
                "Invalid vector dimension: expected {}, got {}",
                self.config.dimension,
                vector.len()
            ));
        }

        let max_layer = self.random_layer();
        let node_idx = self.nodes.len(); // Calculate index before adding node
        let node = HNSWNode::new(vector.clone(), max_layer);
        self.nodes.push(node); // Add node to graph

        // --- Handle First Node Entry Point Update ---
        if node_idx == 0 {
             for l in 0..=max_layer {
                 if l < self.entry_points.len() {
                     self.entry_points[l] = node_idx;
                 } else {
                      eprintln!("Warning: First node max_layer {} exceeds configured layers {}. Entry point not set.", max_layer, self.entry_points.len());
                 }
             }
            return Ok(node_idx); // No connections needed for the first node
        }

        // --- Find Entry Point for Insertion --- 
        let mut current_entry_idx = self.entry_points.last().copied().unwrap_or(0);
        let top_layer = self.entry_points.len() - 1;

        // 1. Search layers above max_layer to find the nearest node to start insertion search
        for layer in (max_layer + 1..=top_layer).rev() {
            // Defensive checks for entry point validity
             if current_entry_idx >= self.nodes.len() -1 { // -1 because we just pushed the new node
                 eprintln!("Warning: Global entry point {} invalid at layer {} during top-down search. Resetting.", current_entry_idx, layer);
                 current_entry_idx = 0;
             }
             if current_entry_idx >= self.nodes.len() - 1 { 
                  eprintln!("Error: Still no valid entry point at layer {}. Skipping layer.", layer);
                  continue;
             }
            
            // Use search_layer with ef=1 to greedily find the path down
            let nearest_neighbors = self.search_layer(
                &vector, // Use the vector being inserted
                current_entry_idx,
                1, // ef=1
                layer,
            );
            if let Some(&(nearest_idx, _)) = nearest_neighbors.first() {
                current_entry_idx = nearest_idx;
            } else {
                 eprintln!("Warning: search_layer returned empty at layer {} during top-down search. Keeping entry point {}.", layer, current_entry_idx);
            }
        }

        // 2. Insert connections from max_layer down to layer 0
        for layer in (0..=max_layer).rev() {
            // Defensive check for entry point validity
            if current_entry_idx >= self.nodes.len() - 1 {
                eprintln!("Warning: Entry point {} invalid at layer {} before neighborhood search. Resetting.", current_entry_idx, layer);
                current_entry_idx = 0; 
            }
            if current_entry_idx >= self.nodes.len() - 1 {
                  eprintln!("Error: Still no valid entry point at layer {}. Cannot find neighbors.", layer);
                  continue; // Skip connection phase for this layer if entry point is bad
             }

            // Find candidate neighbors using ef_construction
            let mut neighbors = self.search_layer(
                &vector,
                current_entry_idx,
                self.config.ef_construction,
                layer,
            );

            // Select M nearest neighbors
            neighbors.truncate(self.config.m); // Keep only the top M closest
            let selected_indices: Vec<usize> = neighbors.iter().map(|(idx, _)| *idx).collect();
            
            // --- Perform Connections ---
            // Ensure the new node's connections vec is large enough
             if layer >= self.nodes[node_idx].connections.len() {
                 self.nodes[node_idx].connections.resize_with(layer + 1, Vec::new);
             }

            for &neighbor_idx in &selected_indices {
                if neighbor_idx >= self.nodes.len() { // Check neighbor validity
                    eprintln!("Error: Invalid neighbor index {} found during insert at layer {}. Skipping connection.", neighbor_idx, layer);
                    continue;
                }
                
                // Ensure neighbor's connections vec is large enough
                 if layer >= self.nodes[neighbor_idx].connections.len() {
                     self.nodes[neighbor_idx].connections.resize_with(layer + 1, Vec::new);
                 }

                // Add forward connection (new node -> neighbor)
                self.nodes[node_idx].connections[layer].push(neighbor_idx);

                // Add backward connection (neighbor -> new node) with pruning
                 let max_neighbor_connections = self.config.m * 2; // M_max = 2 * M
                 if self.nodes[neighbor_idx].connections[layer].len() < max_neighbor_connections {
                    self.nodes[neighbor_idx].connections[layer].push(node_idx);
                 } else {
                     // TODO: Implement connection pruning if M_max is reached (optional)
                 }
            }
            
            // Update entry point for the next layer down (layer - 1)
            // Use the closest neighbor found in this layer as the entry point for the next search
            if let Some(&(nearest_idx, _)) = neighbors.first() {
                 current_entry_idx = nearest_idx;
            } else {
                 // Keep the same entry point if no neighbors found (should be rare)
                 eprintln!("Warning: No neighbors found at layer {} during connection phase. Keeping entry point {}.", layer, current_entry_idx);
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
        let mut current_dist = Self::cosine_distance(
            ArrayView1::from(query),
            ArrayView1::from(&self.nodes[current_entry].vector),
        );

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

    /// Returns the number of nodes (vectors) in the index.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns true if the index contains no nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// Statistics for a single layer in the HNSW index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerStats {
    /// Number of nodes in this layer
    pub nodes: usize,
    /// Average number of connections per node in this layer
    pub avg_connections: f32,
}

/// Overall statistics for the HNSW index
#[derive(Debug, Clone, Serialize, Deserialize)]
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
//    use rand::distributions::Distribution; // Added for vector generation

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

    fn generate_random_vector(dim: usize, rng: &mut StdRng) -> Vec<f32> {
        let mut v = Vec::with_capacity(dim);
        for _ in 0..dim {
            v.push(rng.gen::<f32>());
        }
        v
    }

    #[test]
    fn test_cosine_distance() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let c = vec![1.0, 0.0, 0.0];
        let d = vec![-1.0, 0.0, 0.0];
        let zero = vec![0.0, 0.0, 0.0];

        let view_a = ArrayView1::from(&a);
        let view_b = ArrayView1::from(&b);
        let view_c = ArrayView1::from(&c);
        let view_d = ArrayView1::from(&d);
        let view_zero = ArrayView1::from(&zero);

        // Orthogonal vectors (similarity 0) -> distance 1.0
        assert!((HNSWIndex::cosine_distance(view_a.clone(), view_b.clone()) - 1.0).abs() < 1e-6);
        // Identical vectors (similarity 1) -> distance 0.0
        assert!((HNSWIndex::cosine_distance(view_a.clone(), view_c.clone()) - 0.0).abs() < 1e-6);
        // Opposite vectors (similarity -1) -> distance 2.0
        assert!((HNSWIndex::cosine_distance(view_a.clone(), view_d.clone()) - 2.0).abs() < 1e-6);
         // Distance with zero vector -> distance 2.0
         assert!((HNSWIndex::cosine_distance(view_a.clone(), view_zero.clone()) - 2.0).abs() < 1e-6);
         assert!((HNSWIndex::cosine_distance(view_zero.clone(), view_b.clone()) - 2.0).abs() < 1e-6);
         assert!((HNSWIndex::cosine_distance(view_zero.clone(), view_zero.clone()) - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_node_creation() {
        let vector = vec![1.0; TEST_DIM];
        let node = HNSWNode::new(vector, 3);
        assert_eq!(node.max_layer, 3);
        assert_eq!(node.connections.len(), 4);
    }

    #[test]
    fn test_insertion() -> Result<()> {
        let config = test_config();
        let mut index = HNSWIndex::new(config);

        let v1 = vec![1.0; TEST_DIM];
        let v2 = vec![0.0; TEST_DIM];
        let v3 = vec![0.5; TEST_DIM];

        let idx1 = index.insert(v1)?;
        let idx2 = index.insert(v2)?;
        let idx3 = index.insert(v3)?;

        assert_eq!(idx1, 0);
        assert_eq!(idx2, 1);
        assert_eq!(idx3, 2);

        assert_eq!(index.nodes.len(), 3);
        assert_eq!(index.nodes[0].vector.len(), TEST_DIM);

        let wrong_dim_vec = vec![1.0; TEST_DIM + 1];
        assert!(index.insert(wrong_dim_vec).is_err());

        Ok(())
    }

    #[test]
    fn test_search() -> Result<()> {
        let config = test_config();
        let mut index = HNSWIndex::new(config);

        let v1 = vec![1.0; TEST_DIM];
        let v2 = vec![0.0; TEST_DIM];
        let v3 = vec![0.5; TEST_DIM];

        index.insert(v1)?;
        index.insert(v2)?;
        index.insert(v3)?;

        let query = vec![0.8; TEST_DIM];
        let results = index.search_parallel(&query, 2, 10)?;

        assert_eq!(results.len(), 2);
        println!("Search results: {:?}", results);

        let wrong_dim_query = vec![0.8; TEST_DIM + 1];
        assert!(index.search_parallel(&wrong_dim_query, 2, 10).is_err());

        Ok(())
    }

    #[test]
    fn test_stats() -> Result<()> {
        let config = test_config();
        let mut index = HNSWIndex::new(config.clone());

        for i in 0..5 {
            let v = vec![i as f32; TEST_DIM];
            index.insert(v)?;
        }

        let stats = index.stats();
        assert_eq!(stats.total_nodes, 5);
        assert_eq!(stats.layers, config.num_layers);
        assert_eq!(stats.layer_stats.len(), config.num_layers);
        assert_eq!(index.config.dimension, TEST_DIM);

        Ok(())
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
    fn benchmark_linear_vs_hnsw() -> Result<()> {
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

        for v in &vectors { hnsw_index.insert(v.clone())?; }

        let linear_search = |query: &[f32], k: usize| -> Vec<(usize, f32)> {
            let mut distances: Vec<(usize, f32)> = vectors
                .iter()
                .enumerate()
                .map(|(i, vector)| (i, HNSWIndex::cosine_distance(ArrayView1::from(query), ArrayView1::from(vector))))
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
            let _ = hnsw_index.search_parallel(query, k, 100).expect("HNSW search failed in benchmark");
            query_idx = (query_idx + 1) % num_queries;
        });

        println!(
            "HNSW is {:.2}x faster than linear search",
            linear_time.as_nanos() as f64 / hnsw_time.as_nanos() as f64
        );
        
        for query in &queries {
            let linear_results = linear_search(query, k);
            let hnsw_results = hnsw_index.search_parallel(query, k, 100)?;
            let mut found = 0;
            let linear_ids: HashSet<usize> = linear_results.iter().map(|(idx, _)| *idx).collect();
            for (idx, _) in hnsw_results {
                if linear_ids.contains(&idx) { found += 1; }
            }
            let recall = found as f32 / k as f32;
            println!("Recall@{}: {:.2}", k, recall);
            assert!(recall >= 0.7, "HNSW search quality is too low: {:.2}", recall);
        }

        Ok(())
    }

    #[test]
    #[ignore]
    fn benchmark_insertion() -> Result<()> {
        let config = HNSWConfig {
            dimension: 128, // More realistic dimension
            m: 16,
            ef_construction: 100, // Lower ef for faster build benchmark
            num_layers: 5, // More layers for larger dataset
            random_seed: 42,
        };
        let dim = config.dimension;
        let num_vectors = 5000; // Number of vectors to insert
        let mut rng = StdRng::seed_from_u64(config.random_seed);

        // Pre-generate vectors
        let vectors: Vec<Vec<f32>> = (0..num_vectors)
            .map(|_| generate_random_vector(dim, &mut rng))
            .collect();

        benchmark("HNSW Insertion", 1, || {
            let mut index = HNSWIndex::new(config.clone());
            for vec in vectors.iter() {
                index.insert(vec.clone()).expect("Insert failed in benchmark");
            }
            assert_eq!(index.len(), num_vectors);
        });
        Ok(())
    }
}
