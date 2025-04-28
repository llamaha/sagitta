use crate::{
    embedding::EmbeddingHandler,
    error::{Result, VectorDBError},
    qdrant_client_trait::QdrantClientTrait,
};
use qdrant_client::{
    qdrant::{
        Filter, PointStruct, PrefetchQueryBuilder, Query, QueryPoints, QueryPointsBuilder,
        SearchResponse, VectorParams, VectorsConfig, Distance, PointId, RecommendPointsBuilder, points_selector, ScoredPoint, value, WithPayloadSelector, Condition, FieldCondition, Match,
        QueryResponse,
    },
    Qdrant, // Use concrete type if trait causes issues, or keep trait
};
use std::sync::Arc;


/// Performs a hybrid vector search in a specified Qdrant collection using a rescoring approach.
///
/// # Arguments
/// * `client` - An Arc-wrapped Qdrant client (or trait object).
/// * `collection_name` - The name of the collection to search.
/// * `embedding_handler` - Handler to generate the query embedding.
/// * `query_text` - The text to search for.
/// * `limit` - The final maximum number of results to return after rescoring.
/// * `filter` - An optional Qdrant filter to apply to the initial prefetch stage.
///
/// # Returns
/// * `Result<QueryResponse>` - The search results from Qdrant.
pub async fn search_collection<C>(
    client: Arc<C>,
    collection_name: &str,
    embedding_handler: &EmbeddingHandler,
    query_text: &str,
    limit: u64,
    filter: Option<Filter>,
) -> Result<QueryResponse>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    log::debug!(
        "Core: Hybrid searching collection \"{}\" for query: \"{}\" with limit {} and filter: {:?}",
        collection_name,
        query_text,
        limit,
        filter
    );

    // 1. Get query embedding
    let query_embedding = embedding_handler
        .embed(&[query_text])?
        .into_iter()
        .next()
        .ok_or_else(|| {
            VectorDBError::EmbeddingError("Failed to generate embedding for the query ".to_string())
        })?;
    log::trace!("Core: Generated query embedding.");

    // Define prefetch parameters
    let prefetch_limit = limit * 5; // Fetch more candidates initially

    // 2. Build hybrid search request using QueryPointsBuilder for rescoring
    let mut prefetch_builder = PrefetchQueryBuilder::default()
        .query(Query::new_nearest(query_embedding.clone())) // Use clone for prefetch query
        .limit(prefetch_limit);

    if let Some(f) = filter {
        prefetch_builder = prefetch_builder.filter(f);
        log::trace!("Core: Applied search filter to prefetch stage.");
    }

    let mut query_builder = QueryPointsBuilder::new(collection_name)
        .add_prefetch(prefetch_builder)
        .query(Query::new_nearest(query_embedding)) // Use original embedding for final query/rescore
        .limit(limit)
        .with_payload(true); // Include payload in final results

    let query_request: QueryPoints = query_builder.into();

    // 3. Perform search using query endpoint
    log::debug!("Core: Executing hybrid search request...");
    // Assuming QdrantClientTrait needs an update or the underlying client handles QueryPoints
    // We might need to add a `query_points` method to the trait similar to `search_points`
    let search_response = client.query(query_request).await?; // !!! This likely requires changes to QdrantClientTrait !!!
    log::info!("Found {} search results after rescoring.", search_response.result.len());
    Ok(search_response)
}

// Potential future function specifically for repositories?
// pub async fn search_repository(...) -> Result<SearchResponse> {
//     // Might involve looking up collection name, default branch etc.
//     // Calls search_collection internally
// }

#[cfg(test)]
mod tests {
    extern crate mockall; // Keep this for predicate matching if needed, or remove

    use super::*;
    use crate::embedding::EmbeddingModelType;
    use crate::qdrant_client_trait::QdrantClientTrait; // Import the actual trait
    use qdrant_client::qdrant::{
        QueryPoints, QueryResponse, ScoredPoint, PointId, Filter, SearchPoints, SearchResponse,
        HealthCheckReply, CollectionInfo, CountPoints, CountResponse, PointsSelector, 
        DeletePoints, ScrollPoints, ScrollResponse, UpsertPoints, PointsOperationResponse,
        CreateCollection, DeleteCollection,
    };
    use mockall::predicate::*; // Keep for .withf() argument matching
    use std::sync::{Arc, Mutex};
    use async_trait::async_trait;

    // Manual Mock Implementation
    #[derive(Clone, Debug)]
    struct ManualMockQdrantClient {
        // Use Mutex to allow checks after Arc moves ownership
        query_points_called: Arc<Mutex<bool>>,
        expected_query_response: Arc<Mutex<Option<Result<QueryResponse>>>>,
        query_called: Arc<Mutex<bool>>,
        // Add fields for other methods if they need to be mocked
    }

    impl ManualMockQdrantClient {
        fn new() -> Self {
            Self {
                query_points_called: Arc::new(Mutex::new(false)),
                expected_query_response: Arc::new(Mutex::new(None)),
                query_called: Arc::new(Mutex::new(false)),
            }
        }

