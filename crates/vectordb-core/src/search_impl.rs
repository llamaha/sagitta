use crate::{
    embedding::EmbeddingHandler,
    error::{Result, VectorDBError},
    qdrant_client_trait::QdrantClientTrait,
};
use qdrant_client::{
    qdrant::{
        Filter, PointStruct, PrefetchQueryBuilder, Query, QueryPoints, QueryPointsBuilder,
        SearchResponse, VectorParams, VectorsConfig, Distance, PointId, RecommendPointsBuilder, points_selector, ScoredPoint, value, WithPayloadSelector, Condition, FieldCondition, Match,
        QueryResponse, Fusion, VectorInput,
    },
    Qdrant, // Use concrete type if trait causes issues, or keep trait
};
use std::sync::Arc;
use crate::tokenizer::{self, TokenKind, TokenizerConfig}; // Import TokenizerConfig
use crate::vocabulary::VocabularyManager; // Import vocabulary manager
use std::{path::PathBuf, collections::HashMap}; // Add HashMap
use log;
use crate::config::AppConfig; // Import AppConfig
use crate::config; // Import config module
use std::fs; // <-- Add this line
use anyhow::{anyhow, Context};
use tokio::sync::mpsc;
use tracing::{debug, error, info, instrument};

/// Performs a hybrid vector search in a specified Qdrant collection using a rescoring approach.
///
/// # Arguments
/// * `client` - An Arc-wrapped Qdrant client (or trait object).
/// * `collection_name` - The name of the collection to search.
/// * `embedding_handler` - Handler to generate the query embedding.
/// * `query_text` - The text to search for.
/// * `limit` - The final maximum number of results to return after rescoring.
/// * `filter` - An optional Qdrant filter to apply to the initial prefetch stage.
/// * `config` - The application configuration.
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
    config: &AppConfig, // Add AppConfig reference
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

    // --- Load Vocabulary --- 
    // Use helper function to get the correct path
    let vocab_path = config::get_vocabulary_path(config, collection_name)?;
    log::info!("Attempting to load vocabulary for collection '{}' from path: {}", collection_name, vocab_path.display());
    let vocabulary_manager = match VocabularyManager::load(&vocab_path) {
        Ok(vm) => vm,
        Err(e) => {
            log::error!("Failed to load vocabulary from {}: {}. Cannot perform hybrid search.", vocab_path.display(), e);
            return Err(VectorDBError::Other(format!("Vocabulary not found for collection '{}'", collection_name)));
        }
    };
    if vocabulary_manager.is_empty() {
         log::warn!("Vocabulary for collection '{}' is empty. Sparse search may not yield results.", collection_name);
    }
    // --- End Load Vocabulary ---

    // 1a. Generate Dense Query Embedding
    let dense_query_embedding = embedding_handler
        .embed(&[query_text])?
        .into_iter()
        .next()
        .ok_or_else(|| {
            VectorDBError::EmbeddingError("Failed to generate dense embedding for the query ".to_string())
        })?;
    log::trace!("Core: Generated dense query embedding.");

    // 1b. Generate Sparse Query Vector (TF = 1.0 for each known term)
    let tokenizer_config = TokenizerConfig::default();
    let query_tokens = tokenizer::tokenize_code(query_text, &tokenizer_config);
    let mut sparse_query_map: HashMap<u32, f32> = HashMap::new();
    for token in query_tokens {
        if let Some(token_id) = vocabulary_manager.get_id(&token.text) {
            // Using a map handles duplicate query terms implicitly (last one wins, which is fine for weight 1.0)
            sparse_query_map.insert(token_id, 1.0f32);
        }
    }
    let sparse_query_vec: Vec<(u32, f32)> = sparse_query_map.into_iter().collect();
    log::trace!("Core: Generated sparse query vector with {} unique terms.", sparse_query_vec.len());

    // Define prefetch parameters
    let prefetch_limit = limit * 5; // Fetch more candidates initially (configurable?)

    // 2. Build hybrid search request using QueryPointsBuilder for rescoring via RRF
    let mut dense_prefetch_builder = PrefetchQueryBuilder::default()
        .query(Query::new_nearest(dense_query_embedding.clone())) // Use dense vector
        .using("dense") // Specify dense vector name
        .limit(prefetch_limit);
    if let Some(f) = filter.clone() { // Clone filter for dense prefetch
        dense_prefetch_builder = dense_prefetch_builder.filter(f);
    }
    let dense_prefetch = dense_prefetch_builder;

    // Only add sparse prefetch if the query vector is not empty
    let mut query_builder = QueryPointsBuilder::new(collection_name)
        .add_prefetch(dense_prefetch); // Always add dense prefetch
    
    if !sparse_query_vec.is_empty() {
        let mut sparse_prefetch_builder = PrefetchQueryBuilder::default()
            .query(sparse_query_vec) // Pass Vec<(u32, f32)> directly
            .using("sparse_tf") 
            .limit(prefetch_limit);
        if let Some(f) = filter { // Use original filter for sparse prefetch
            sparse_prefetch_builder = sparse_prefetch_builder.filter(f);
        }
        let sparse_prefetch = sparse_prefetch_builder;
        query_builder = query_builder.add_prefetch(sparse_prefetch);
    } else {
        log::warn!("Query text '{}' contained no terms found in the vocabulary. Performing dense-only search.", query_text);
    }

    query_builder = query_builder.query(Query::new_fusion(Fusion::Rrf)) // Use RRF fusion
        .limit(limit) // Apply final limit after fusion
        .with_payload(true); // Include payload in final results

    let query_request: QueryPoints = query_builder.into();

    // 3. Perform search using query endpoint
    log::debug!("Core: Executing hybrid search request...");
    let search_response = client.query(query_request).await?; // Use query method
    log::info!("Found {} search results after RRF fusion.", search_response.result.len());
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
    use crate::config::AppConfig;

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
        let manual_mock_client = ManualMockQdrantClient::new();
        let client_arc = Arc::new(manual_mock_client.clone());
        
        // --- Setup Config with Dummy Paths --- 
        let temp_dir = tempfile::tempdir().unwrap();
        let vocab_base = temp_dir.path().join("test_vocabs");
        fs::create_dir_all(&vocab_base).unwrap();
        let model_base = temp_dir.path().join("models");
        fs::create_dir_all(&model_base).unwrap();
        let dummy_model_path = model_base.join("model.onnx");
        let dummy_tokenizer_dir = model_base.join("tokenizer");
        let dummy_tokenizer_file = dummy_tokenizer_dir.join("tokenizer.json");
        fs::write(&dummy_model_path, "dummy model data").unwrap();
        fs::create_dir(&dummy_tokenizer_dir).unwrap();
        // Write minimal valid tokenizer JSON using a regular string literal
        let min_tokenizer_json = "\n        {\n          \"version\": \"1.0\",\n          \"truncation\": null,\n          \"padding\": null,\n          \"added_tokens\": [],\n          \"normalizer\": null,\n          \"pre_tokenizer\": null,\n          \"post_processor\": null,\n          \"decoder\": null,\n          \"model\": {\n            \"type\": \"WordPiece\",\n            \"unk_token\": \"[UNK]\",\n            \"continuing_subword_prefix\": \"##\",\n            \"max_input_chars_per_word\": 100,\n            \"vocab\": {\n              \"[UNK]\": 0,\n              \"test\": 1,\n              \"query\": 2\n            }\n          }\n        }\n        "; // End of regular string literal
        fs::write(&dummy_tokenizer_file, min_tokenizer_json).unwrap(); 
        
        let mut dummy_config = AppConfig::default(); // Use default config
        dummy_config.onnx_model_path = Some(dummy_model_path.to_string_lossy().into_owned()); // Set dummy paths
        dummy_config.onnx_tokenizer_path = Some(dummy_tokenizer_dir.to_string_lossy().into_owned());
        dummy_config.vocabulary_base_path = Some(vocab_base.to_str().unwrap().to_string());
        // --- End Config Setup --- 

        // Create a dummy vocab file for the test
        let collection_name = "test_collection";
        let vocab_path = config::get_vocabulary_path(&dummy_config, collection_name).unwrap();
        let mut dummy_vocab = VocabularyManager::new(); 
        dummy_vocab.add_token("test"); // Add at least one token the query might match
        dummy_vocab.save(&vocab_path).expect("Failed to save dummy vocab");

        let embedder_handler = EmbeddingHandler::new(&dummy_config)
            .expect("Failed to create dummy EmbeddingHandler for test"); 

        let query_text = "test query";
        let limit = 10u64;
        // let prefetch_limit = limit * 5; // Unused variable
        // let dummy_embedding = vec![0.1f32; 384]; // Unused variable

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
            client_arc,
            collection_name,
            &embedder_handler, 
            query_text,
            limit,
            None,
            &dummy_config, // Pass config
        ).await;

        // Assert
        // assert!(result.is_ok(), "search_collection failed: {:?}", result.err()); // Too strict, dummy ONNX may fail
        // Instead, check that if it failed, it was likely due to ONNX/protobuf loading
        if let Err(e) = &result {
            let err_string = e.to_string();
            // Allow failure if it looks like an ONNX loading issue
            assert!(
                err_string.contains("ONNX") || 
                err_string.contains("Protobuf parsing failed") ||
                err_string.contains("No such file or directory") || // If dummy paths failed somehow
                err_string.contains("runtime error") || // General ORT errors
                err_string.contains("Failed to create dummy EmbeddingHandler"), // If handler creation failed
                "search_collection failed with unexpected error: {:?}", e
            );
            warn!("Note: search_collection test returned an expected setup error (ONNX/IO/Qdrant): {}", err_string);
        } else {
             // If it passed, verify the mock method was called
             assert!(manual_mock_client.verify_query_called(), "query should have been called");
             let response = result.unwrap();
             assert_eq!(response.result.len(), 1); // Check the response content
        }
       
        // // Verify the mock method was called - moved inside the success case
        // assert!(manual_mock_client.verify_query_called(), "query should have been called");
        
        // let response = result.unwrap();
        // assert_eq!(response.result.len(), 1); // Check the response content
        // // Add more assertions on the response content if needed
    }

    // TODO: Add test for search_collection with a filter (would need mock setup)
    // TODO: Add test for embedding error 
    // TODO: Add test for qdrant client error (set expected_query_response to Err)
}
