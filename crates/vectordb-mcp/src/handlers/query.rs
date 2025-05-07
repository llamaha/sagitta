use crate::mcp::{
    error_codes,
    types::{ErrorObject, QueryParams, QueryResult, SearchResultItem},
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, instrument, warn};
use vectordb_core::{
    config::AppConfig,
    constants::{
        FIELD_BRANCH, FIELD_CHUNK_CONTENT, FIELD_END_LINE, FIELD_FILE_PATH, FIELD_START_LINE,
    },
    embedding::EmbeddingHandler,
    error::VectorDBError,
    qdrant_client_trait::QdrantClientTrait,
    repo_helpers::get_collection_name,
    search_collection,
};
use qdrant_client::qdrant::{value::Kind, Condition, Filter};
use anyhow::Result;

#[instrument(skip(config, qdrant_client, embedding_handler), fields(repo_name = %params.repository_name, query = %params.query_text))]
pub async fn handle_query<C: QdrantClientTrait + Send + Sync + 'static>(
    params: QueryParams,
    config: Arc<RwLock<AppConfig>>,
    qdrant_client: Arc<C>,
    embedding_handler: Arc<EmbeddingHandler>,
) -> Result<QueryResult, ErrorObject> {
    let config_read_guard = config.read().await;

    let repo_config = config_read_guard
        .repositories
        .iter()
        .find(|r| r.name == params.repository_name)
        .ok_or_else(|| ErrorObject {
            code: error_codes::REPO_NOT_FOUND,
            message: format!("Repository '{}' not found", params.repository_name),
            data: None,
        })?;

    let branch_name = params.branch_name.as_ref()
        .or(repo_config.active_branch.as_ref())
        .ok_or_else(|| ErrorObject {
            code: error_codes::INVALID_QUERY_PARAMS,
            message: format!("Cannot determine branch for repository '{}'. No branch specified and no active branch set.", params.repository_name),
            data: None,
        })?;

    let collection_name = get_collection_name(&params.repository_name, &config_read_guard);
    let query_text = params.query_text;
    let limit = params.limit;

    info!(collection=%collection_name, branch=%branch_name, limit=%limit, "Preparing query");

    let mut filter_conditions = vec![Condition::matches(
        FIELD_BRANCH,
        branch_name.to_string(),
    )];
    if let Some(ref element_type) = params.element_type {
        filter_conditions.push(Condition::matches(
            vectordb_core::constants::FIELD_ELEMENT_TYPE,
            element_type.clone(),
        ));
    }
    if let Some(ref lang) = params.lang {
        filter_conditions.push(Condition::matches(
            vectordb_core::constants::FIELD_LANGUAGE,
            lang.clone(),
        ));
    }
    let filter = Some(Filter::must(filter_conditions));
    
    let search_response = search_collection(
        qdrant_client,
        &collection_name,
        &embedding_handler,
        &query_text,
        limit,
        filter,
        &config_read_guard,
    )
    .await
    .map_err(|e| {
        error!(error = %e, collection=%collection_name, "Core search failed");
        match e {
            VectorDBError::EmbeddingError(_) => ErrorObject {
                code: error_codes::EMBEDDING_ERROR,
                message: format!("Failed to generate embedding for query: {}", e),
                data: None,
            },
            _ => ErrorObject {
                code: error_codes::QUERY_EXECUTION_FAILED,
                message: format!("Failed to execute query: {}", e),
                data: None,
            },
        }
    })?;

    let mut results: Vec<SearchResultItem> = Vec::new();
    for scored_point in search_response.result {
        let payload = scored_point.payload;

        let file_path = payload.get(FIELD_FILE_PATH)
            .and_then(|v| v.kind.as_ref())
            .and_then(|k| if let Kind::StringValue(s) = k { Some(s.clone()) } else { None })
            .unwrap_or_else(|| { warn!(point_id=?scored_point.id, "Missing file_path in payload"); String::from("<unknown>") });

        let start_line = payload.get(FIELD_START_LINE)
            .and_then(|v| v.kind.as_ref())
            .and_then(|k| if let Kind::IntegerValue(i) = k { usize::try_from(*i).ok() } else { None })
            .unwrap_or_else(|| { warn!(point_id=?scored_point.id, "Missing or invalid start_line in payload"); 0usize });

        let end_line = payload.get(FIELD_END_LINE)
            .and_then(|v| v.kind.as_ref())
            .and_then(|k| if let Kind::IntegerValue(i) = k { usize::try_from(*i).ok() } else { None })
            .unwrap_or_else(|| { warn!(point_id=?scored_point.id, "Missing or invalid end_line in payload"); 0usize });

        let content = payload.get(FIELD_CHUNK_CONTENT)
            .and_then(|v| v.kind.as_ref())
            .and_then(|k| if let Kind::StringValue(s) = k { Some(s.clone()) } else { None })
            .unwrap_or_else(|| { warn!(point_id=?scored_point.id, "Missing or invalid content in payload"); "<content missing>".to_string() });

        results.push(SearchResultItem {
            file_path,
            start_line,
            end_line: end_line + 1,
            score: scored_point.score,
            content,
        });
    }

    info!(count = results.len(), "Returning query results");
    Ok(QueryResult { results })
} 