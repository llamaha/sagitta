use std::collections::{HashMap, HashSet};
use anyhow::Result;
use crate::vectordb::embedding::EMBEDDING_DIM;
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

/// Configuration parameters for HNSW index
#[derive(Debug, Clone)]
pub struct HNSWConfig {
    /// Maximum number of connections per layer per element
    pub m: usize,
    /// Size of dynamic candidate list for construction
    pub ef_construction: usize,
    /// Number of layers in the index
    pub num_layers: usize,
    /// Random seed for layer assignment
    pub random_seed: u64,
}

impl Default for HNSWConfig {
    fn default() -> Self {
        Self {
            m: 16,
            ef_construction: 100,
            num_layers: 16,
            random_seed: 42,
        }
    }
}

/// Represents a node in the HNSW graph
#[derive(Debug, Clone)]
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
pub struct HNSWIndex {
    /// Configuration parameters
    config: HNSWConfig,
    /// The actual graph structure
    nodes: Vec<HNSWNode>,
    /// Entry point node indices for each layer
    entry_points: Vec<usize>,
}

impl HNSWIndex {
    pub fn new(config: HNSWConfig) -> Self {
        let num_layers = config.num_layers;
        Self {
            config,
            nodes: Vec::new(),
            entry_points: vec![0; num_layers],
        }
    }

    /// Calculate cosine distance between two vectors
    fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        1.0 - (dot_product / (norm_a * norm_b))
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
    fn search_layer(&self, query: &[f32], entry_point: usize, ef: usize, layer: usize) -> Vec<(usize, f32)> {
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
            let current = match candidates.iter()
                .min_by(|&&a, &&b| distances[&a].partial_cmp(&distances[&b]).unwrap_or(std::cmp::Ordering::Equal)) {
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
        if vector.len() != EMBEDDING_DIM {
            return Err(anyhow::anyhow!("Invalid vector dimension"));
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
                layer
            );

            // Select neighbors to connect to
            let selected = neighbors.into_iter()
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
                    &self.nodes[node_idx].vector
                );
                let best_dist = Self::cosine_distance(
                    &self.nodes[selected[0]].vector,
                    &self.nodes[node_idx].vector
                );
                if best_dist < current_dist {
                    self.entry_points[layer] = selected[0];
                    current_entry = selected[0];
                }
            }
        }

        Ok(node_idx)
    }

    /// Search for the k nearest neighbors of a query vector
    pub fn search(&mut self, query: &[f32], k: usize, ef: usize) -> Result<Vec<(usize, f32)>> {
        if query.len() != EMBEDDING_DIM {
            return Err(anyhow::anyhow!("Invalid query vector dimension"));
        }

        if self.nodes.is_empty() {
            return Ok(Vec::new());
        }

        // Find the highest layer where we have nodes
        let mut max_layer = 0;
        for (i, node) in self.nodes.iter().enumerate() {
            max_layer = max_layer.max(node.max_layer);
            self.entry_points[node.max_layer] = i;
        }

        let mut current_entry = self.entry_points[max_layer];
        let mut current_dist = Self::cosine_distance(query, &self.nodes[current_entry].vector);

        // Start from top layer and work down
        for layer in (0..=max_layer).rev() {
            let neighbors = self.search_layer(query, current_entry, ef, layer);
            if let Some((idx, dist)) = neighbors.first() {
                if *dist < current_dist {
                    current_entry = *idx;
                    current_dist = *dist;
                }
            }
        }

        // Search in the bottom layer
        let results = self.search_layer(query, current_entry, ef, 0);
        
        // Take top k results
        Ok(results.into_iter().take(k).collect())
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
}

/// Statistics for a single layer in the HNSW index
#[derive(Debug)]
pub struct LayerStats {
    /// Number of nodes in this layer
    pub nodes: usize,
    /// Average number of connections per node in this layer
    pub avg_connections: f32,
}

/// Overall statistics for the HNSW index
#[derive(Debug)]
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

    #[test]
    fn test_cosine_distance() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let dist = HNSWIndex::cosine_distance(&a, &b);
        assert!((dist - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_node_creation() {
        let vector = vec![1.0; EMBEDDING_DIM];
        let node = HNSWNode::new(vector, 3);
        assert_eq!(node.max_layer, 3);
        assert_eq!(node.connections.len(), 4);
    }

    #[test]
    fn test_insertion() {
        let config = HNSWConfig::default();
        let mut index = HNSWIndex::new(config);
        
        // Insert a few test vectors
        let v1 = vec![1.0; EMBEDDING_DIM];
        let v2 = vec![0.0; EMBEDDING_DIM];
        let v3 = vec![0.5; EMBEDDING_DIM];
        
        let idx1 = index.insert(v1).unwrap();
        let idx2 = index.insert(v2).unwrap();
        let idx3 = index.insert(v3).unwrap();
        
        assert_eq!(idx1, 0);
        assert_eq!(idx2, 1);
        assert_eq!(idx3, 2);
        
        // Check that nodes were created with correct dimensions
        assert_eq!(index.nodes.len(), 3);
        assert_eq!(index.nodes[0].vector.len(), EMBEDDING_DIM);
    }

    #[test]
    fn test_search() {
        let config = HNSWConfig::default();
        let mut index = HNSWIndex::new(config);
        
        // Insert test vectors
        let v1 = vec![1.0; EMBEDDING_DIM];
        let v2 = vec![0.0; EMBEDDING_DIM];
        let v3 = vec![0.5; EMBEDDING_DIM];
        
        index.insert(v1).unwrap();
        index.insert(v2).unwrap();
        index.insert(v3).unwrap();
        
        // Search for nearest neighbors
        let query = vec![0.8; EMBEDDING_DIM];
        let results = index.search(&query, 2, 10).unwrap();
        
        assert_eq!(results.len(), 2);
        assert!(results[0].1 <= results[1].1); // Results should be sorted by distance
    }

    #[test]
    fn test_stats() {
        let config = HNSWConfig::default();
        let mut index = HNSWIndex::new(config.clone());
        
        // Insert some test vectors
        for i in 0..5 {
            let v = vec![i as f32; EMBEDDING_DIM];
            index.insert(v).unwrap();
        }
        
        let stats = index.stats();
        assert_eq!(stats.total_nodes, 5);
        assert_eq!(stats.layers, config.num_layers);
        assert_eq!(stats.layer_stats.len(), config.num_layers);
    }
} 