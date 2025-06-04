use crate::mcp::{
    error_codes,
    types::{ErrorObject, QueryParams, QueryResult, SearchResultItem},
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, instrument, warn};
use sagitta_search::{
    config::AppConfig,
    constants::{
        FIELD_BRANCH, FIELD_CHUNK_CONTENT, FIELD_END_LINE, FIELD_FILE_PATH, FIELD_START_LINE,
    },
    EmbeddingPool, EmbeddingProcessor,
    app_config_to_embedding_config,
    error::SagittaError,
    qdrant_client_trait::QdrantClientTrait,
    repo_helpers::{get_collection_name, get_branch_aware_collection_name},
    search_impl::search_collection,
};
use qdrant_client::qdrant::{value::Kind, Condition, Filter};
use anyhow::Result;
use axum::Extension;
use crate::middleware::auth_middleware::AuthenticatedUser;

#[instrument(skip(config, qdrant_client, auth_user_ext), fields(repo_name = %params.repository_name, query = %params.query_text))]
pub async fn handle_query<C: QdrantClientTrait + Send + Sync + 'static>(
    params: QueryParams,
    config: Arc<RwLock<AppConfig>>,
    qdrant_client: Arc<C>,
    auth_user_ext: Option<Extension<AuthenticatedUser>>,
) -> Result<QueryResult, ErrorObject> {
    let query_text = params.query_text.clone();
    let limit = params.limit;
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

    // Tenant isolation check: Determine acting_tenant_id
    let acting_tenant_id: Option<String> = if let Some(auth_user) = auth_user_ext.as_ref() {
        info!(tenant_source = "AuthenticatedUser", tenant_id = %auth_user.0.tenant_id, repo_name = %params.repository_name);
        Some(auth_user.0.tenant_id.clone())
    } else if let Some(default_tenant_id) = config_read_guard.tenant_id.as_ref() {
        info!(tenant_source = "ServerConfigDefault", tenant_id = %default_tenant_id, repo_name = %params.repository_name);
        Some(default_tenant_id.clone())
    } else {
        info!(tenant_source = "None", repo_name = %params.repository_name, "No acting tenant ID determined (no auth, no server default) for query.");
        None
    };

    // Perform tenant check and get the tenant_id to use for the collection
    let tenant_id_for_collection_str: String = match (&acting_tenant_id, &repo_config.tenant_id) {
        (Some(act_tid), Some(repo_tid)) => {
            if act_tid == repo_tid {
                info!(repo_name = %params.repository_name, acting_tenant_id = %act_tid, "Tenant ID match successful for query.");
                repo_tid.clone() // Use this tenant ID for the collection
            } else {
                warn!(
                    acting_tenant_id = %act_tid,
                    repo_tenant_id = %repo_tid,
                    repo_name = %params.repository_name,
                    "Access denied: Acting tenant ID does not match repository's tenant ID for query."
                );
                return Err(ErrorObject {
                    code: error_codes::ACCESS_DENIED,
                    message: "Access denied: Tenant ID mismatch for query operation.".to_string(),
                    data: None,
                });
            }
        }
        _ => { // All other cases: (None, Some), (Some, None), (None, None) -> Deny
            warn!(
                acting_tenant_id = ?acting_tenant_id,
                repo_tenant_id = ?repo_config.tenant_id,
                repo_name = %params.repository_name,
                "Access denied: Tenant ID mismatch or missing for query. Both acting context and repository must have a matching, defined tenant ID."
            );
            return Err(ErrorObject {
                code: error_codes::ACCESS_DENIED,
                message: "Access denied: Query requires matching and defined tenant IDs for both context and repository.".to_string(),
                data: None,
            });
        }
    };

    let branch_name = params.branch_name.as_ref()
        .or(repo_config.active_branch.as_ref())
        .ok_or_else(|| ErrorObject {
            code: error_codes::INVALID_QUERY_PARAMS,
            message: format!("Cannot determine branch for repository '{}'. No branch specified and no active branch set.", params.repository_name),
            data: None,
        })?;

    // Use tenant_id_for_collection_str which is confirmed String
    let collection_name = get_collection_name(&tenant_id_for_collection_str, &params.repository_name, &config_read_guard);

    info!(
        collection=%collection_name,
        branch=%branch_name,
        limit=%limit,
        "Handling query for repo: {}, branch: {:?}, query: '{}', limit: {}",
        params.repository_name,
        params.branch_name,
        query_text,
        limit
    );

    let mut filter_conditions = vec![Condition::matches(
        FIELD_BRANCH,
        branch_name.to_string(),
    )];
    if let Some(ref element_type) = params.element_type {
        filter_conditions.push(Condition::matches(
            sagitta_search::constants::FIELD_ELEMENT_TYPE,
            element_type.to_string(),
        ));
    }
    if let Some(ref lang) = params.lang {
        filter_conditions.push(Condition::matches(
            sagitta_search::constants::FIELD_LANGUAGE,
            lang.to_string(),
        ));
    }
    let filter = Some(Filter::must(filter_conditions));
    
    // Create EmbeddingPool instance locally for this operation
    let embedding_config = sagitta_search::app_config_to_embedding_config(&config_read_guard);
    let embedding_pool = EmbeddingPool::with_configured_sessions(embedding_config).map_err(|e| {
        error!(error = %e, "Failed to create embedding pool for query");
        ErrorObject {
            code: error_codes::INTERNAL_ERROR,
            message: format!("Failed to initialize embedding pool: {}", e),
            data: None,
        }
    })?;

    let search_response = search_collection(
        qdrant_client,
        &collection_name,
        &embedding_pool,
        &query_text,
        limit,
        filter,
        &config_read_guard,
    )
    .await
    .map_err(|e| {
        error!(error = %e, collection=%collection_name, "Core search failed");
        match e {
            SagittaError::EmbeddingError(_) => ErrorObject {
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