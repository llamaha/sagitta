//! Central vector store trait for abstracting over different vector database implementations.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fmt;

/// Errors that can occur during vector store operations.
#[derive(Debug, Clone)]
pub enum VectorStoreError {
    /// Connection error to the vector database
    Connection(String),
    /// Collection/index does not exist
    CollectionNotFound(String),
    /// Invalid query parameters
    InvalidQuery(String),
    /// Serialization/deserialization error
    Serialization(String),
    /// General operation error
    Operation(String),
}

impl fmt::Display for VectorStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VectorStoreError::Connection(msg) => write!(f, "Connection error: {}", msg),
            VectorStoreError::CollectionNotFound(name) => write!(f, "Collection not found: {}", name),
            VectorStoreError::InvalidQuery(msg) => write!(f, "Invalid query: {}", msg),
            VectorStoreError::Serialization(msg) => write!(f, "Serialization error: {}", msg),
            VectorStoreError::Operation(msg) => write!(f, "Operation error: {}", msg),
        }
    }
}

impl Error for VectorStoreError {}

/// Result type for vector store operations.
pub type VectorStoreResult<T> = Result<T, VectorStoreError>;

/// A search result from the vector store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Unique identifier for the result
    pub id: String,
    /// Similarity score (0.0 to 1.0, higher is more similar)
    pub score: f32,
    /// Dense embedding vector
    pub vector: Vec<f32>,
    /// Associated metadata/payload
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Result of an upsert operation.
#[derive(Debug, Clone)]
pub struct UpsertResult {
    /// Number of points successfully upserted
    pub upserted_count: usize,
    /// Any errors that occurred during upsert
    pub errors: Vec<String>,
}

/// Configuration for creating a collection.
#[derive(Debug, Clone)]
pub struct CollectionConfig {
    /// Name of the collection
    pub name: String,
    /// Dimension of the dense vectors
    pub vector_dimension: usize,
    /// Whether to enable sparse vectors
    pub enable_sparse_vectors: bool,
    /// Distance metric for similarity calculation
    pub distance_metric: DistanceMetric,
}

/// Distance metrics supported by the vector store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DistanceMetric {
    /// Cosine similarity (default for most text embeddings)
    #[default]
    Cosine,
    /// Euclidean distance
    Euclidean,
    /// Dot product
    Dot,
    /// Manhattan distance
    Manhattan,
}

/// A point to be inserted into the vector store.
#[derive(Debug, Clone)]
pub struct VectorPoint {
    /// Unique identifier
    pub id: String,
    /// Dense embedding vector
    pub vector: Vec<f32>,
    /// Optional sparse vector for hybrid search
    pub sparse_vector: Option<HashMap<u32, f32>>,
    /// Associated metadata/payload
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Query parameters for vector search.
#[derive(Debug, Clone)]
pub struct SearchQuery {
    /// Collection name to search in
    pub collection: String,
    /// Query vector for similarity search
    pub vector: Vec<f32>,
    /// Optional sparse query vector for hybrid search
    pub sparse_vector: Option<HashMap<u32, f32>>,
    /// Maximum number of results to return
    pub limit: usize,
    /// Minimum similarity score threshold
    pub score_threshold: Option<f32>,
    /// Optional metadata filters
    pub filters: Option<HashMap<String, serde_json::Value>>,
}

/// Central trait for vector store operations.
/// 
/// This trait provides a unified interface for different vector database
/// implementations (Qdrant, Pinecone, Weaviate, etc.) that can be used
/// by GUI components and analytics without depending on specific implementations.
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// Check if the vector store is healthy and accessible.
    async fn health_check(&self) -> VectorStoreResult<bool>;

    /// Create a new collection with the specified configuration.
    async fn create_collection(&self, config: CollectionConfig) -> VectorStoreResult<bool>;

    /// Check if a collection exists.
    async fn collection_exists(&self, name: &str) -> VectorStoreResult<bool>;

