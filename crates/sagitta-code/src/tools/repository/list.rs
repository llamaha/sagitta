// List repositories tool will go here

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::gui::repository::manager::RepositoryManager;
use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::utils::errors::SagittaCodeError;

/// Tool for listing repositories
#[derive(Debug)]
pub struct ListRepositoriesTool {
    /// Repository manager for accessing repositories
    repo_manager: Arc<Mutex<RepositoryManager>>,
}

impl ListRepositoriesTool {
    /// Create a new list repositories tool
    pub fn new(repo_manager: Arc<Mutex<RepositoryManager>>) -> Self {
        Self {
            repo_manager,
        }
    }
}

#[async_trait]
impl Tool for ListRepositoriesTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "list_repositories".to_string(),
            description: "List all available repositories".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            is_required: false,
            category: ToolCategory::Repository,
            metadata: Default::default(),
        }
    }
    
    async fn execute(&self, _parameters: Value) -> Result<ToolResult, SagittaCodeError> {
        // Get the repository manager
        let repo_manager = self.repo_manager.lock().await;
        
        // Get enhanced repository list
        match repo_manager.get_enhanced_repository_list().await {
            Ok(enhanced_list) => {
                // Convert enhanced repository list to JSON value with comprehensive information
                let repo_list: Vec<Value> = enhanced_list.repositories.iter().map(|repo| {
                    let mut repo_json = serde_json::json!({
                        "name": repo.name,
                        "url": repo.url,
                        "local_path": repo.local_path.to_string_lossy(),
                        "default_branch": repo.default_branch,
                        "active_branch": repo.active_branch,
                        "tracked_branches": repo.tracked_branches,
                        "indexed_languages": repo.indexed_languages,
                        "added_as_local_path": repo.added_as_local_path,
                        "target_ref": repo.target_ref,
                        "tenant_id": repo.tenant_id,
                        "filesystem_status": {
                            "exists": repo.filesystem_status.exists,
                            "accessible": repo.filesystem_status.accessible,
                            "is_git_repository": repo.filesystem_status.is_git_repository,
                            "total_files": repo.filesystem_status.total_files,
                            "size_bytes": repo.filesystem_status.size_bytes
                        },
                        "sync_status": {
                            "state": format!("{:?}", repo.sync_status.state),
                            "last_synced_commits": repo.sync_status.last_synced_commits,
                            "branches_needing_sync": repo.sync_status.branches_needing_sync,
                            "sync_in_progress": repo.sync_status.sync_in_progress
                        },
                        "file_extensions": repo.file_extensions.iter().map(|ext| serde_json::json!({
                            "extension": ext.extension,
                            "count": ext.count,
                            "size_bytes": ext.size_bytes
                        })).collect::<Vec<_>>()
                    });

                    // Add git status if available
                    if let Some(git_status) = &repo.git_status {
                        repo_json["git_status"] = serde_json::json!({
                            "current_commit": git_status.current_commit,
                            "is_clean": git_status.is_clean,
                            "remote_url": git_status.remote_url,
                            "available_branches": git_status.available_branches,
                            "is_detached_head": git_status.is_detached_head
                        });
                    }

                    // Add last sync time if available
                    if let Some(last_sync_time) = &repo.last_sync_time {
                        repo_json["last_sync_time"] = serde_json::json!(last_sync_time.to_rfc3339());
                    }

                    repo_json
                }).collect();
                
                Ok(ToolResult::Success(serde_json::json!({
                    "repositories": repo_list,
                    "active_repository": enhanced_list.active_repository,
                    "total_count": enhanced_list.total_count,
                    "summary": {
                        "existing_count": enhanced_list.summary.existing_count,
                        "needs_sync_count": enhanced_list.summary.needs_sync_count,
                        "dirty_count": enhanced_list.summary.dirty_count,
                        "total_files": enhanced_list.summary.total_files,
                        "total_size_bytes": enhanced_list.summary.total_size_bytes,
                        "common_extensions": enhanced_list.summary.common_extensions.iter().map(|ext| serde_json::json!({
                            "extension": ext.extension,
                            "count": ext.count,
                            "size_bytes": ext.size_bytes
                        })).collect::<Vec<_>>()
                    }
                })))
            }
            Err(e) => {
                Err(SagittaCodeError::ToolError(format!("Failed to get enhanced repository list: {}", e)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::repository::manager::RepositoryManager;
    use sagitta_search::AppConfig as SagittaAppConfig;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use serde_json::json;

    fn create_test_repo_manager() -> Arc<Mutex<RepositoryManager>> {
        let config = SagittaAppConfig::default();
        let repo_manager = RepositoryManager::new_for_test(Arc::new(Mutex::new(config)));
        Arc::new(Mutex::new(repo_manager))
    }

    #[tokio::test]
    async fn test_list_repositories_tool_creation() {
        let repo_manager = create_test_repo_manager();
        let tool = ListRepositoriesTool::new(repo_manager);
        
        assert!(format!("{:?}", tool).contains("ListRepositoriesTool"));
    }

    #[tokio::test]
    async fn test_tool_definition() {
        let repo_manager = create_test_repo_manager();
        let tool = ListRepositoriesTool::new(repo_manager);
        let definition = tool.definition();
        
        assert_eq!(definition.name, "list_repositories");
        assert!(definition.description.contains("List all available repositories"));
        assert_eq!(definition.category, ToolCategory::Repository);
        assert!(!definition.is_required);
        
        // Check parameters structure - should be empty object
        let params = definition.parameters;
        assert!(params.get("type").is_some());
        assert!(params.get("properties").is_some());
        
        // Properties should be empty for list operation
        let properties = params.get("properties").unwrap();
        assert!(properties.as_object().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_execute_with_empty_parameters() {
        let repo_manager = create_test_repo_manager();
        let tool = ListRepositoriesTool::new(repo_manager);
        
        // Test with empty parameters (should work fine)
        let result = tool.execute(json!({})).await.unwrap();
        match result {
            ToolResult::Success(data) => {
                assert!(data.get("repositories").is_some());
                assert!(data.get("total_count").is_some());
                
                // Should be empty list since no repositories are configured
                let repos = data.get("repositories").unwrap().as_array().unwrap();
                assert_eq!(repos.len(), 0);
                
                let count = data.get("total_count").unwrap().as_u64().unwrap();
                assert_eq!(count, 0);
            }
            ToolResult::Error { error } => {
                // This might fail due to uninitialized repository manager
                // but should not be a parameter error
                assert!(!error.contains("Invalid parameters"));
            }
        }
    }

    #[tokio::test]
    async fn test_execute_with_any_parameters() {
        let repo_manager = create_test_repo_manager();
        let tool = ListRepositoriesTool::new(repo_manager);
        
        // Test with random parameters (should ignore them)
        let result = tool.execute(json!({
            "random_param": "value",
            "another_param": 123
        })).await.unwrap();
        
        match result {
            ToolResult::Success(data) => {
                assert!(data.get("repositories").is_some());
                assert!(data.get("total_count").is_some());
            }
            ToolResult::Error { error } => {
                // This might fail due to uninitialized repository manager
                // but should not be a parameter error
                assert!(!error.contains("Invalid parameters"));
            }
        }
    }

    #[tokio::test]
    async fn test_execute_with_null_parameters() {
        let repo_manager = create_test_repo_manager();
        let tool = ListRepositoriesTool::new(repo_manager);
        
        // Test with null value
        let result = tool.execute(Value::Null).await.unwrap();
        
        match result {
            ToolResult::Success(data) => {
                assert!(data.get("repositories").is_some());
                assert!(data.get("total_count").is_some());
            }
            ToolResult::Error { error } => {
                // This might fail due to uninitialized repository manager
                // but should not be a parameter error
                assert!(!error.contains("Invalid parameters"));
            }
        }
    }

    #[tokio::test]
    async fn test_tool_category_and_metadata() {
        let repo_manager = create_test_repo_manager();
        let tool = ListRepositoriesTool::new(repo_manager);
        let definition = tool.definition();
        
        assert_eq!(definition.category, ToolCategory::Repository);
        assert!(definition.metadata.is_empty());
        assert!(!definition.is_required);
    }

    #[tokio::test]
    async fn test_tool_name_consistency() {
        let repo_manager = create_test_repo_manager();
        let tool = ListRepositoriesTool::new(repo_manager);
        let definition = tool.definition();
        
        // Ensure the tool name follows the expected pattern
        assert_eq!(definition.name, "list_repositories");
        assert!(definition.name.contains("list"));
        assert!(definition.name.contains("repositories"));
    }

    #[tokio::test]
    async fn test_parameters_schema_structure() {
        let repo_manager = create_test_repo_manager();
        let tool = ListRepositoriesTool::new(repo_manager);
        let definition = tool.definition();
        
        let params = definition.parameters;
        
        // Should be a valid JSON schema object
        assert_eq!(params.get("type").unwrap().as_str().unwrap(), "object");
        
        let properties = params.get("properties").unwrap().as_object().unwrap();
        assert!(properties.is_empty(), "List tool should not require any parameters");
    }

    #[tokio::test]
    async fn test_description_content() {
        let repo_manager = create_test_repo_manager();
        let tool = ListRepositoriesTool::new(repo_manager);
        let definition = tool.definition();
        
        let description = &definition.description;
        assert!(description.len() > 10, "Description should be meaningful");
        assert!(description.to_lowercase().contains("list"));
        assert!(description.to_lowercase().contains("repositories"));
    }
}