        fn expect_query_points(&self, response: Result<QueryResponse>) {
            *self.expected_query_response.lock().unwrap() = Some(response);
        }
        fn expect_query(&self, response: Result<QueryResponse>) {
            *self.expected_query_response.lock().unwrap() = Some(response);
        }

        fn verify_query_points_called(&self) -> bool {
            *self.query_points_called.lock().unwrap()
        }
        fn verify_query_called(&self) -> bool {
            *self.query_called.lock().unwrap()
        }
    }

    #[async_trait]
    impl QdrantClientTrait for ManualMockQdrantClient {
        async fn query_points(&self, request: QueryPoints) -> Result<QueryResponse> {
            // Mark the method as called
            *self.query_points_called.lock().unwrap() = true;
            
            // Log the request for debugging (optional)
            log::debug!("ManualMock: query_points called with {:?}", request);

            // Return the expected response
            self.expected_query_response.lock().unwrap()
                .take() // Take the response out, so it's consumed
                .expect("query_points called without setting expected response")
        }

        async fn query(&self, request: QueryPoints) -> Result<QueryResponse> {
            // Mark the method as called
            *self.query_called.lock().unwrap() = true;
            
            // Log the request for debugging (optional)
            log::debug!("ManualMock: query called with {:?}", request);

            // Return the expected response
            self.expected_query_response.lock().unwrap()
                .take() // Take the response out, so it's consumed
                .expect("query called without setting expected response")
        }

        // --- Implement other trait methods with default "unimplemented" behavior --- 
        async fn health_check(&self) -> Result<HealthCheckReply> { unimplemented!("health_check not mocked") }
        async fn delete_collection(&self, _collection_name: String) -> Result<bool> { unimplemented!("delete_collection not mocked") }
        async fn search_points(&self, _request: SearchPoints) -> Result<SearchResponse> { unimplemented!("search_points not mocked") }
        async fn get_collection_info(&self, _collection_name: String) -> Result<CollectionInfo> { unimplemented!("get_collection_info not mocked") }
        async fn count(&self, _request: CountPoints) -> Result<CountResponse> { unimplemented!("count not mocked") }
        async fn collection_exists(&self, _collection_name: String) -> Result<bool> { unimplemented!("collection_exists not mocked") }
        async fn delete_points_blocking(&self, _collection_name: &str, _points_selector: &PointsSelector) -> Result<()> { unimplemented!("delete_points_blocking not mocked") }
        async fn scroll(&self, _request: ScrollPoints) -> Result<ScrollResponse> { unimplemented!("scroll not mocked") }
        async fn upsert_points(&self, _request: UpsertPoints) -> Result<PointsOperationResponse> { unimplemented!("upsert_points not mocked") }
        async fn create_collection(&self, _collection_name: &str, _vector_dimension: u64) -> Result<bool> { unimplemented!("create_collection not mocked") }
        async fn delete_points(&self, _request: DeletePoints) -> Result<PointsOperationResponse> { unimplemented!("delete_points not mocked") }
        
    }

    #[tokio::test]
    async fn test_search_collection_calls_query_points() {
        // Arrange
        // Use the manual mock
        let manual_mock_client = ManualMockQdrantClient::new();
        let client_arc = Arc::new(manual_mock_client.clone()); // Clone Arc for handler

        // --- Embedding Handler Setup (same as before) ---
        let dummy_config = crate::config::AppConfig { 
            onnx_model_path: Some("dummy/model.onnx".into()),
            onnx_tokenizer_path: Some("dummy/tokenizer.json".into()),
            ..Default::default()
        };
        let embedder_handler = EmbeddingHandler::new(&dummy_config)
            .expect("Failed to create dummy EmbeddingHandler for test"); 
        // --- End Embedding Handler Setup ---

        let collection_name = "test_collection";
        let query_text = "test query";
        let limit = 10u64;
        let prefetch_limit = limit * 5;
        let dummy_embedding = vec![0.1f32; 384]; 

        // Set expectations on the manual mock
        let point_id: PointId = 1u64.into(); 
        let expected_response = Ok(QueryResponse {
            result: vec![ScoredPoint { 
                id: Some(point_id), 
                version: 1, 
                score: 0.9, 
                payload: Default::default(), 
                vectors: None, 
                shard_key: None,
                order_value: None, 
            }],
            time: 0.1,
            usage: None,
        });
        manual_mock_client.expect_query(expected_response);

        // Act
        let result = search_collection(
            client_arc, // Pass Arc<ManualMockQdrantClient>
            collection_name,
            &embedder_handler, 
            query_text,
            limit,
            None,
        ).await;

        // Assert
        assert!(result.is_ok()); // Check if the function returned Ok
        // Verify the mock method was called
        assert!(manual_mock_client.verify_query_called(), "query should have been called");
        
        let response = result.unwrap();
        assert_eq!(response.result.len(), 1); // Check the response content
        // Add more assertions on the response content if needed
    }

    // TODO: Add test for search_collection with a filter (would need mock setup)
    // TODO: Add test for embedding error 
    // TODO: Add test for qdrant client error (set expected_query_response to Err)
}
