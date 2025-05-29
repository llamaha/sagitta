#![allow(clippy::arc_with_non_send_sync)] 

use crate::qdrant_client_trait::QdrantClientTrait;
use crate::error::{Result, SagittaError};
use async_trait::async_trait;
use qdrant_client::qdrant::{
    CollectionInfo, CountPoints, CountResponse, DeletePoints, HealthCheckReply, PointsOperationResponse, PointsSelector, QueryPoints, 
    QueryResponse, /*ScoredPoint,*/ ScrollPoints, ScrollResponse, SearchPoints, SearchResponse, UpsertPoints, /*PointId*/ CreateCollection
};
use std::sync::{Arc, Mutex};

/// A manual mock implementation of `QdrantClientTrait` for testing purposes.
/// Allows setting expectations and verifying calls for specific methods.
#[derive(Clone, Debug)]
pub struct ManualMockQdrantClient {
    // For query and query_points
    query_called: Arc<Mutex<bool>>,
    query_points_called: Arc<Mutex<bool>>,
    expected_query_response: Arc<Mutex<Option<Result<QueryResponse>>>>,
    
    // For create_collection
    create_collection_called: Arc<Mutex<bool>>,
    create_collection_args: Arc<Mutex<Option<(String, u64)>>>,
    expected_create_collection_response: Arc<Mutex<Option<Result<bool>>>>,

    // For collection_exists
    collection_exists_called: Arc<Mutex<u32>>,
    collection_exists_args: Arc<Mutex<Vec<String>>>,
    expected_collection_exists_responses: Arc<Mutex<Vec<Result<bool>>>>,
}

impl ManualMockQdrantClient {
    /// Creates a new instance of the manual mock client.
    pub fn new() -> Self {
        Self {
            query_called: Arc::new(Mutex::new(false)),
            query_points_called: Arc::new(Mutex::new(false)),
            expected_query_response: Arc::new(Mutex::new(None)),
            create_collection_called: Arc::new(Mutex::new(false)),
            create_collection_args: Arc::new(Mutex::new(None)),
            expected_create_collection_response: Arc::new(Mutex::new(None)),
            collection_exists_called: Arc::new(Mutex::new(0)),
            collection_exists_args: Arc::new(Mutex::new(Vec::new())),
            expected_collection_exists_responses: Arc::new(Mutex::new(Vec::new())),
        }
    }

    // --- Methods for query/query_points --- 
    /// Sets the expected response for the next `query` or `query_points` call.
    pub fn expect_query(&self, response: Result<QueryResponse>) {
        *self.expected_query_response.lock().unwrap() = Some(response);
    }

    /// Verifies if the `query` method was called.
    pub fn verify_query_called(&self) -> bool {
        *self.query_called.lock().unwrap()
    }
    
    /// Sets the expected response for the next `query` or `query_points` call. 
    /// (Currently shares expectation with `expect_query`).
    pub fn expect_query_points(&self, response: Result<QueryResponse>) {
        *self.expected_query_response.lock().unwrap() = Some(response); 
    }

    /// Verifies if the `query_points` method was called.
    pub fn verify_query_points_called(&self) -> bool {
        *self.query_points_called.lock().unwrap()
    }

    // --- Methods for create_collection --- 
    /// Sets the expected response for the next `create_collection` call.
    pub fn expect_create_collection(&self, response: Result<bool>) {
        *self.expected_create_collection_response.lock().unwrap() = Some(response);
    }

    /// Verifies if the `create_collection` method was called.
    pub fn verify_create_collection_called(&self) -> bool {
        *self.create_collection_called.lock().unwrap()
    }

    /// Verifies the arguments passed to the last `create_collection` call.
    pub fn verify_create_collection_args(&self, name: &str, dim: u64) -> bool {
        if let Some(args) = &*self.create_collection_args.lock().unwrap() {
            args.0 == name && args.1 == dim
        } else {
            false
        }
    }

    // --- Methods for collection_exists --- 
    /// Adds an expected response to the queue for future `collection_exists` calls.
    pub fn expect_collection_exists(&self, response: Result<bool>) {
        self.expected_collection_exists_responses.lock().unwrap().push(response);
    }

    /// Returns the number of times `collection_exists` was called.
    pub fn verify_collection_exists_called_times(&self) -> u32 {
        *self.collection_exists_called.lock().unwrap()
    }

