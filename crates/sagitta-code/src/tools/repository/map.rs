use std::sync::Arc;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;
use std::collections::HashMap;

use crate::gui::repository::manager::RepositoryManager;
use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::utils::errors::SagittaCodeError;
use repo_mapper::{generate_repo_map, get_cached_page, RepoMapOptions};
use sagitta_search::config::AppConfig;

/// Parameters for mapping a repository
#[derive(Debug, Deserialize, Serialize)]
pub struct RepositoryMapParams {
    /// Name of the repository to map (uses current repository context if not provided)
    pub name: Option<String>,
    /// Verbosity level (0=minimal, 1=normal, 2=detailed)
    #[serde(default = "default_verbosity")]
    pub verbosity: u8,
    /// Optional: Specific paths to scan within the repository
    pub paths: Option<Vec<String>>,
    /// Optional: Filter by file extension (e.g., "rs", "py", "js")
    pub file_extension: Option<String>,
    /// Optional: Content pattern to filter files by
    pub content_pattern: Option<String>,
    /// Optional: Number of files per page (enables pagination)
    pub files_per_page: Option<usize>,
    /// Optional: Page number to retrieve (1-based)
    pub page: Option<usize>,
    /// Optional: Cache key for retrieving subsequent pages
    pub cache_key: Option<String>,
}

fn default_verbosity() -> u8 {
    1 // Default verbosity level
}

/// Tool for generating repository structure maps using the repo-mapper crate
#[derive(Debug)]
pub struct RepositoryMapTool {
    repo_manager: Arc<Mutex<RepositoryManager>>,
}

impl RepositoryMapTool {
    /// Create a new repository map tool
    pub fn new(repo_manager: Arc<Mutex<RepositoryManager>>) -> Self {
        Self {
            repo_manager,
        }
    }
    
    /// Generate a repository map using the repo-mapper crate
    async fn generate_map(&self, params: &RepositoryMapParams) -> Result<serde_json::Value, SagittaCodeError> {
        // Try to use cache if cache_key is provided
        if let Some(ref cache_key) = params.cache_key {
            if let (Some(page), Some(files_per_page)) = (params.page, params.files_per_page) {
                match get_cached_page(cache_key, page, files_per_page) {
                    Ok(result) => {
                        return Ok(self.format_response(&result, &params.name.as_ref().unwrap_or(&"cached".to_string())));
                    }
                    Err(_) => {
                        // Cache miss, continue with normal flow
                    }
                }
            }
        }

        let repo_manager = self.repo_manager.lock().await;
        
        // Check if repository exists and get its path
        let repositories = repo_manager.list_repositories().await
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to list repositories: {}", e)))?;
        
        // Get the repository name
        let repo_name = match &params.name {
            Some(name) => name.clone(),
            None => {
                return Err(SagittaCodeError::ToolError(
                    "No repository name provided. Please either: 1) Select a repository from the UI dropdown to set the context, or 2) Provide the 'name' parameter explicitly. You can use the 'list_repositories' tool to see available repositories.".to_string()
                ));
            }
        };
        
        let repo = repositories.iter()
            .find(|r| r.name == repo_name)
            .ok_or_else(|| SagittaCodeError::ToolError(format!("Repository '{}' not found", repo_name)))?;

        let repo_path = &repo.local_path;

        // Convert parameters to repo-mapper options
        let options = RepoMapOptions {
            verbosity: params.verbosity,
            file_extension: params.file_extension.clone(),
            content_pattern: params.content_pattern.clone(),
            paths: params.paths.clone(),
            files_per_page: params.files_per_page,
            page: params.page,
            max_output_lines: Some(1000), // Auto-paginate large outputs
            smart_sort: true,
            max_calls_per_method: match params.verbosity {
                0 => 3,
                1 => 10,
                _ => 20,
            },
            include_context: params.verbosity >= 2,
            include_docstrings: params.verbosity >= 1,
        };

        // Generate the repository map
        let result = generate_repo_map(repo_path, options)
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to generate repository map: {}", e)))?;
        
        Ok(self.format_response(&result, &repo_name))
    }
    
