use async_trait::async_trait;
use qdrant_client::qdrant::{
    DeleteCollection, HealthCheckReply, SearchPoints, CountPoints, GetCollectionInfoRequest, 
    CollectionInfo, CountResponse, SearchResponse, PointsSelector, DeletePoints,
    ScrollPoints, ScrollResponse,
    UpsertPoints, PointsOperationResponse, CreateCollection, Distance,
    VectorParamsBuilder, QueryPoints, QueryResponse
};
use qdrant_client::Qdrant;
// Import our custom error type instead
use crate::error::{VectorDBError, Result}; // Use crate::error

// Use mockall::automock for automatic mock generation in tests
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait QdrantClientTrait: Send + Sync {
    async fn health_check(&self) -> Result<HealthCheckReply>;
    async fn delete_collection(&self, collection_name: String) -> Result<bool>;
    async fn search_points(&self, request: SearchPoints) -> Result<SearchResponse>;
    async fn get_collection_info(&self, collection_name: String) -> Result<CollectionInfo>;
    async fn count(&self, request: CountPoints) -> Result<CountResponse>;
    async fn collection_exists(&self, collection_name: String) -> Result<bool>;
    async fn delete_points_blocking(&self, collection_name: &str, points_selector: &PointsSelector) -> Result<()>;
    async fn scroll(&self, request: ScrollPoints) -> Result<ScrollResponse>;
    async fn upsert_points(&self, request: UpsertPoints) -> Result<PointsOperationResponse>;
    async fn create_collection(&self, collection_name: &str, vector_dimension: u64) -> Result<bool>;
    async fn delete_points(&self, request: DeletePoints) -> Result<PointsOperationResponse>;
    async fn query_points(&self, request: QueryPoints) -> Result<QueryResponse>;
    async fn query(&self, request: QueryPoints) -> Result<QueryResponse>;
    // Add other methods used by the application as needed
}

#[async_trait]
impl QdrantClientTrait for Qdrant {
    async fn health_check(&self) -> Result<HealthCheckReply> {
        self.health_check().await.map_err(|e| VectorDBError::QdrantError(e))
    }

    async fn delete_collection(&self, collection_name: String) -> Result<bool> {
        let request = DeleteCollection {
            collection_name: collection_name,
            ..Default::default()
        };
        Ok(self.delete_collection(request).await.map_err(|e| VectorDBError::QdrantError(e))?.result)
    }

    async fn search_points(&self, request: SearchPoints) -> Result<SearchResponse> {
        self.search_points(request).await.map_err(|e| VectorDBError::QdrantError(e))
    }

    async fn get_collection_info(&self, collection_name: String) -> Result<CollectionInfo> {
        let request = GetCollectionInfoRequest {
            collection_name: collection_name.clone(),
        };
        let response = self.collection_info(request).await.map_err(|e| VectorDBError::QdrantError(e))?;
        match response.result {
            Some(info) => Ok(info),
            None => {
                let err_msg = format!("Collection info not found for '{}'", collection_name);
                Err(VectorDBError::RepositoryNotFound(err_msg))
            }
        }
    }

    async fn count(&self, request: CountPoints) -> Result<CountResponse> {
        self.count(request).await.map_err(|e| VectorDBError::QdrantError(e))
    }

    async fn collection_exists(&self, collection_name: String) -> Result<bool> {
        self.collection_exists(collection_name).await.map_err(|e| VectorDBError::QdrantError(e))
    }

    async fn delete_points_blocking(&self, collection_name: &str, points_selector: &PointsSelector) -> Result<()> {
        let request = DeletePoints {
            collection_name: collection_name.to_string(),
            wait: Some(true),
            points: Some(points_selector.clone()),
            ..Default::default()
        };
        self.delete_points(request).await.map_err(|e| VectorDBError::QdrantError(e))?;
        Ok(())
    }

    async fn scroll(&self, request: ScrollPoints) -> Result<ScrollResponse> {
        self.scroll(request).await.map_err(|e| VectorDBError::QdrantError(e))
    }

    async fn upsert_points(&self, request: UpsertPoints) -> Result<PointsOperationResponse> {
        self.upsert_points(request).await.map_err(|e| VectorDBError::QdrantError(e))
    }
    
    async fn create_collection(&self, collection_name: &str, vector_dimension: u64) -> Result<bool> {
        let vector_params = VectorParamsBuilder::new(vector_dimension, Distance::Cosine).build();
        
        let request = CreateCollection {
            collection_name: collection_name.to_string(),
            vectors_config: Some(vector_params.into()),
            // Add any other parameters needed
            ..Default::default()
        };
        
        let response = self.create_collection(request).await.map_err(|e| VectorDBError::QdrantError(e))?;
        Ok(response.result)
    }
    
    async fn delete_points(&self, request: DeletePoints) -> Result<PointsOperationResponse> {
        self.delete_points(request).await.map_err(VectorDBError::from)
    }

    async fn query_points(&self, request: QueryPoints) -> Result<QueryResponse> {
        self.query_points(request).await.map_err(VectorDBError::from)
    }

    async fn query(&self, request: QueryPoints) -> Result<QueryResponse> {
        self.query(request).await.map_err(VectorDBError::from)
    }
} 