    /// Delete a collection and all its data.
    async fn delete_collection(&self, name: &str) -> VectorStoreResult<bool>;

    /// List all available collections.
    async fn list_collections(&self) -> VectorStoreResult<Vec<String>>;

    /// Get information about a collection.
    async fn get_collection_info(&self, name: &str) -> VectorStoreResult<CollectionInfo>;

    /// Insert or update points in a collection.
    async fn upsert_points(&self, collection: &str, points: Vec<VectorPoint>) -> VectorStoreResult<UpsertResult>;

    /// Search for similar vectors in a collection.
    async fn search(&self, query: SearchQuery) -> VectorStoreResult<Vec<SearchResult>>;

    /// Delete points from a collection by their IDs.
    async fn delete_points(&self, collection: &str, point_ids: Vec<String>) -> VectorStoreResult<usize>;

    /// Count the total number of points in a collection.
    async fn count_points(&self, collection: &str) -> VectorStoreResult<usize>;

    /// Scroll through all points in a collection (for bulk operations).
    async fn scroll_points(&self, collection: &str, limit: Option<usize>, offset: Option<String>) -> VectorStoreResult<ScrollResult>;
}

/// Information about a collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionInfo {
    /// Collection name
    pub name: String,
    /// Number of points in the collection
    pub points_count: usize,
    /// Vector dimension
    pub vector_dimension: usize,
    /// Distance metric used
    pub distance_metric: DistanceMetric,
    /// Whether sparse vectors are enabled
    pub has_sparse_vectors: bool,
    /// Collection status
    pub status: CollectionStatus,
}

/// Status of a collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollectionStatus {
    /// Collection is ready for operations
    Ready,
    /// Collection is being created or updated
    Pending,
    /// Collection has an error
    Error,
}

/// Result of a scroll operation.
#[derive(Debug, Clone)]
pub struct ScrollResult {
    /// Retrieved points
    pub points: Vec<SearchResult>,
    /// Next offset for pagination (None if no more results)
    pub next_offset: Option<String>,
}


impl fmt::Display for DistanceMetric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DistanceMetric::Cosine => write!(f, "cosine"),
            DistanceMetric::Euclidean => write!(f, "euclidean"),
            DistanceMetric::Dot => write!(f, "dot"),
            DistanceMetric::Manhattan => write!(f, "manhattan"),
        }
    }
}

impl fmt::Display for CollectionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CollectionStatus::Ready => write!(f, "ready"),
            CollectionStatus::Pending => write!(f, "pending"),
            CollectionStatus::Error => write!(f, "error"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance_metric_display() {
        assert_eq!(DistanceMetric::Cosine.to_string(), "cosine");
        assert_eq!(DistanceMetric::Euclidean.to_string(), "euclidean");
        assert_eq!(DistanceMetric::Dot.to_string(), "dot");
        assert_eq!(DistanceMetric::Manhattan.to_string(), "manhattan");
    }

    #[test]
    fn test_collection_status_display() {
        assert_eq!(CollectionStatus::Ready.to_string(), "ready");
        assert_eq!(CollectionStatus::Pending.to_string(), "pending");
        assert_eq!(CollectionStatus::Error.to_string(), "error");
    }

    #[test]
    fn test_vector_store_error_display() {
        let error = VectorStoreError::Connection("timeout".to_string());
        assert_eq!(error.to_string(), "Connection error: timeout");

        let error = VectorStoreError::CollectionNotFound("test_collection".to_string());
        assert_eq!(error.to_string(), "Collection not found: test_collection");
    }

    #[test]
    fn test_search_result_serialization() {
        let result = SearchResult {
            id: "test_id".to_string(),
            score: 0.95,
            vector: vec![0.1, 0.2, 0.3],
            metadata: HashMap::new(),
        };

        let serialized = serde_json::to_string(&result).unwrap();
        let deserialized: SearchResult = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(result.id, deserialized.id);
        assert_eq!(result.score, deserialized.score);
        assert_eq!(result.vector, deserialized.vector);
    }
} 