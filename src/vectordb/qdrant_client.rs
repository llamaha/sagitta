use qdrant_client::{
    config::QdrantConfig,
    Payload,
    qdrant::{
        points_selector::PointsSelectorOneOf, CreateCollection, Distance, PointId, PointStruct,
        PointsIdsList, PointsSelector, SearchPoints, VectorParams, VectorsConfig,
    },
    Qdrant as QdrantApiClient,
    QdrantError as NativeQdrantError,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use thiserror::Error;
use tokio::runtime::Builder;

// Use the error module and types from the vectordb_core crate
// use vectordb_core::error::VectorDBError; // Removed - Unused due to alias/impl
// use vectordb_core::error::Result as CoreResult; // Removed - Unused due to alias

// Define the specific result type alias for this module if not re-exported from core
type VectordbResult<T> = std::result::Result<T, vectordb_core::error::VectorDBError>;

// Add back the internal QResult type alias
type QResult<T> = std::result::Result<T, QdrantError>;

#[derive(Error, Debug)]
pub enum QdrantError {
    #[error("Qdrant client error: {0}")]
    Client(#[from] qdrant_client::QdrantError),

    #[error("Collection '{0}' not found")]
    CollectionNotFound(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Payload conversion error: {0}")]
    PayloadConversion(#[from] serde_json::Error),

    #[error("URL parsing error: {0}")]
    UrlParse(#[from] url::ParseError),

    #[error("Unexpected error: {0}")]
    Unexpected(String),
}

impl From<QdrantError> for vectordb_core::error::VectorDBError {
    fn from(error: QdrantError) -> Self {
        vectordb_core::error::VectorDBError::DatabaseError(error.to_string())
    }
}

pub struct Qdrant {
    client: QdrantApiClient,
}

impl Qdrant {
    pub async fn new(url: &str) -> QResult<Self> {
        let client: QdrantApiClient = qdrant_client::Qdrant::from_url(url)
            .build()
            .map_err(QdrantError::Client)?;
        Ok(Qdrant { client })
    }

    pub async fn create_collection(
        &self,
        name: &str,
        vector_size: u64,
        distance: Distance,
    ) -> QResult<()> {
        let request = CreateCollection {
            collection_name: name.to_string(),
            vectors_config: Some(
                VectorsConfig {
                    config: Some(qdrant_client::qdrant::vectors_config::Config::Params(
                        VectorParams {
                            size: vector_size,
                            distance: distance.into(),
                            ..Default::default()
                        },
                    )),
                }
                .into(),
            ),
            ..Default::default()
        };
        self.client
            .create_collection(request)
            .await
            .map_err(QdrantError::Client)?;
        Ok(())
    }

    pub async fn list_collections(&self) -> QResult<Vec<String>> {
        let response = self
            .client
            .list_collections()
            .await
            .map_err(QdrantError::Client)?;
        Ok(response.collections.into_iter().map(|c| c.name).collect())
    }

    pub async fn delete_collection(&self, name: &str) -> QResult<()> {
        self.client
            .delete_collection(name)
            .await
            .map_err(QdrantError::Client)?;
        Ok(())
    }

    pub async fn upsert_points(&self, collection_name: &str, points: Vec<PointStruct>) -> QResult<()> {
        // Construct the request struct required by the underlying client
        let upsert_request = qdrant_client::qdrant::UpsertPoints {
            collection_name: collection_name.to_string(),
            wait: Some(true), // Wait for operation to complete - adjust if needed
            points,
            ordering: None,
            shard_key_selector: None,
        };

        // Call the underlying client's method
        self.client
            .upsert_points(upsert_request) // Pass the request struct
            .await
            .map_err(QdrantError::Client)?;
        Ok(())
    }

    pub async fn search_points(
        &self,
        collection_name: &str,
        vector: &[f32],
        limit: u64,
    ) -> QResult<Vec<qdrant_client::qdrant::ScoredPoint>> {
        let search_points_request = SearchPoints {
            collection_name: collection_name.to_string(),
            vector: vector.to_vec(),
            limit,
            with_payload: Some(true.into()),
            ..Default::default()
        };
        // Call the underlying client's method
        let search_result_response = self
            .client
            .search_points(search_points_request) // Pass the request struct by value
            .await
            .map_err(QdrantError::Client)?;
        // Extract results from the response struct
        Ok(search_result_response.result)
    }

    pub async fn delete_points(&self, collection_name: &str, ids: &[PointId]) -> QResult<()> {
        // Construct the selector
        let points_selector = PointsSelector {
            points_selector_one_of: Some(PointsSelectorOneOf::Points(PointsIdsList {
                ids: ids.to_vec(),
            })),
        };
        // Construct the request struct
        let delete_request = qdrant_client::qdrant::DeletePoints {
            collection_name: collection_name.to_string(),
            wait: Some(true), // Wait for operation to complete - adjust if needed
            points: Some(points_selector),
            ordering: None,
            shard_key_selector: None,
        };
        // Call the underlying client's method
        self.client
            .delete_points(delete_request) // Pass the request struct
            .await
            .map_err(QdrantError::Client)?;
        Ok(())
    }

    pub async fn get_points_by_id(
        &self,
        collection_name: &str,
        ids: Vec<PointId>,
    ) -> QResult<Vec<qdrant_client::qdrant::RetrievedPoint>> {
        // Construct the request struct directly with the provided IDs
        let get_request = qdrant_client::qdrant::GetPoints {
            collection_name: collection_name.to_string(),
            ids,
            with_payload: None,
            with_vectors: None,
            read_consistency: None,
            shard_key_selector: None,
            timeout: None,
        };

        // Call the underlying client's method
        let result_response = self
            .client
            .get_points(get_request) // Pass the request struct
            .await
            .map_err(QdrantError::Client)?;
        // Extract results from the response
        Ok(result_response.result)
    }

    // Add collection_exists method
    pub async fn collection_exists(&self, collection_name: &str) -> QResult<bool> {
        self.client
            .collection_exists(collection_name)
            .await
            .map_err(QdrantError::Client)
    }
}

pub async fn new_qdrant_client(url: &str) -> VectordbResult<Qdrant> {
    Qdrant::new(url).await.map_err(|e| e.into())
}

#[cfg(test)]
mod tests {
    use qdrant_client::qdrant::{PointId as QdrantPointId, PointsIdsList, PointsSelector, GetPointsBuilder, PointStruct, Distance};
    // use qdrant_client::QdrantError as NativeQdrantError; // Keep if needed for specific checks
    use serde_json::json;
    use uuid::Uuid;
    use tokio::runtime::Builder; // Keep builder if needed elsewhere, maybe not needed now

    use super::*;

    const TEST_QDRANT_URL: &str = "http://localhost:6334";
    const TEST_VECTOR_SIZE: u64 = 4;
    const TEST_DISTANCE: Distance = Distance::Cosine;

    // Make the test helper async and await the new call
    async fn test_client() -> Qdrant {
        Qdrant::new(TEST_QDRANT_URL)
            .await // Await the async new method
            .expect("Failed to create test client")
    }

    fn unique_collection_name() -> String {
        format!("vectordb-test-{}", Uuid::new_v4())
    }

    #[tokio::test]
    async fn test_new_qdrant_client_wrapper() {
        let client_result = new_qdrant_client(TEST_QDRANT_URL).await;
        assert!(client_result.is_ok());
    }

    #[tokio::test]
    async fn test_create_list_delete_collection() {
        let client = test_client().await;
        let collection_name = unique_collection_name();
        let client_ref = &client; // Use client directly

        let create_result = client_ref
            .create_collection(
                &collection_name,
                TEST_VECTOR_SIZE,
                TEST_DISTANCE,
            )
            .await;
        assert!(
            create_result.is_ok(),
            "Create failed: {:?}",
            create_result.err()
        );

        let list_result = client_ref.list_collections().await;
        assert!(list_result.is_ok());
        let collections = list_result.unwrap();
        assert!(
            collections.iter().any(|c| c == &collection_name),
            "Collection {} not found after creation. Collections: {:?}",
            collection_name,
            collections
        );

        // Explicitly delete at the end of the test
        let delete_result = client_ref.delete_collection(&collection_name).await;
        assert!(delete_result.is_ok(), "Explicit delete failed: {:?}", delete_result.err());

        // Optionally, verify deletion
        let list_after_delete = client_ref.list_collections().await.expect("Failed to list after delete");
        assert!(!list_after_delete.iter().any(|c| c == &collection_name), "Collection should be deleted");
    }

    #[tokio::test]
    async fn test_create_existing_collection() {
        let client = test_client().await;
        let collection_name = unique_collection_name();
        let client_ref = &client; // Use client directly

        // Create once
        assert!(client_ref
            .create_collection(
                &collection_name,
                TEST_VECTOR_SIZE,
                TEST_DISTANCE,
            )
            .await
            .is_ok());

        // Try creating again
        let create_again_result = client_ref
            .create_collection(
                &collection_name,
                TEST_VECTOR_SIZE,
                TEST_DISTANCE,
            )
            .await;
        
        match create_again_result {
            Ok(_) => panic!("Creating an existing collection should fail, but it succeeded."),
            Err(QdrantError::Client(NativeQdrantError::ResponseError { status })) => {
                // Check if it's the expected 'already exists' kind of error
                let msg = status.message().to_lowercase();
                assert!(
                    msg.contains("exists")
                        || msg.contains("already present")
                        || status.code() == tonic::Code::AlreadyExists
                        || status.code() == tonic::Code::InvalidArgument // Sometimes used
                        || status.code() == tonic::Code::FailedPrecondition, // Another possibility
                    "Unexpected RPC error: code={:?}, message='{}'",
                    status.code(),
                    status.message()
                );
            }
            Err(e) => panic!("Unexpected error type: {:?}", e),
        }

        // Explicitly delete at the end of the test
        let delete_result = client_ref.delete_collection(&collection_name).await;
        assert!(delete_result.is_ok(), "Explicit delete failed: {:?}", delete_result.err());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_collection() {
        let client = test_client().await;
        let collection_name = unique_collection_name(); // Ensure it's unique and doesn't exist

        // Attempt to delete
        let delete_result = client.delete_collection(&collection_name).await;

        // Updated Assertion: Check that the delete operation succeeded (or handle specific errors if needed)
        // And then verify the collection does not exist.
        assert!(delete_result.is_ok(), "Delete operation failed unexpectedly: {:?}", delete_result.err());

        // Verify absence using collection_exists or list_collections
        let exists = client.collection_exists(&collection_name).await.expect("Failed to check existence");
        assert!(!exists, "Collection '{}' should not exist after deletion attempt.", collection_name);
    }

    #[tokio::test]
    async fn test_upsert_search_delete() {
        let client = test_client().await;
        let collection_name = unique_collection_name();
        let client_ref = &client; // Use client directly

        client_ref
            .create_collection(&collection_name, TEST_VECTOR_SIZE, TEST_DISTANCE)
            .await
            .expect("Failed to create collection for upsert test");

        let point_id_1: QdrantPointId = 1_u64.into();
        let point_id_2: QdrantPointId = Uuid::new_v4().to_string().into();
        let vector_1: Vec<f32> = vec![0.1, 0.2, 0.3, 0.4];
        let vector_2: Vec<f32> = vec![0.5, 0.6, 0.7, 0.8];

        // Define payload using HashMap<String, Value>
        let mut payload_1 = HashMap::new();
        payload_1.insert("name".to_string(), json!("point1"));
        payload_1.insert("value".to_string(), json!(10));

        let mut payload_2 = HashMap::new();
        payload_2.insert("name".to_string(), json!("point2"));
        payload_2.insert("value".to_string(), json!(20));

        // Convert HashMap to qdrant_client::Payload using try_from and serde_json::to_value
        let qdrant_payload_1 = Payload::try_from(serde_json::to_value(payload_1).expect("Failed to convert map to JSON value"))
            .expect("Failed to convert JSON value to Qdrant Payload");
        let qdrant_payload_2 = Payload::try_from(serde_json::to_value(payload_2).expect("Failed to convert map to JSON value"))
            .expect("Failed to convert JSON value to Qdrant Payload");

        let points_to_upsert = vec![
            PointStruct::new(point_id_1.clone(), vector_1.clone(), qdrant_payload_1),
            PointStruct::new(point_id_2.clone(), vector_2.clone(), qdrant_payload_2),
        ];

        // Upsert
        let upsert_result = client_ref
            .upsert_points(&collection_name, points_to_upsert)
            .await;
        assert!(upsert_result.is_ok(), "Upsert failed: {:?}", upsert_result.err());

        // Wait briefly for indexing (optional, but good practice in tests)
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Search
        let search_vector = vec![0.15, 0.25, 0.35, 0.45]; // Close to vector_1
        let search_result = client_ref
            .search_points(&collection_name, &search_vector, 1)
            .await;
        assert!(search_result.is_ok(), "Search failed: {:?}", search_result.err());
        let search_hits = search_result.unwrap();
        assert_eq!(search_hits.len(), 1);
        assert_eq!(
            search_hits[0].id.clone().unwrap(),
            point_id_1.clone(),
            "Search did not return the expected closest point (ID 1)"
        );
        // Check payload retrieval
        let retrieved_payload = &search_hits[0].payload;
        assert!(retrieved_payload.contains_key("name"));
        assert_eq!(retrieved_payload.get("name").unwrap().as_str().unwrap(), "point1");

        // Delete
        let ids_to_delete = vec![point_id_1.clone(), point_id_2.clone()];
        let delete_result = client_ref
            .delete_points(&collection_name, &ids_to_delete)
            .await;
        assert!(delete_result.is_ok(), "Delete points failed: {:?}", delete_result.err());

        // Wait briefly for indexing (optional)
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Verify deletion by trying to retrieve them
        let retrieve_result = client_ref
            .get_points_by_id(&collection_name, ids_to_delete.clone())
            .await;

        assert!(retrieve_result.is_ok(), "Retrieval check failed: {:?}", retrieve_result.err());
        // Adjust assertion for the new get_points_by_id which returns Vec<RetrievedPoint>
        assert!(retrieve_result.unwrap().is_empty(), "Points were not deleted");

        // Explicitly delete collection at the end of the test
        let delete_collection_result = client_ref.delete_collection(&collection_name).await;
        assert!(delete_collection_result.is_ok(), "Explicit collection delete failed: {:?}", delete_collection_result.err());
    }
}