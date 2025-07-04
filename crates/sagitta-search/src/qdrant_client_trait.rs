use async_trait::async_trait;
use qdrant_client::qdrant::{
    DeleteCollection, HealthCheckReply, SearchPoints, CountPoints, GetCollectionInfoRequest, 
    CollectionInfo, CountResponse, SearchResponse, PointsSelector, DeletePoints,
    ScrollPoints, ScrollResponse,
    UpsertPoints, PointsOperationResponse, CreateCollection, Distance,
    VectorParamsBuilder, QueryPoints, QueryResponse,
    VectorsConfig, VectorParamsMap, vectors_config::Config as VectorsConfig_oneof_config,
    SparseVectorParams, SparseVectorConfig, Modifier
};
use qdrant_client::Qdrant;
// Import our custom error type instead
use crate::error::{SagittaError, Result}; // Use crate::error

// use mockall::automock for automatic mock generation in tests
#[cfg_attr(test, mockall::automock)] // Remove mockall
#[async_trait]
/// Trait defining the interface for a Qdrant client.
/// This allows for mocking the client in tests.
pub trait QdrantClientTrait: Send + Sync {
    /// Checks the health of the Qdrant server.
    async fn health_check(&self) -> Result<HealthCheckReply>;
    /// Deletes a collection from the Qdrant server.
    async fn delete_collection(&self, collection_name: String) -> Result<bool>;
    /// Searches for points in a collection.
    async fn search_points(&self, request: SearchPoints) -> Result<SearchResponse>;
    /// Gets information about a collection.
    async fn get_collection_info(&self, collection_name: String) -> Result<CollectionInfo>;
    /// Counts the number of points in a collection.
    async fn count(&self, request: CountPoints) -> Result<CountResponse>;
    /// Checks if a collection exists.
    async fn collection_exists(&self, collection_name: String) -> Result<bool>;
    /// Deletes points from a collection and waits for the operation to complete.
    async fn delete_points_blocking(&self, collection_name: &str, points_selector: &PointsSelector) -> Result<()>;
    /// Scrolls through points in a collection.
    async fn scroll(&self, request: ScrollPoints) -> Result<ScrollResponse>;
    /// Upserts points into a collection.
    async fn upsert_points(&self, request: UpsertPoints) -> Result<PointsOperationResponse>;
    /// Creates a new collection.
    async fn create_collection(&self, collection_name: &str, vector_dimension: u64) -> Result<bool>;
    /// Creates a new collection with detailed configuration.
    async fn create_collection_detailed(&self, request: CreateCollection) -> Result<bool>;
    /// Deletes points from a collection.
    async fn delete_points(&self, request: DeletePoints) -> Result<PointsOperationResponse>;
    /// Queries points in a collection using a query request.
    async fn query_points(&self, request: QueryPoints) -> Result<QueryResponse>;
    /// Performs a query on points in a collection.
    async fn query(&self, request: QueryPoints) -> Result<QueryResponse>;
    /// Lists all collections.
    async fn list_collections(&self) -> Result<Vec<String>>;
    // Add other methods used by the application as needed
}

#[async_trait]
impl QdrantClientTrait for Qdrant {
    async fn health_check(&self) -> Result<HealthCheckReply> {
        self.health_check().await.map_err(SagittaError::QdrantError)
    }

    async fn delete_collection(&self, collection_name: String) -> Result<bool> {
        let request = DeleteCollection {
            collection_name,
            ..Default::default()
        };
        Ok(self.delete_collection(request).await.map_err(SagittaError::QdrantError)?.result)
    }

    async fn search_points(&self, request: SearchPoints) -> Result<SearchResponse> {
        self.search_points(request).await.map_err(SagittaError::QdrantError)
    }

    async fn get_collection_info(&self, collection_name: String) -> Result<CollectionInfo> {
        let request = GetCollectionInfoRequest {
            collection_name: collection_name.clone(),
        };
        let response = self.collection_info(request).await.map_err(|e| SagittaError::QdrantError(e))?;
        match response.result {
            Some(info) => Ok(info),
            None => {
                let err_msg = format!("Collection info not found for '{}'", collection_name);
                Err(SagittaError::RepositoryNotFound(err_msg))
            }
        }
    }

    async fn count(&self, request: CountPoints) -> Result<CountResponse> {
        self.count(request).await.map_err(|e| SagittaError::QdrantError(e))
    }

    async fn collection_exists(&self, collection_name: String) -> Result<bool> {
        self.collection_exists(collection_name).await.map_err(|e| SagittaError::QdrantError(e))
    }

    async fn delete_points_blocking(&self, collection_name: &str, points_selector: &PointsSelector) -> Result<()> {
        let request = DeletePoints {
            collection_name: collection_name.to_string(),
            wait: Some(true),
            points: Some(points_selector.clone()),
            ..Default::default()
        };
        self.delete_points(request).await.map_err(|e| SagittaError::QdrantError(e))?;
        Ok(())
    }

    async fn scroll(&self, request: ScrollPoints) -> Result<ScrollResponse> {
        self.scroll(request).await.map_err(|e| SagittaError::QdrantError(e))
    }

    async fn upsert_points(&self, request: UpsertPoints) -> Result<PointsOperationResponse> {
        self.upsert_points(request).await.map_err(|e| SagittaError::QdrantError(e))
    }
    
    async fn create_collection(&self, collection_name: &str, vector_dimension: u64) -> Result<bool> {
        // Define dense vector parameters (naming it "dense")
        let dense_params = VectorParamsBuilder::new(vector_dimension, Distance::Cosine).build();
        let vectors_config = VectorsConfig { 
            config: Some(VectorsConfig_oneof_config::ParamsMap(VectorParamsMap {
                map: std::collections::HashMap::from([( 
                    "dense".to_string(), 
                    dense_params.into(),
                )])
            }))
        };

        // Define sparse vector parameters (naming it "sparse_tf" and enabling IDF)
        let sparse_params = SparseVectorParams {
            index: None, // Use default index for now
            modifier: Some(Modifier::Idf as i32),
        };
        let sparse_vectors_config = SparseVectorConfig {
            map: std::collections::HashMap::from([(
                "sparse_tf".to_string(), // Name the sparse vector
                sparse_params,
            )])
        };
        
        let request = CreateCollection {
            collection_name: collection_name.to_string(),
            vectors_config: Some(vectors_config),
            sparse_vectors_config: Some(sparse_vectors_config),
            // Add other parameters like hnsw_config, optimizers_config, etc. if needed
            ..Default::default()
        };
        
        let response = self.create_collection(request).await.map_err(|e| SagittaError::QdrantError(e))?;
        Ok(response.result)
    }
    
    async fn create_collection_detailed(&self, request: CreateCollection) -> Result<bool> {
        let response = self.create_collection(request).await.map_err(|e| SagittaError::QdrantError(e))?;
        Ok(response.result)
    }
    
    async fn delete_points(&self, request: DeletePoints) -> Result<PointsOperationResponse> {
        self.delete_points(request).await.map_err(SagittaError::from)
    }

    async fn query_points(&self, request: QueryPoints) -> Result<QueryResponse> {
        self.query_points(request).await.map_err(SagittaError::from)
    }

    async fn query(&self, request: QueryPoints) -> Result<QueryResponse> {
        self.query(request).await.map_err(SagittaError::from)
    }

    async fn list_collections(&self) -> Result<Vec<String>> {
        let response = self.list_collections().await.map_err(|e| SagittaError::QdrantError(e))?;
        let collection_names = response.collections
            .into_iter()
            .map(|collection| collection.name)
            .collect();
        Ok(collection_names)
    }
} 