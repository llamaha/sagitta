/// Contains operations specific to Qdrant interactions, like deleting points.
use crate::{
    error::{Result as CoreResult, VectorDBError},
    qdrant_client_trait::QdrantClientTrait,
};
use anyhow::{Context, Result};
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
    log::info!("Deleting all points from collection: {}", collection_name);

    let select_all_filter = Filter::default();

    let delete_request = DeletePointsBuilder::new(collection_name)
        .points(PointsSelectorOneOf::Filter(select_all_filter));

    client.delete_points(delete_request.into()).await.map_err(VectorDBError::from)
}

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
    log::info!("Deleting collection: {}", collection_name);
    match client.delete_collection(collection_name.to_string()).await {
        Ok(success) => { // `success` is the boolean result from the QdrantClientTrait
            if success { // `success` is used directly as the boolean
                log::info!("Successfully deleted collection: {}", collection_name);
            } else {
                log::warn!(
                    "Delete collection operation for '{}' acknowledged by Qdrant but result was false (e.g., collection might not have existed).",
                    collection_name
                );
            }
            Ok(())
        }
        Err(core_err) => { // The trait method returns CoreResult (Result<T, VectorDBError>)
            log::error!("Qdrant client error while deleting collection '{}': {:?}", collection_name, core_err);
            Err(core_err) // Propagate the VectorDBError
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
                "Failed to get collection info for {}",
                collection_name
            )));
        }
    };

    if collection_info.payload_schema.contains_key(field_name) {
        log::debug!(
            "Payload index for '{}' on field '{}' already exists.",
            collection_name,
            field_name
        );
        return Ok(());
    }

    log::info!(
        "Creating payload index for '{}' on field '{}'...",
        collection_name,
        field_name
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
                         log::info!("Payload index created successfully for field '{}'.", field_name);
                         Ok(())
                     }
                     Ok(status) => {
                         log::warn!("Payload index creation for field '{}' resulted in status: {:?}", field_name, status);
                         Ok(())
                     }
                     Err(_) => {
                         log::warn!("Payload index creation for field '{}' returned unknown status code: {}", field_name, result.status);
                         Ok(())
                     }
                 }
             } else {
                  log::warn!("Payload index creation response for field '{}' did not contain a result.", field_name);
                  Ok(())
             }
        }
        Err(e) => {
            log::error!("Failed to create payload index for field '{}': {}. Ignoring error.", field_name, e);
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
    // TODO: Add tests for delete_all_points
    // Need mock QdrantClientTrait
} 