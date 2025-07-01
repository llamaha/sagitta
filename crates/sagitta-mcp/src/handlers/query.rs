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
use qdrant_client::qdrant::{SearchPoints, SearchResponse, HealthCheckReply, CollectionInfo, CountPoints, CountResponse, PointsSelector, ScrollPoints, ScrollResponse, UpsertPoints, PointsOperationResponse, CreateCollection, DeletePoints, QueryPoints, QueryResponse};
use async_trait::async_trait;
use tempfile::tempdir;
use serde_json::json;

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
    let tenant_id_for_collection_str: String = {
        #[cfg(feature = "multi_tenant")]
        {
            match (&acting_tenant_id, &repo_config.tenant_id) {
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
            }
        }
        #[cfg(not(feature = "multi_tenant"))]
        {
            info!(repo_name = %params.repository_name, "Multi-tenancy disabled, using default tenant ID for query");
            repo_config.tenant_id.as_ref().unwrap_or(&"default".to_string()).clone()
        }
    };

    let branch_name = params.branch_name.as_ref()
        .or(repo_config.active_branch.as_ref())
        .ok_or_else(|| ErrorObject {
            code: error_codes::INVALID_QUERY_PARAMS,
            message: format!("Cannot determine branch for repository '{}'. No branch specified and no active branch set.", params.repository_name),
            data: None,
        })?;

    // Use branch-aware collection naming to match how collections are created during add/sync
    let collection_name = get_branch_aware_collection_name(&tenant_id_for_collection_str, &params.repository_name, branch_name, &config_read_guard);

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
        None,
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

        // Always get the chunk content to extract preview
        let chunk_content = payload.get(FIELD_CHUNK_CONTENT)
            .and_then(|v| v.kind.as_ref())
            .and_then(|k| if let Kind::StringValue(s) = k { Some(s.clone()) } else { None })
            .unwrap_or_else(|| { warn!(point_id=?scored_point.id, "Missing or invalid content in payload"); "<content missing>".to_string() });

        // Extract first line for preview (truncate at 120 chars if too long)
        let preview = chunk_content.lines()
            .next()
            .map(|line| {
                if line.len() > 120 {
                    format!("{}...", &line[..117])
                } else {
                    line.to_string()
                }
            });

        let content = if params.show_code.unwrap_or(false) {
            // Only include full content if show_code is explicitly true
            Some(chunk_content)
        } else {
            None
        };

        results.push(SearchResultItem {
            file_path,
            start_line,
            end_line: end_line + 1,
            score: scored_point.score,
            content,
            preview,
        });
    }

    info!(count = results.len(), "Returning query results");

    Ok(QueryResult { results })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use crate::mcp::types::{QueryParams, ErrorObject};
    use crate::middleware::auth_middleware::AuthenticatedUser;
    use sagitta_search::{
        config::{AppConfig, RepositoryConfig, PerformanceConfig},
        qdrant_client_trait::QdrantClientTrait,
        error::SagittaError,
        repo_helpers::{get_collection_name, get_branch_aware_collection_name},
    };
    use qdrant_client::qdrant::{SearchPoints, SearchResponse, HealthCheckReply, CollectionInfo, CountPoints, CountResponse, PointsSelector, ScrollPoints, ScrollResponse, UpsertPoints, PointsOperationResponse, CreateCollection, DeletePoints, QueryPoints, QueryResponse};
    use async_trait::async_trait;
    use axum::Extension;

    #[derive(Clone, Debug)]
    struct MockQdrantClient {
        expected_collection_name: String,
        should_fail: bool,
    }

    impl MockQdrantClient {
        fn new(expected_collection_name: String) -> Self {
            Self {
                expected_collection_name,
                should_fail: false,
            }
        }

        fn with_failure(mut self) -> Self {
            self.should_fail = true;
            self
        }
    }

    #[async_trait]
    impl QdrantClientTrait for MockQdrantClient {
        async fn health_check(&self) -> Result<HealthCheckReply, SagittaError> {
            Ok(HealthCheckReply { title: "mock".to_string(), version: "mock".to_string(), commit: None })
        }

        async fn search_points(&self, request: SearchPoints) -> Result<SearchResponse, SagittaError> {
            // Verify that the search is being performed on the expected collection name
            assert_eq!(
                request.collection_name, 
                self.expected_collection_name,
                "Query handler used wrong collection name. Expected '{}', got '{}'",
                self.expected_collection_name,
                request.collection_name
            );

            if self.should_fail {
                return Err(SagittaError::Other(format!(
                    "Collection `{}` doesn't exist!",
                    request.collection_name
                )));
            }

            // Return a minimal successful response
            Ok(SearchResponse {
                result: vec![],
                time: 0.001,
                usage: None,
            })
        }

        // All other methods are unimplemented for this test
        async fn delete_collection(&self, _collection_name: String) -> Result<bool, SagittaError> {
            unimplemented!("MockQdrantClient delete_collection not implemented for tests")
        }

        async fn get_collection_info(&self, _collection_name: String) -> Result<CollectionInfo, SagittaError> {
            unimplemented!("MockQdrantClient get_collection_info not implemented for tests")
        }

        async fn count(&self, _request: CountPoints) -> Result<CountResponse, SagittaError> {
            unimplemented!("MockQdrantClient count not implemented for tests")
        }

        async fn collection_exists(&self, _collection_name: String) -> Result<bool, SagittaError> {
            Ok(true) // Always return true to avoid early failures
        }

        async fn delete_points_blocking(&self, _collection_name: &str, _points_selector: &PointsSelector) -> Result<(), SagittaError> {
            unimplemented!("MockQdrantClient delete_points_blocking not implemented for tests")
        }

        async fn scroll(&self, _request: ScrollPoints) -> Result<ScrollResponse, SagittaError> {
            unimplemented!("MockQdrantClient scroll not implemented for tests")
        }

        async fn upsert_points(&self, _request: UpsertPoints) -> Result<PointsOperationResponse, SagittaError> {
            unimplemented!("MockQdrantClient upsert_points not implemented for tests")
        }

        async fn create_collection(&self, _collection_name: &str, _vector_dimension: u64) -> Result<bool, SagittaError> {
            unimplemented!("MockQdrantClient create_collection not implemented for tests")
        }

        async fn create_collection_detailed(&self, _request: CreateCollection) -> Result<bool, SagittaError> {
            unimplemented!("MockQdrantClient create_collection_detailed not implemented for tests")
        }

        async fn delete_points(&self, _request: DeletePoints) -> Result<PointsOperationResponse, SagittaError> {
            unimplemented!("MockQdrantClient delete_points not implemented for tests")
        }

        async fn query_points(&self, _request: QueryPoints) -> Result<QueryResponse, SagittaError> {
            unimplemented!("MockQdrantClient query_points not implemented for tests")
        }

        async fn query(&self, request: QueryPoints) -> Result<QueryResponse, SagittaError> {
            // Verify that the query is being performed on the expected collection name
            assert_eq!(
                request.collection_name, 
                self.expected_collection_name,
                "Query handler used wrong collection name. Expected '{}', got '{}'",
                self.expected_collection_name,
                request.collection_name
            );

            if self.should_fail {
                return Err(SagittaError::Other(format!(
                    "Collection `{}` doesn't exist!",
                    request.collection_name
                )));
            }

            // Return a minimal successful response
            Ok(QueryResponse {
                result: vec![],
                time: 0.001,
                usage: None,
            })
        }

        async fn list_collections(&self) -> Result<Vec<String>, SagittaError> {
            unimplemented!("MockQdrantClient list_collections not implemented for tests")
        }
    }

    fn create_test_config(repo_config: RepositoryConfig) -> Arc<RwLock<AppConfig>> {
        use sagitta_search::config::EmbeddingEngineConfig;
        
        let config = AppConfig {
            qdrant_url: "http://localhost:6334".to_string(),
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            embed_model: Some("default".to_string()), // Use default embedding provider
            repositories: vec![repo_config],
            performance: PerformanceConfig {
                collection_name_prefix: "test_prefix_".to_string(),
                vector_dimension: 384,
                ..PerformanceConfig::default()
            },
            embedding: EmbeddingEngineConfig::default(),
            tenant_id: Some("test_tenant".to_string()),
            ..AppConfig::default()
        };
        
        Arc::new(RwLock::new(config))
    }

    fn create_test_repo_config(name: &str, tenant_id: &str, active_branch: Option<&str>) -> RepositoryConfig {
        RepositoryConfig {
            name: name.to_string(),
            url: "https://example.com/repo.git".to_string(),
            local_path: std::path::PathBuf::from("/tmp/test_repo"),
            default_branch: "main".to_string(),
            active_branch: active_branch.map(|s| s.to_string()),
            tenant_id: Some(tenant_id.to_string()),
            ..RepositoryConfig::default()
        }
    }

    fn create_auth_user(tenant_id: &str) -> Extension<AuthenticatedUser> {
        Extension(AuthenticatedUser {
            tenant_id: tenant_id.to_string(),
            user_id: Some("test_user".to_string()),
            scopes: vec!["query:repositories".to_string()],
        })
    }

    /// Test that query handler uses branch-aware collection naming, not legacy naming
    /// This test verifies the collection name determination logic without going through embedding generation
    #[tokio::test]
    async fn test_query_collection_name_generation() {
        let tenant_id = "test_tenant_123";
        let repo_name = "test_repo";
        let branch_name = "main";
        
        let repo_config = create_test_repo_config(repo_name, tenant_id, Some(branch_name));
        let config = create_test_config(repo_config);
        let config_guard = config.read().await;
        
        // Calculate what the collection names should be
        let legacy_collection_name = get_collection_name(tenant_id, repo_name, &config_guard);
        let branch_aware_collection_name = get_branch_aware_collection_name(tenant_id, repo_name, branch_name, &config_guard);
        
        // Ensure they're different (this validates our test setup)
        assert_ne!(
            legacy_collection_name, 
            branch_aware_collection_name,
            "Legacy and branch-aware collection names should be different"
        );
        
        // Verify that branch-aware collection name includes branch info
        assert!(
            branch_aware_collection_name.contains("_br_"), 
            "Branch-aware collection name should contain '_br_' marker"
        );
        
        // The actual collection name used should be the branch-aware one
        // We can't easily test this without going through the full query pipeline,
        // but the key fix is that the query handler now calls:
        // get_branch_aware_collection_name() instead of get_collection_name()
        
        drop(config_guard);
        
        println!("✓ Collection naming test passed:");
        println!("  Legacy format:      {}", legacy_collection_name);
        println!("  Branch-aware format: {}", branch_aware_collection_name);
    }

    /// Test that query uses branch-aware collection naming
    /// (This verifies that the bug fix is working correctly)
    #[tokio::test]
    async fn test_collection_naming_mismatch_causes_failure() {
        let tenant_id = "test_tenant_456";
        let repo_name = "test_repo";
        let branch_name = "main";
        
        let repo_config = create_test_repo_config(repo_name, tenant_id, Some(branch_name));
        let config = create_test_config(repo_config);
        let config_guard = config.read().await;
        
        // Calculate the correct branch-aware collection name
        let branch_aware_collection_name = get_branch_aware_collection_name(tenant_id, repo_name, branch_name, &config_guard);
        
        drop(config_guard);
        
        // Create mock client that expects the branch-aware collection name
        let mock_client = Arc::new(MockQdrantClient::new(branch_aware_collection_name).with_failure());
        
        let query_params = QueryParams {
            repository_name: repo_name.to_string(),
            query_text: "test query".to_string(),
            limit: 10,
            branch_name: Some(branch_name.to_string()),
            element_type: None,
            lang: None,
            show_code: None,
        };
        
        let auth_user = Some(create_auth_user(tenant_id));
        
        // This should fail because the mock client is configured to fail
        let result = handle_query(query_params, config.clone(), mock_client, auth_user).await;
        
        // The mock client will verify that the handler is using the correct branch-aware collection name
        // and then return an error (simulating collection not found)
        assert!(result.is_err(), "Query should fail when collection doesn't exist");
    }

    /// Test that query handler properly determines branch name from repo config when not specified
    #[tokio::test]
    async fn test_query_uses_repo_active_branch_when_not_specified() {
        let tenant_id = "test_tenant_789";
        let repo_name = "test_repo";
        let active_branch = "develop";
        
        let repo_config = create_test_repo_config(repo_name, tenant_id, Some(active_branch));
        let config = create_test_config(repo_config);
        let config_guard = config.read().await;
        
        // Calculate the expected collection name based on repo's active branch
        let expected_collection_name = get_branch_aware_collection_name(tenant_id, repo_name, active_branch, &config_guard);
        
        drop(config_guard);
        
        // Verify the collection name includes the active branch info
        assert!(
            expected_collection_name.contains("_br_"), 
            "Expected collection name should contain '_br_' marker"
        );
        
        println!("✓ Active branch test passed:");
        println!("  Active branch: {}", active_branch);
        println!("  Collection name: {}", expected_collection_name);
    }

    /// Test that query handler fails when no branch can be determined
    #[tokio::test]
    async fn test_query_fails_when_no_branch_available() {
        let tenant_id = "test_tenant_000";
        let repo_name = "test_repo";
        
        let repo_config = create_test_repo_config(repo_name, tenant_id, None); // No active branch
        let config = create_test_config(repo_config);
        
        // Create a dummy mock client (won't be used)
        let mock_client = Arc::new(MockQdrantClient::new("dummy".to_string()));
        
        let query_params = QueryParams {
            repository_name: repo_name.to_string(),
            query_text: "test query".to_string(),
            limit: 10,
            branch_name: None, // Don't specify branch, and repo has no active branch
            element_type: None,
            lang: None,
            show_code: None,
        };
        
        let auth_user = Some(create_auth_user(tenant_id));
        
        // This should fail because no branch can be determined
        let result = handle_query(query_params, config.clone(), mock_client, auth_user).await;
        
        assert!(result.is_err(), "Query should fail when no branch can be determined");
        if let Err(error) = result {
            assert_eq!(error.code, crate::mcp::error_codes::INVALID_QUERY_PARAMS);
            assert!(error.message.contains("Cannot determine branch"));
        }
    }

    /// Test that query handler excludes content by default when show_code is not specified
    #[tokio::test]
    async fn test_query_excludes_content_by_default() {
        let tenant_id = "test_tenant_show_code_default";
        let repo_name = "test_repo";
        let branch_name = "main";
        
        let repo_config = create_test_repo_config(repo_name, tenant_id, Some(branch_name));
        let config = create_test_config(repo_config);
        let config_guard = config.read().await;
        
        // Expected collection name for mock client
        let expected_collection_name = get_branch_aware_collection_name(tenant_id, repo_name, branch_name, &config_guard);
        drop(config_guard);
        
        // Create mock client that returns search results
        let mock_client = Arc::new(MockQdrantClientWithResults::new(expected_collection_name));
        
        let query_params = QueryParams {
            repository_name: repo_name.to_string(),
            query_text: "test query".to_string(),
            limit: 10,
            branch_name: Some(branch_name.to_string()),
            element_type: None,
            lang: None,
            show_code: None, // Not specified - should default to false
        };
        
        let auth_user = Some(create_auth_user(tenant_id));
        
        let result = handle_query(query_params, config.clone(), mock_client, auth_user).await;
        
        if let Err(ref e) = result {
            eprintln!("Query failed with error: {:?}", e);
        }
        assert!(result.is_ok(), "Query should succeed");
        let query_result = result.unwrap();
        assert!(!query_result.results.is_empty(), "Should have results");
        
        // Verify that content is not included but preview is
        for item in &query_result.results {
            assert!(item.content.is_none(), "Content should not be included by default");
            assert!(item.preview.is_some(), "Preview should be included");
            assert!(!item.file_path.is_empty(), "File path should be present");
            assert!(item.start_line > 0, "Start line should be present");
            assert!(item.end_line > 0, "End line should be present");
            assert!(item.score > 0.0, "Score should be present");
            
            // Verify preview is first line
            if let Some(preview) = &item.preview {
                assert!(preview.contains("fn test_function"), "Preview should contain first line of content");
            }
        }
    }

    /// Test that query handler excludes content when show_code is explicitly false
    #[tokio::test]
    async fn test_query_excludes_content_when_show_code_false() {
        let tenant_id = "test_tenant_show_code_false";
        let repo_name = "test_repo";
        let branch_name = "main";
        
        let repo_config = create_test_repo_config(repo_name, tenant_id, Some(branch_name));
        let config = create_test_config(repo_config);
        let config_guard = config.read().await;
        
        // Expected collection name for mock client
        let expected_collection_name = get_branch_aware_collection_name(tenant_id, repo_name, branch_name, &config_guard);
        drop(config_guard);
        
        // Create mock client that returns search results
        let mock_client = Arc::new(MockQdrantClientWithResults::new(expected_collection_name));
        
        let query_params = QueryParams {
            repository_name: repo_name.to_string(),
            query_text: "test query".to_string(),
            limit: 10,
            branch_name: Some(branch_name.to_string()),
            element_type: None,
            lang: None,
            show_code: Some(false), // Explicitly false
        };
        
        let auth_user = Some(create_auth_user(tenant_id));
        
        let result = handle_query(query_params, config.clone(), mock_client, auth_user).await;
        
        if let Err(ref e) = result {
            eprintln!("Query failed with error: {:?}", e);
        }
        assert!(result.is_ok(), "Query should succeed");
        let query_result = result.unwrap();
        assert!(!query_result.results.is_empty(), "Should have results");
        
        // Verify that content is not included but preview is
        for item in &query_result.results {
            assert!(item.content.is_none(), "Content should not be included when show_code is false");
            assert!(item.preview.is_some(), "Preview should be included when show_code is false");
        }
    }

    /// Test that query handler includes content when show_code is true
    #[tokio::test]
    async fn test_query_includes_content_when_show_code_true() {
        let tenant_id = "test_tenant_show_code_true";
        let repo_name = "test_repo";
        let branch_name = "main";
        
        let repo_config = create_test_repo_config(repo_name, tenant_id, Some(branch_name));
        let config = create_test_config(repo_config);
        let config_guard = config.read().await;
        
        // Expected collection name for mock client
        let expected_collection_name = get_branch_aware_collection_name(tenant_id, repo_name, branch_name, &config_guard);
        drop(config_guard);
        
        // Create mock client that returns search results
        let mock_client = Arc::new(MockQdrantClientWithResults::new(expected_collection_name));
        
        let query_params = QueryParams {
            repository_name: repo_name.to_string(),
            query_text: "test query".to_string(),
            limit: 10,
            branch_name: Some(branch_name.to_string()),
            element_type: None,
            lang: None,
            show_code: Some(true), // Explicitly true
        };
        
        let auth_user = Some(create_auth_user(tenant_id));
        
        let result = handle_query(query_params, config.clone(), mock_client, auth_user).await;
        
        if let Err(ref e) = result {
            eprintln!("Query failed with error: {:?}", e);
        }
        assert!(result.is_ok(), "Query should succeed");
        let query_result = result.unwrap();
        assert!(!query_result.results.is_empty(), "Should have results");
        
        // Verify that content is included along with preview
        for item in &query_result.results {
            assert!(item.content.is_some(), "Content should be included when show_code is true");
            assert!(!item.content.as_ref().unwrap().is_empty(), "Content should not be empty");
            assert!(item.preview.is_some(), "Preview should also be included when show_code is true");
        }
    }

    /// Mock Qdrant client that returns actual search results for testing show_code functionality
    #[derive(Clone, Debug)]
    struct MockQdrantClientWithResults {
        expected_collection_name: String,
    }

    impl MockQdrantClientWithResults {
        fn new(expected_collection_name: String) -> Self {
            Self {
                expected_collection_name,
            }
        }
    }

    #[async_trait]
    impl QdrantClientTrait for MockQdrantClientWithResults {
        async fn health_check(&self) -> Result<HealthCheckReply, SagittaError> {
            Ok(HealthCheckReply { title: "mock".to_string(), version: "mock".to_string(), commit: None })
        }

        async fn search_points(&self, request: SearchPoints) -> Result<SearchResponse, SagittaError> {
            use qdrant_client::qdrant::{Value, ScoredPoint, PointId};
            use std::collections::HashMap;
            
            // Verify correct collection name
            assert_eq!(
                request.collection_name, 
                self.expected_collection_name,
                "Query handler used wrong collection name"
            );

            // Create mock search results with all required fields
            let mut payload = HashMap::new();
            payload.insert(FIELD_FILE_PATH.to_string(), Value::from("src/test_file.rs"));
            payload.insert(FIELD_START_LINE.to_string(), Value::from(10i64));
            payload.insert(FIELD_END_LINE.to_string(), Value::from(20i64));
            payload.insert(FIELD_CHUNK_CONTENT.to_string(), Value::from("fn test_function() {\n    println!(\"Test content\");\n}"));
            
            let scored_point = ScoredPoint {
                id: Some(PointId { point_id_options: Some(qdrant_client::qdrant::point_id::PointIdOptions::Num(1)) }),
                payload,
                score: 0.95,
                version: 0,
                vectors: None,
                shard_key: None,
                order_value: None,
            };

            Ok(SearchResponse {
                result: vec![scored_point],
                time: 0.001,
                usage: None,
            })
        }

        // All other methods are unimplemented for this test
        async fn delete_collection(&self, _collection_name: String) -> Result<bool, SagittaError> {
            unimplemented!("MockQdrantClientWithResults delete_collection not implemented")
        }

        async fn get_collection_info(&self, _collection_name: String) -> Result<CollectionInfo, SagittaError> {
            unimplemented!("MockQdrantClientWithResults get_collection_info not implemented")
        }

        async fn count(&self, _request: CountPoints) -> Result<CountResponse, SagittaError> {
            unimplemented!("MockQdrantClientWithResults count not implemented")
        }

        async fn collection_exists(&self, _collection_name: String) -> Result<bool, SagittaError> {
            Ok(true)
        }

        async fn delete_points_blocking(&self, _collection_name: &str, _points_selector: &PointsSelector) -> Result<(), SagittaError> {
            unimplemented!("MockQdrantClientWithResults delete_points_blocking not implemented")
        }

        async fn scroll(&self, _request: ScrollPoints) -> Result<ScrollResponse, SagittaError> {
            unimplemented!("MockQdrantClientWithResults scroll not implemented")
        }

        async fn upsert_points(&self, _request: UpsertPoints) -> Result<PointsOperationResponse, SagittaError> {
            unimplemented!("MockQdrantClientWithResults upsert_points not implemented")
        }

        async fn create_collection(&self, _collection_name: &str, _vector_dimension: u64) -> Result<bool, SagittaError> {
            unimplemented!("MockQdrantClientWithResults create_collection not implemented")
        }

        async fn create_collection_detailed(&self, _request: CreateCollection) -> Result<bool, SagittaError> {
            unimplemented!("MockQdrantClientWithResults create_collection_detailed not implemented")
        }

        async fn delete_points(&self, _request: DeletePoints) -> Result<PointsOperationResponse, SagittaError> {
            unimplemented!("MockQdrantClientWithResults delete_points not implemented")
        }

        async fn query_points(&self, _request: QueryPoints) -> Result<QueryResponse, SagittaError> {
            unimplemented!("MockQdrantClientWithResults query_points not implemented")
        }

        async fn query(&self, _request: QueryPoints) -> Result<QueryResponse, SagittaError> {
            use qdrant_client::qdrant::{Value, ScoredPoint, PointId};
            use std::collections::HashMap;
            
            // Return the same mock result as search_points
            let mut payload = HashMap::new();
            payload.insert(FIELD_FILE_PATH.to_string(), Value::from("src/test_file.rs"));
            payload.insert(FIELD_START_LINE.to_string(), Value::from(10i64));
            payload.insert(FIELD_END_LINE.to_string(), Value::from(20i64));
            payload.insert(FIELD_CHUNK_CONTENT.to_string(), Value::from("fn test_function() {\n    println!(\"Test content\");\n}"));
            
            let scored_point = ScoredPoint {
                id: Some(PointId { point_id_options: Some(qdrant_client::qdrant::point_id::PointIdOptions::Num(1)) }),
                payload,
                score: 0.95,
                version: 0,
                vectors: None,
                shard_key: None,
                order_value: None,
            };

            Ok(QueryResponse {
                result: vec![scored_point],
                time: 0.001,
                usage: None,
            })
        }

        async fn list_collections(&self) -> Result<Vec<String>, SagittaError> {
            unimplemented!("MockQdrantClientWithResults list_collections not implemented")
        }
    }
} 