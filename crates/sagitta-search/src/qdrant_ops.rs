/// Contains operations specific to Qdrant interactions, like deleting points.
use crate::{
    error::Result as CoreResult,
    qdrant_client_trait::QdrantClientTrait,
};
use anyhow::Result;
use qdrant_client::{
    qdrant::{
        PointStruct, UpsertPointsBuilder,
        UpdateStatus, 
        CreateFieldIndexCollectionBuilder, DeletePointsBuilder,
        FieldType, TokenizerType, payload_index_params::IndexParams,
        KeywordIndexParams, IntegerIndexParams, TextIndexParams,
        Filter, points_selector::PointsSelectorOneOf, 
        PointsOperationResponse, CollectionInfo,
    },
    Qdrant,
    
};
use std::sync::Arc;
use log;

/// Deletes all points from a specified Qdrant collection.
///
/// Uses an empty filter to select all points.
/// Waits for the operation to complete.
///
/// # Arguments
/// * `client` - An Arc-wrapped Qdrant client.
/// * `collection_name` - The name of the collection to clear.
///
/// # Returns
/// * `Result<UpdateResult>` - The result of the delete operation from Qdrant.
pub async fn delete_all_points<C>(
    client: Arc<C>,
    collection_name: &str,
) -> CoreResult<PointsOperationResponse>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    log::info!("Deleting all points from collection: {collection_name}");

    let select_all_filter = Filter::default();

    let delete_request = DeletePointsBuilder::new(collection_name)
        .points(PointsSelectorOneOf::Filter(select_all_filter));

    client.delete_points(delete_request.into()).await}

/// Deletes a Qdrant collection by its name.
///
/// # Arguments
/// * `client` - An Arc-wrapped Qdrant client.
/// * `collection_name` - The name of the collection to delete.
///
/// # Returns
/// * `CoreResult<()>` - Ok if successful, or an error.
pub async fn delete_collection_by_name<C>(
    client: Arc<C>,
    collection_name: &str,
) -> CoreResult<()>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    log::info!("Deleting collection: {collection_name}");
    match client.delete_collection(collection_name.to_string()).await {
        Ok(success) => { // `success` is the boolean result from the QdrantClientTrait
            if success { // `success` is used directly as the boolean
                log::info!("Successfully deleted collection: {collection_name}");
            } else {
                log::warn!(
                    "Delete collection operation for '{collection_name}' acknowledged by Qdrant but result was false (e.g., collection might not have existed)."
                );
            }
            Ok(())
        }
        Err(core_err) => { // The trait method returns CoreResult (Result<T, SagittaError>)
            log::error!("Qdrant client error while deleting collection '{collection_name}': {core_err:?}");
            Err(core_err) // Propagate the SagittaError
        }
    }
}

/// Helper to ensure a specific payload field index exists in Qdrant.
#[allow(clippy::too_many_arguments)]
pub async fn ensure_payload_index(
    client: &Qdrant,
    collection_name: &str,
    field_name: &str,
    field_type: FieldType,
    tokenizer: Option<TokenizerType>,
) -> Result<()> {
    let get_info_result = client.get_collection_info(collection_name.to_string()).await;

    let collection_info: CollectionInfo = match get_info_result {
        Ok(info) => info,
        Err(e) => {
            return Err(anyhow::anyhow!(e).context(format!(
                "Failed to get collection info for {collection_name}"
            )));
        }
    };

    if collection_info.payload_schema.contains_key(field_name) {
        log::debug!(
            "Payload index for '{collection_name}' on field '{field_name}' already exists."
        );
        return Ok(());
    }

    log::info!(
        "Creating payload index for '{collection_name}' on field '{field_name}'..."
    );

    let index_params = match field_type {
        FieldType::Keyword => Some(IndexParams::KeywordIndexParams(KeywordIndexParams {
            on_disk: None,
            is_tenant: Some(false),
        })),
        FieldType::Integer => Some(IndexParams::IntegerIndexParams(IntegerIndexParams {
            lookup: Some(false),
            range: Some(false),
            is_principal: Some(false),
            on_disk: None,
        })),
        FieldType::Text => Some(IndexParams::TextIndexParams(TextIndexParams {
            tokenizer: tokenizer.map(|t| t as i32).unwrap_or_default(),
            lowercase: Some(true),
            min_token_len: None,
            max_token_len: None,
            on_disk: None,
        })),
        _ => None,
    };

    let mut builder = CreateFieldIndexCollectionBuilder::new(collection_name, field_name, field_type);

    if let Some(params) = index_params {
        builder = builder.field_index_params(params);
    }

    match client.create_field_index(builder).await {
        Ok(response) => {
            if let Some(result) = response.result {
                 match UpdateStatus::try_from(result.status) {
                     Ok(UpdateStatus::Completed) => {
                         log::info!("Payload index created successfully for field '{field_name}'.");
                         Ok(())
                     }
                     Ok(status) => {
                         log::warn!("Payload index creation for field '{field_name}' resulted in status: {status:?}");
                         Ok(())
                     }
                     Err(_) => {
                         log::warn!("Payload index creation for field '{}' returned unknown status code: {}", field_name, result.status);
                         Ok(())
                     }
                 }
             } else {
                  log::warn!("Payload index creation response for field '{field_name}' did not contain a result.");
                  Ok(())
             }
        }
        Err(e) => {
            log::error!("Failed to create payload index for field '{field_name}': {e}. Ignoring error.");
            Ok(())
        }
    }
}