    fn format_response(&self, result: &repo_mapper::RepoMapResult, repo_name: &str) -> serde_json::Value {
        let mut response = serde_json::json!({
            "success": true,
            "repository_name": repo_name,
            "map_content": result.map_content,
            "summary": result.summary,
        });
        
        // Add pagination info if present
        if let Some(ref pagination) = result.pagination {
            response["pagination"] = serde_json::json!(pagination);
            
            // Add navigation hints for LLM
            response["navigation_hint"] = if pagination.has_next {
                format!("To see more files, request page {} with files_per_page={}", 
                    pagination.current_page + 1, 
                    pagination.files_per_page).into()
            } else {
                "This is the last page.".into()
            };
        }
        
        // Add cache key if present
        if let Some(ref cache_key) = result.cache_key {
            response["cache_key"] = serde_json::json!(cache_key);
        }
        
        // Only include raw methods if not paginated
        if result.pagination.is_none() {
            response["raw_methods_by_file"] = serde_json::json!(result.methods_by_file);
        }
        
        response
    }
}

#[async_trait]
impl Tool for RepositoryMapTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "repository_map".to_string(),
            description: "Generate a high-level map of a repository's code structure. Automatically paginates large results (>1000 lines). For subsequent pages, use the cache_key returned in the first response.".to_string(),
            category: ToolCategory::Repository,
            is_required: false,
            parameters: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": [],
                "properties": {
                    "name": {
                        "type": ["string", "null"],
                        "description": "Name of the repository to map (uses current context if not provided)"
                    },
                    "verbosity": {
                        "type": ["integer", "null"],
                        "description": "Verbosity level (0=minimal, 1=normal, 2=detailed)",
                        "minimum": 0,
                        "maximum": 2,
                        "default": 1
                    },
                    "paths": {
                        "type": ["array", "null"],
                        "items": {"type": "string"},
                        "description": "Specific paths to scan within the repository"
                    },
                    "file_extension": {
                        "type": ["string", "null"],
                        "description": "Filter by file extension (e.g., 'rs', 'py', 'js')"
                    },
                    "content_pattern": {
                        "type": ["string", "null"],
                        "description": "Content pattern to filter files by"
                    },
                    "files_per_page": {
                        "type": ["integer", "null"],
                        "description": "Number of files per page (enables manual pagination)",
                        "minimum": 1,
                        "maximum": 100
                    },
                    "page": {
                        "type": ["integer", "null"],
                        "description": "Page number to retrieve (1-based)",
                        "minimum": 1
                    },
                    "cache_key": {
                        "type": ["string", "null"],
                        "description": "Cache key from previous response for retrieving subsequent pages"
                    }
                }
            }),
            metadata: std::collections::HashMap::new(),
        }
    }
    
    async fn execute(&self, parameters: serde_json::Value) -> Result<ToolResult, SagittaCodeError> {
        let params: RepositoryMapParams = match serde_json::from_value(parameters) {
            Ok(params) => params,
            Err(e) => return Ok(ToolResult::Error {
                error: format!("Invalid parameters: {}", e)
            })
        };
        
        // Use the generate_map method that includes repo-mapper functionality
        match self.generate_map(&params).await {
            Ok(map_result) => Ok(ToolResult::Success(map_result)),
            Err(e) => Ok(ToolResult::Error {
                error: e.to_string()
            })
        }
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use crate::gui::repository::manager::RepositoryManager;
    use sagitta_search::config::AppConfig;

    fn create_test_repo_manager() -> Arc<Mutex<RepositoryManager>> {
        let mock_config = Arc::new(Mutex::new(AppConfig::default()));
        Arc::new(Mutex::new(RepositoryManager::new_for_test(mock_config)))
    }

    #[tokio::test]
    async fn test_repository_map_tool_definition() {
        let repo_manager = create_test_repo_manager();
        let tool = RepositoryMapTool::new(repo_manager);
        let definition = tool.definition();
        
        assert_eq!(definition.name, "repository_map");
        assert!(!definition.description.is_empty());
        assert!(definition.description.contains("high-level map"));
        assert!(definition.description.contains("Automatically paginates"));
        assert_eq!(definition.category, ToolCategory::Repository);
        assert!(!definition.is_required);
        
        // Check parameters
        let params = definition.parameters;
        assert!(params.get("properties").is_some());
        let properties = params.get("properties").unwrap().as_object().unwrap();
        assert!(properties.contains_key("name"));
        assert!(properties.contains_key("verbosity"));
        assert!(properties.contains_key("paths"));
        assert!(properties.contains_key("file_extension"));
        assert!(properties.contains_key("content_pattern"));
        
        // Check required fields (should be empty now)
        let required = params.get("required").unwrap().as_array().unwrap();
        assert!(required.is_empty());
    }

    #[tokio::test]
    async fn test_repository_map_tool_execution() {
        let repo_manager = create_test_repo_manager();
        let tool = RepositoryMapTool::new(repo_manager);
        
        let params = serde_json::json!({
            "name": "test_repo",
            "verbosity": 1,
            "file_extension": "rs"
        });
        
        let result = tool.execute(params).await.unwrap();
        
        // Should return an error since the repository doesn't exist
        match result {
            ToolResult::Error { error } => {
                assert!(error.contains("not found"));
            }
            _ => panic!("Expected error for non-existent repository"),
        }
    }

    #[tokio::test]
    async fn test_repository_map_tool_parameter_validation() {
        let repo_manager = create_test_repo_manager();
        let tool = RepositoryMapTool::new(repo_manager);
        
        // Test with missing required parameter
        let params = serde_json::json!({
            "verbosity": 1
        });
        
        let result = tool.execute(params).await.unwrap();
        
        match result {
            ToolResult::Error { error } => {
                assert!(error.contains("missing field `name`") || error.contains("No repository name provided"));
            }
            _ => panic!("Expected parameter validation error"),
        }
    }

    #[tokio::test]
    async fn test_repository_map_params_defaults() {
        let json_str = r#"{"name": "test_repo"}"#;
        let params: RepositoryMapParams = serde_json::from_str(json_str).unwrap();
        
        assert_eq!(params.name, Some("test_repo".to_string()));
        assert_eq!(params.verbosity, 1); // Should use default
        assert!(params.paths.is_none());
        assert!(params.file_extension.is_none());
        assert!(params.content_pattern.is_none());
    }

    #[tokio::test]
    async fn test_repository_map_verbosity_levels() {
        let repo_manager = create_test_repo_manager();
        let tool = RepositoryMapTool::new(repo_manager);
        
        for verbosity in 0..=2 {
            let params = serde_json::json!({
                "name": "test_repo",
                "verbosity": verbosity
            });
            
            let result = tool.execute(params).await.unwrap();
            
            // Should fail due to missing repo, but parameter parsing should work
            match result {
                ToolResult::Error { error } => {
                    assert!(!error.contains("Invalid parameters"));
                }
                _ => {}
            }
        }
    }

    #[tokio::test]
    async fn test_repository_map_with_content_pattern() {
        let repo_manager = create_test_repo_manager();
        let tool = RepositoryMapTool::new(repo_manager);
        
        let params = serde_json::json!({
            "name": "test_repo",
            "verbosity": 1,
            "content_pattern": "async"
        });
        
        let result = tool.execute(params).await.unwrap();
        
        // Should fail due to missing repo, but parameter parsing should work
        match result {
            ToolResult::Error { error } => {
                assert!(!error.contains("Invalid parameters"));
                assert!(error.contains("not found"));
            }
            _ => panic!("Expected error for non-existent repository"),
        }
    }

    #[tokio::test]
    async fn test_repository_map_params_serialization() {
        let params = RepositoryMapParams {
            name: Some("test_repo".to_string()),
            verbosity: 2,
            paths: Some(vec!["src/".to_string(), "tests/".to_string()]),
            file_extension: Some("rs".to_string()),
            content_pattern: Some("async".to_string()),
        };
        
        let serialized = serde_json::to_string(&params).unwrap();
        let deserialized: RepositoryMapParams = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(params.name, deserialized.name);
        assert_eq!(params.verbosity, deserialized.verbosity);
        assert_eq!(params.paths, deserialized.paths);
        assert_eq!(params.file_extension, deserialized.file_extension);
        assert_eq!(params.content_pattern, deserialized.content_pattern);
    }

    #[tokio::test]
    async fn test_default_verbosity_function() {
        assert_eq!(default_verbosity(), 1);
    }

    #[tokio::test]
    async fn test_repository_map_tool_creation() {
        let repo_manager = create_test_repo_manager();
        let tool = RepositoryMapTool::new(repo_manager);
        
        // Test that the tool can be created without panicking
        assert!(format!("{:?}", tool).contains("RepositoryMapTool"));
    }
} 