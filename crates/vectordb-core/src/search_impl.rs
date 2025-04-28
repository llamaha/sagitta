use crate::{
    embedding::EmbeddingHandler,
    error::{Result, VectorDBError},
    qdrant_client_trait::QdrantClientTrait,
};
use qdrant_client::{
    qdrant::{Filter, SearchPoints, SearchPointsBuilder, SearchResponse},
    Qdrant, // Use concrete type if trait causes issues, or keep trait
};
use std::sync::Arc;


/// Performs a vector search in a specified Qdrant collection.
///
/// # Arguments
/// * `client` - An Arc-wrapped Qdrant client (or trait object).
/// * `collection_name` - The name of the collection to search.
/// * `embedding_handler` - Handler to generate the query embedding.
/// * `query_text` - The text to search for.
/// * `limit` - The maximum number of results to return.
/// * `filter` - An optional Qdrant filter to apply.
///
/// # Returns
/// * `Result<SearchResponse>` - The search results from Qdrant.
pub async fn search_collection<C>(
    client: Arc<C>,
    collection_name: &str,
    embedding_handler: &EmbeddingHandler,
    query_text: &str,
    limit: u64,
    filter: Option<Filter>,
) -> Result<SearchResponse>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    log::debug!(
        "Core: Searching collection \"{}\" for query: \"{}\" with limit {} and filter: {:?}",
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

    // 2. Build search request
    let mut search_builder = SearchPointsBuilder::new(collection_name, query_embedding, limit)
        .with_payload(true); // Always include payload for context

    if let Some(f) = filter {
        search_builder = search_builder.filter(f);
        log::trace!("Core: Applied search filter.");
    }

    let search_request: SearchPoints = search_builder.into();

    // 3. Perform search
    log::debug!("Core: Executing search request...");
    let search_response = client.search_points(search_request).await?;
    log::info!("Found {} search results.", search_response.result.len());
    Ok(search_response)
}

// Potential future function specifically for repositories?
// pub async fn search_repository(...) -> Result<SearchResponse> {
//     // Might involve looking up collection name, default branch etc.
//     // Calls search_collection internally
// }

#[cfg(test)]
mod tests {
    // TODO: Add tests for search_collection
    // Need mock EmbeddingHandler and mock QdrantClientTrait
}
