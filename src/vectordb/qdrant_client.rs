use qdrant_client::{
    client::QdrantClient,
    qdrant::{
        CreateCollection, Distance, VectorParams, PointStruct,
        // VectorsConfig, // No VectorsConfig needed for this structure
    },
    client::QdrantClientError as NativeQdrantError,
};
use thiserror::Error;
use anyhow;

#[derive(Error, Debug)]
pub enum QdrantError {
    #[error("Qdrant client error: {0}")]
    Client(#[from] NativeQdrantError),
    #[error("Collection already exists: {0}")]
    CollectionExists(String),
    #[error("Collection does not exist: {0}")]
    CollectionNotFound(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Configuration or connection error: {0}")]
    ConfigError(#[from] anyhow::Error), // Keep for build errors
    #[error("Other Qdrant error: {0}")]
    Other(String),
}

pub type Result<T, E = QdrantError> = std::result::Result<T, E>;

/// Creates a new Qdrant client connected to the specified URL.
pub fn new_qdrant_client(url: &str) -> Result<QdrantClient> { // Use QdrantClient
    let client = QdrantClient::from_url(url).build()?;
    Ok(client)
}

// --- Collection Management Functions ---

/// Creates a new collection in Qdrant.
pub async fn create_collection(
    client: &QdrantClient,
    name: &str,
    vector_size: u64,
    distance: Distance,
) -> Result<()> {
    client
        // Pass CreateCollection by reference
        .create_collection(&CreateCollection {
            collection_name: name.to_string(),
            // Use Some(VectorParams { ... }) directly
            vectors_config: Some(VectorParams {
                size: vector_size,
                distance: distance.into(),
                ..Default::default()
            }),
            ..Default::default()
        })
        .await?;
    Ok(())
}

/// Lists all collections in Qdrant.
pub async fn list_collections(client: &QdrantClient) -> Result<Vec<String>> {
    let response = client.list_collections().await?;
    Ok(response.collections.into_iter().map(|c| c.name).collect())
}

/// Deletes a collection from Qdrant.
pub async fn delete_collection(client: &QdrantClient, name: &str) -> Result<()> {
    client.delete_collection(name.to_string()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use qdrant_client::qdrant::Distance;
    use tokio;

    const TEST_COLLECTION_NAME: &str = "test_collection_phase2";
    const TEST_VECTOR_SIZE: u64 = 4; // Small dimension for testing
    const TEST_DISTANCE: Distance = Distance::Cosine;

    // Helper to get a client for tests
    fn get_test_client() -> QdrantClient { // Use QdrantClient
        new_qdrant_client("http://localhost:6333").expect("Failed to create test client")
    }

    // Helper to clean up collection after test
    async fn cleanup_test_collection(client: &QdrantClient) { // Use QdrantClient
        let _ = delete_collection(client, TEST_COLLECTION_NAME).await;
    }

    #[tokio::test]
    async fn test_client_connection() {
        let client = get_test_client();
        // Use list_collections as basic check
        let list_result = list_collections(&client).await;
        assert!(list_result.is_ok(), "Client connection failed (list_collections): {:?}", list_result.err());
        println!("Qdrant list collections successful.");
    }

    #[tokio::test]
    async fn test_create_list_delete_collection() {
        let client = get_test_client();
        cleanup_test_collection(&client).await;
        let create_result = create_collection(
            &client,
            TEST_COLLECTION_NAME,
            TEST_VECTOR_SIZE,
            TEST_DISTANCE,
        )
        .await;
        assert!(create_result.is_ok(), "Failed to create collection: {:?}", create_result.err());
        let list_result = list_collections(&client).await;
        assert!(list_result.is_ok(), "Failed to list collections: {:?}", list_result.err());
        let collections = list_result.unwrap();
        assert!(
            collections.iter().any(|name| name == TEST_COLLECTION_NAME),
            "Test collection '{}' not found in list: {:?}", TEST_COLLECTION_NAME, collections
        );
        let delete_result = delete_collection(&client, TEST_COLLECTION_NAME).await;
        assert!(delete_result.is_ok(), "Failed to delete collection: {:?}", delete_result.err());
        let list_after_delete_result = list_collections(&client).await;
        assert!(list_after_delete_result.is_ok(), "Failed to list collections after delete: {:?}", list_after_delete_result.err());
        let collections_after_delete = list_after_delete_result.unwrap();
        assert!(
            !collections_after_delete.iter().any(|name| name == TEST_COLLECTION_NAME),
            "Test collection '{}' still found after deletion: {:?}", TEST_COLLECTION_NAME, collections_after_delete
        );
        cleanup_test_collection(&client).await;
    }

    #[tokio::test]
    async fn test_create_existing_collection() {
        let client = get_test_client();
        cleanup_test_collection(&client).await;
        create_collection(&client, TEST_COLLECTION_NAME, TEST_VECTOR_SIZE, TEST_DISTANCE)
            .await
            .expect("Initial collection creation failed");
        let create_again_result = create_collection(
            &client,
            TEST_COLLECTION_NAME,
            TEST_VECTOR_SIZE,
            TEST_DISTANCE,
        )
        .await;
        assert!(create_again_result.is_err(), "Creating an existing collection should fail");
        cleanup_test_collection(&client).await;
    }

    #[tokio::test]
    async fn test_delete_nonexistent_collection() {
        let client = get_test_client();
        cleanup_test_collection(&client).await;
        let delete_result = delete_collection(&client, "non_existent_collection_123").await;
        assert!(delete_result.is_err(), "Deleting non-existent collection should return an error");
         match delete_result.err().unwrap() {
             QdrantError::Client(native_error) => {
                 let error_string = format!("{:?}", native_error);
                 assert!(error_string.contains("Not found") || error_string.contains("doesn\'t exist") || error_string.contains("NotFound"),
                        "Expected 'Not found' error, got: {}", error_string);
             }
             _ => panic!("Expected a Qdrant Client error, got something else"),
         }
    }
}