/// Upserts a batch of points into a Qdrant collection.
pub async fn upsert_batch<
    C: QdrantClientTrait
>(
    client: Arc<C>,
    collection_name: &str,
    points: Vec<PointStruct>,
) -> Result<()> {
    if points.is_empty() {
        return Ok(());
    }
    log::debug!("Upserting batch of {} points to \"{}\"", points.len(), collection_name);

    let upsert_builder = UpsertPointsBuilder::new(collection_name, points);
    client.upsert_points(upsert_builder.into()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qdrant_client_trait::QdrantClientTrait;
    use crate::error::{SagittaError, Result as CoreResult};
    use async_trait::async_trait;
    use qdrant_client::qdrant::{
        CreateCollection, DeletePoints, UpsertPoints,
        PointsOperationResponse, UpdateResult, UpdateStatus,
        HealthCheckReply, SearchPoints, SearchResponse, CountPoints, CountResponse,
        PointsSelector, ScrollPoints, ScrollResponse, QueryPoints, QueryResponse,
    };
    use std::sync::Mutex;
    use std::collections::HashMap;

    #[derive(Default)]
    struct MockQdrantClient {
        delete_points_called: Mutex<bool>,
        delete_collection_called: Mutex<bool>,
        upsert_points_called: Mutex<bool>,
        collection_exists: Mutex<bool>,
        should_fail: Mutex<bool>,
    }

    #[async_trait]
    impl QdrantClientTrait for MockQdrantClient {
        async fn health_check(&self) -> CoreResult<HealthCheckReply> {
            Ok(HealthCheckReply::default())
        }

        async fn delete_collection(&self, _name: String) -> CoreResult<bool> {
            *self.delete_collection_called.lock().unwrap() = true;
            if *self.should_fail.lock().unwrap() {
                Err(SagittaError::Other("Mock delete failed".to_string()))
            } else {
                Ok(true)
            }
        }

        async fn search_points(&self, _request: SearchPoints) -> CoreResult<SearchResponse> {
            Ok(SearchResponse::default())
        }

        async fn get_collection_info(&self, _name: String) -> CoreResult<CollectionInfo> {
            Ok(CollectionInfo::default())
        }

        async fn count(&self, _request: CountPoints) -> CoreResult<CountResponse> {
            Ok(CountResponse::default())
        }

        async fn collection_exists(&self, _name: String) -> CoreResult<bool> {
            Ok(*self.collection_exists.lock().unwrap())
        }

        async fn delete_points_blocking(&self, _collection_name: &str, _points_selector: &PointsSelector) -> CoreResult<()> {
            Ok(())
        }

        async fn scroll(&self, _request: ScrollPoints) -> CoreResult<ScrollResponse> {
            Ok(ScrollResponse::default())
        }

        async fn upsert_points(&self, _request: UpsertPoints) -> CoreResult<PointsOperationResponse> {
            *self.upsert_points_called.lock().unwrap() = true;
            if *self.should_fail.lock().unwrap() {
                Err(SagittaError::Other("Mock upsert failed".to_string()))
            } else {
                Ok(PointsOperationResponse {
                    result: Some(UpdateResult {
                        operation_id: Some(1),
                        status: UpdateStatus::Completed as i32,
                    }),
                    time: 0.1,
                    usage: None,
                })
            }
        }

        async fn create_collection(&self, _collection_name: &str, _vector_dimension: u64) -> CoreResult<bool> {
            Ok(true)
        }

        async fn create_collection_detailed(&self, _request: CreateCollection) -> CoreResult<bool> {
            Ok(true)
        }

        async fn delete_points(&self, _request: DeletePoints) -> CoreResult<PointsOperationResponse> {
            *self.delete_points_called.lock().unwrap() = true;
            if *self.should_fail.lock().unwrap() {
                Err(SagittaError::Other("Mock delete points failed".to_string()))
            } else {
                Ok(PointsOperationResponse {
                    result: Some(UpdateResult {
                        operation_id: Some(1),
                        status: UpdateStatus::Completed as i32,
                    }),
                    time: 0.1,
                    usage: None,
                })
            }
        }

        async fn query_points(&self, _request: QueryPoints) -> CoreResult<QueryResponse> {
            Ok(QueryResponse::default())
        }

        async fn query(&self, _request: QueryPoints) -> CoreResult<QueryResponse> {
            Ok(QueryResponse::default())
        }

        async fn list_collections(&self) -> CoreResult<Vec<String>> {
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn test_delete_all_points() {
        let client = Arc::new(MockQdrantClient::default());
        let result = delete_all_points(client.clone(), "test_collection").await;
        
        assert!(result.is_ok());
        assert!(*client.delete_points_called.lock().unwrap());
    }

    #[tokio::test]
    async fn test_delete_all_points_error() {
        let client = Arc::new(MockQdrantClient {
            should_fail: Mutex::new(true),
            ..Default::default()
        });
        
        let result = delete_all_points(client.clone(), "test_collection").await;
        
        assert!(result.is_err());
        assert!(*client.delete_points_called.lock().unwrap());
    }

    #[tokio::test]
    async fn test_delete_collection_by_name_success() {
        let client = Arc::new(MockQdrantClient::default());
        let result = delete_collection_by_name(client.clone(), "test_collection").await;
        
        assert!(result.is_ok());
        assert!(*client.delete_collection_called.lock().unwrap());
    }

    #[tokio::test]
    async fn test_delete_collection_by_name_error() {
        let client = Arc::new(MockQdrantClient {
            should_fail: Mutex::new(true),
            ..Default::default()
        });
        
        let result = delete_collection_by_name(client.clone(), "test_collection").await;
        
        assert!(result.is_err());
        assert!(*client.delete_collection_called.lock().unwrap());
    }

    #[tokio::test]
    async fn test_upsert_batch_empty() {
        let client = Arc::new(MockQdrantClient::default());
        let result = upsert_batch(client.clone(), "test_collection", vec![]).await;
        
        assert!(result.is_ok());
        assert!(!*client.upsert_points_called.lock().unwrap()); // Should not be called for empty batch
    }

    #[tokio::test]
    async fn test_upsert_batch_with_points() {
        let client = Arc::new(MockQdrantClient::default());
        
        let points = vec![
            PointStruct {
                id: Some(1.into()),
                vectors: None,
                payload: HashMap::new(),
            },
            PointStruct {
                id: Some(2.into()),
                vectors: None,
                payload: HashMap::new(),
            },
        ];
        
        let result = upsert_batch(client.clone(), "test_collection", points).await;
        
        assert!(result.is_ok());
        assert!(*client.upsert_points_called.lock().unwrap());
    }

    #[tokio::test]
    async fn test_upsert_batch_error() {
        let client = Arc::new(MockQdrantClient {
            should_fail: Mutex::new(true),
            ..Default::default()
        });
        
        let points = vec![
            PointStruct {
                id: Some(1.into()),
                vectors: None,
                payload: HashMap::new(),
            },
        ];
        
        let result = upsert_batch(client.clone(), "test_collection", points).await;
        
        assert!(result.is_err());
        assert!(*client.upsert_points_called.lock().unwrap());
    }
} 