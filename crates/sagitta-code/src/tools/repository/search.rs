use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::gui::repository::manager::RepositoryManager;
use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::utils::errors::SagittaCodeError;
use sagitta_search::AppConfig as SagittaAppConfig;

/// Parameters for searching files in a repository
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchFileParams {
    /// The repository to search in
    pub repository_name: String,
    
    /// The pattern to search for (e.g., "*.rs" or "src/*.js")
    pub pattern: String,
    
    /// Whether to use case-sensitive matching
    #[serde(default)]
    pub case_sensitive: bool,
}

/// Tool for searching files in repositories
#[derive(Debug)]
pub struct SearchFileInRepositoryTool {
    /// Repository manager for accessing repositories
    repo_manager: Arc<Mutex<RepositoryManager>>,
}

impl SearchFileInRepositoryTool {
    /// Create a new search file tool
    pub fn new(repo_manager: Arc<Mutex<RepositoryManager>>) -> Self {
        Self {
            repo_manager,
        }
    }
}

#[async_trait]
impl Tool for SearchFileInRepositoryTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search_file_in_repository".to_string(),
            description: "Search for files in a repository using a glob pattern (e.g., '*.rs' or 'src/*.js'). Returns a list of matching file paths.".to_string(),
            category: ToolCategory::Repository,
            is_required: false,
            parameters: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["repository_name", "pattern"],
                "properties": {
                    "repository_name": {
                        "type": "string",
                        "description": "The repository to search in"
                    },
                    "pattern": {
                        "type": "string",
                        "description": "The pattern to search for (e.g., '*.rs' or 'src/*.js')"
                    },
                    "case_sensitive": {
                        "type": ["boolean", "null"],
                        "description": "Whether to use case-sensitive matching (defaults to false if null)"
                    }
                }
            }),
            metadata: std::collections::HashMap::new(),
        }
    }
    
    async fn execute(&self, parameters: Value) -> Result<ToolResult, SagittaCodeError> {
        let params: SearchFileParams = serde_json::from_value(parameters)
            .map_err(|e| SagittaCodeError::ToolError(format!("Invalid parameters: {}", e)))?;
        
        let repo_manager = self.repo_manager.lock().await;
        let result = repo_manager.search_file(&params.repository_name, &params.pattern, false).await;
        
        match result {
            Ok(files) => Ok(ToolResult::Success(serde_json::json!({
                "files": files,
                "pattern": params.pattern
            }))),
            Err(e) => Err(SagittaCodeError::ToolError(format!("Failed to search files: {}", e)))
        }
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
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

    #[test]
    fn test_search_repository_tool_creation() {
        let tool = SearchFileInRepositoryTool::new(create_test_repo_manager());
        let definition = tool.definition();
        assert_eq!(definition.name, "search_file_in_repository");
    }

    #[test]
    fn test_tool_definition() {
        let tool = SearchFileInRepositoryTool::new(create_test_repo_manager());
        let definition = tool.definition();
        
        assert_eq!(definition.name, "search_file_in_repository");
        assert_eq!(definition.category, ToolCategory::Repository);
        assert!(definition.description.contains("Search for files"));
    }

    #[test]
    fn test_tool_definition_properties() {
        let tool = SearchFileInRepositoryTool::new(create_test_repo_manager());
        let definition = tool.definition();
        
        let properties = definition.parameters.get("properties").unwrap().as_object().unwrap();
        assert!(properties.contains_key("repository_name"));
        assert!(properties.contains_key("pattern"));
        assert!(properties.contains_key("case_sensitive"));
        
        let required = definition.parameters.get("required").unwrap().as_array().unwrap();
        assert!(required.contains(&json!("repository_name")));
        assert!(required.contains(&json!("pattern")));
    }

    #[test]
    fn test_tool_category_and_metadata() {
        let tool = SearchFileInRepositoryTool::new(create_test_repo_manager());
        let definition = tool.definition();
        
        assert_eq!(definition.category, ToolCategory::Repository);
        assert!(definition.description.len() > 10);
    }

    #[test]
    fn test_description_content() {
        let tool = SearchFileInRepositoryTool::new(create_test_repo_manager());
        let definition = tool.definition();
        
        assert!(definition.description.contains("Search"));
        assert!(definition.description.contains("files"));
        assert!(definition.description.contains("repository"));
    }

    #[test]
    fn test_search_repository_params_default_case_sensitive() {
        let params = SearchFileParams {
            repository_name: "test-repo".to_string(),
            pattern: "*.rs".to_string(),
            case_sensitive: false,
        };
        
        assert_eq!(params.repository_name, "test-repo");
        assert_eq!(params.pattern, "*.rs");
        assert!(!params.case_sensitive);
    }

    #[test]
    fn test_search_repository_params_with_case_sensitive() {
        let params = SearchFileParams {
            repository_name: "test-repo".to_string(),
            pattern: "*.rs".to_string(),
            case_sensitive: true,
        };
        
        assert_eq!(params.repository_name, "test-repo");
        assert_eq!(params.pattern, "*.rs");
        assert!(params.case_sensitive);
    }

    #[test]
    fn test_search_repository_params_serialization() {
        let params = SearchFileParams {
            repository_name: "test-repo".to_string(),
            pattern: "*.rs".to_string(),
            case_sensitive: true,
        };
        
        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("test-repo"));
        assert!(json.contains("*.rs"));
        assert!(json.contains("true"));
        
        let deserialized: SearchFileParams = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.repository_name, params.repository_name);
        assert_eq!(deserialized.pattern, params.pattern);
        assert_eq!(deserialized.case_sensitive, params.case_sensitive);
    }

    #[tokio::test]
    async fn test_execute_with_invalid_parameters() {
        let tool = SearchFileInRepositoryTool::new(create_test_repo_manager());
        let invalid_params = json!({
            "pattern": "*.rs"
            // missing repository_name
        });
        
        let result = tool.execute(invalid_params).await;
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("missing field `repository_name`"));
    }

    #[tokio::test]
    async fn test_execute_with_missing_repository_name() {
        let tool = SearchFileInRepositoryTool::new(create_test_repo_manager());
        let params = json!({
            "pattern": "*.rs"
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("missing field `repository_name`"));
    }

    #[tokio::test]
    async fn test_execute_with_missing_pattern() {
        let tool = SearchFileInRepositoryTool::new(create_test_repo_manager());
        let params = json!({
            "repository_name": "test-repo"
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("missing field `pattern`"));
    }

    #[tokio::test]
    async fn test_execute_with_valid_params() {
        let tool = SearchFileInRepositoryTool::new(create_test_repo_manager());
        let params = json!({
            "repository_name": "test-repo",
            "pattern": "*.rs",
            "case_sensitive": false
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_err());
        // Should get repository not found error since test-repo doesn't exist
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Repository 'test-repo' not found"));
    }

    #[tokio::test]
    async fn test_execute_with_case_sensitive_flag() {
        let tool = SearchFileInRepositoryTool::new(create_test_repo_manager());
        let params = json!({
            "repository_name": "test-repo",
            "pattern": "*.RS",
            "case_sensitive": true
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_err());
        // Should get repository not found error since test-repo doesn't exist
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Repository 'test-repo' not found"));
    }

    #[tokio::test]
    async fn test_empty_pattern_parameter() {
        let tool = SearchFileInRepositoryTool::new(create_test_repo_manager());
        let params = json!({
            "repository_name": "test-repo",
            "pattern": "",
            "case_sensitive": false
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_err());
        // Should get repository not found error since test-repo doesn't exist
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Repository 'test-repo' not found"));
    }

    #[tokio::test]
    async fn test_parameter_validation_edge_cases() {
        let tool = SearchFileInRepositoryTool::new(create_test_repo_manager());
        let params = json!({
            "repository_name": "",
            "pattern": "*.rs",
            "case_sensitive": false
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_err());
        // Should get repository not found error for empty repository name
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Repository '' not found"));
    }

    #[tokio::test]
    async fn test_complex_pattern_validation() {
        let tool = SearchFileInRepositoryTool::new(create_test_repo_manager());
        let params = json!({
            "repository_name": "test-repo",
            "pattern": "**/*.{rs,toml,md}",
            "case_sensitive": false
        });
        
        let result = tool.execute(params).await;
        assert!(result.is_err());
        // Should get repository not found error since test-repo doesn't exist
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Repository 'test-repo' not found"));
    }
} 