    /// Returns a clone of the arguments passed to each `collection_exists` call.
    pub fn get_collection_exists_args(&self) -> Vec<String> {
        self.collection_exists_args.lock().unwrap().clone()
    }
}

#[async_trait]
impl QdrantClientTrait for ManualMockQdrantClient {
    async fn query_points(&self, _request: QueryPoints) -> Result<QueryResponse> {
        *self.query_points_called.lock().unwrap() = true;
        log::debug!("ManualMock: query_points called");
        self.expected_query_response.lock().unwrap().take()
            .unwrap_or_else(|| Err(SagittaError::Other("Mock Error: query_points called without expected response".into())))
    }

    async fn query(&self, _request: QueryPoints) -> Result<QueryResponse> {
        *self.query_called.lock().unwrap() = true;
        log::debug!("ManualMock: query called");
        self.expected_query_response.lock().unwrap().take()
            .unwrap_or_else(|| Err(SagittaError::Other("Mock Error: query called without expected response".into())))
    }

    async fn create_collection(&self, collection_name: &str, vector_dimension: u64) -> Result<bool> {
        *self.create_collection_called.lock().unwrap() = true;
        *self.create_collection_args.lock().unwrap() = Some((collection_name.to_string(), vector_dimension));
        log::debug!("ManualMock: create_collection called with name: {}, dim: {}", collection_name, vector_dimension);
        self.expected_create_collection_response.lock().unwrap().take()
            .unwrap_or_else(|| Err(SagittaError::Other("Mock Error: create_collection called without expected response".into())))
    }

    async fn create_collection_detailed(&self, request: CreateCollection) -> Result<bool> {
        // For the mock, we'll just delegate to the simpler create_collection method
        // Extract collection name and vector size from the request
        let collection_name = &request.collection_name;
        let vector_size = if let Some(vectors_config) = &request.vectors_config {
            if let Some(config) = &vectors_config.config {
                match config {
                    qdrant_client::qdrant::vectors_config::Config::Params(params) => params.size,
                    qdrant_client::qdrant::vectors_config::Config::ParamsMap(_) => 384, // Default fallback
                }
            } else {
                384 // Default fallback
            }
        } else {
            384 // Default fallback
        };
        
        log::debug!("ManualMock: create_collection_detailed called, delegating to create_collection");
        self.create_collection(collection_name, vector_size).await
    }

    async fn collection_exists(&self, collection_name: String) -> Result<bool> {
        *self.collection_exists_called.lock().unwrap() += 1;
        self.collection_exists_args.lock().unwrap().push(collection_name.clone());
        log::debug!("ManualMock: collection_exists called with name: {}", collection_name);
        let mut responses = self.expected_collection_exists_responses.lock().unwrap();
        if responses.is_empty() {
            Err(SagittaError::Other("Mock Error: collection_exists called without expected response(s)".into()))
        } else {
            responses.remove(0) // FIFO for multiple expected calls
        }
    }

    // --- Implement other trait methods with default "unimplemented" behavior --- 
    async fn health_check(&self) -> Result<HealthCheckReply> { unimplemented!("health_check not mocked in ManualMockQdrantClient") }
    async fn delete_collection(&self, _collection_name: String) -> Result<bool> { unimplemented!("delete_collection not mocked in ManualMockQdrantClient") }
    async fn search_points(&self, _request: SearchPoints) -> Result<SearchResponse> { unimplemented!("search_points not mocked in ManualMockQdrantClient") }
    async fn get_collection_info(&self, _collection_name: String) -> Result<CollectionInfo> { unimplemented!("get_collection_info not mocked in ManualMockQdrantClient") }
    async fn count(&self, _request: CountPoints) -> Result<CountResponse> { unimplemented!("count not mocked in ManualMockQdrantClient") }
    async fn delete_points_blocking(&self, _collection_name: &str, _points_selector: &PointsSelector) -> Result<()> { unimplemented!("delete_points_blocking not mocked in ManualMockQdrantClient") }
    async fn scroll(&self, _request: ScrollPoints) -> Result<ScrollResponse> { unimplemented!("scroll not mocked in ManualMockQdrantClient") }
    async fn upsert_points(&self, _request: UpsertPoints) -> Result<PointsOperationResponse> { unimplemented!("upsert_points not mocked in ManualMockQdrantClient") }
    async fn delete_points(&self, _request: DeletePoints) -> Result<PointsOperationResponse> { unimplemented!("delete_points not mocked in ManualMockQdrantClient") }
} 