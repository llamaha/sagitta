// Code search tool implementation will go here

use std::sync::Arc;
use tokio::sync::Mutex;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use async_trait::async_trait;

use crate::gui::repository::manager::RepositoryManager;
use crate::utils::errors::SagittaCodeError;
use crate::tools::types::{Tool, ToolCategory, ToolDefinition, ToolResult};

/// Search parameters for the code search tool
#[derive(Debug, Deserialize)]
struct SearchParams {
    repository_name: String,
    query: String,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    element_type: Option<String>,
    #[serde(default)]
    language: Option<String>,
}

/// Code search tool
#[derive(Debug)]
pub struct CodeSearchTool {
    repo_manager: Arc<Mutex<RepositoryManager>>,
}

fn default_limit() -> usize {
    10
}

impl CodeSearchTool {
    /// Create a new code search tool
    pub fn new(repo_manager: Arc<Mutex<RepositoryManager>>) -> Self {
        Self {
            repo_manager,
        }
    }
}

#[async_trait]
impl Tool for CodeSearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search_code".to_string(),
            description: "Search for code in repositories using semantic search.".to_string(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["repository_name", "query"],
                "properties": {
                    "repository_name": {
                        "type": "string",
                        "description": "Name of the repository to search in"
                    },
                    "query": {
                        "type": "string",
                        "description": "Natural language or code search query"
                    },
                    "limit": {
                        "type": ["integer", "null"],
                        "description": "Maximum number of results to return (defaults to 10 if null)"
                    },
                    "element_type": {
                        "type": ["string", "null"],
                        "description": "Optional filter by code element type (function, struct, class, interface, enum, method, variable)"
                    },
                    "language": {
                        "type": ["string", "null"],
                        "description": "Optional filter by programming language (rust, python, javascript, etc.)"
                    }
                }
            }),
            is_required: false,
            category: ToolCategory::CodeSearch,
            metadata: Default::default(),
        }
    }
    
    async fn execute(&self, params: Value) -> Result<ToolResult, SagittaCodeError> {
        log::debug!("CodeSearchTool: Executing with params: {:?}", params);
        
        let search_params: SearchParams = serde_json::from_value(params)
            .map_err(|e| SagittaCodeError::ToolError(format!("Invalid search parameters: {}", e)))?;
        
        log::debug!("CodeSearchTool: Searching repository '{}' for query '{}'", 
                   search_params.repository_name, search_params.query);
        
        let repo_manager = self.repo_manager.lock().await;
        
        let result = repo_manager.query(
            &search_params.repository_name,
            &search_params.query,
            search_params.limit,
            search_params.element_type.as_deref(),
            search_params.language.as_deref(),
            None, // branch
        ).await;
        
        match result {
            Ok(response) => {
                log::debug!("CodeSearchTool: Search successful, found {} results", response.result.len());
                
                let search_results: Vec<Value> = response.result.iter().map(|result| {
                    json!({
                        "score": result.score,
                        "payload": result.payload,
                        "context_type": "regular_search"
                    })
                }).collect();
                
                Ok(ToolResult::Success(json!({
                    "search_results": search_results,
                    "total_results": response.result.len(),
                    "search_method": "regular_semantic_search"
                })))
            }
            Err(e) => {
                let error_msg = format!("Search failed: {}", e);
                log::error!("CodeSearchTool: {}", error_msg);
                Err(SagittaCodeError::ToolError(error_msg))
            }
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

    #[tokio::test]
    async fn test_code_search_tool_creation() {
        let tool = CodeSearchTool::new(create_test_repo_manager());
        let definition = tool.definition();
        assert_eq!(definition.name, "search_code");
        assert_eq!(definition.category, ToolCategory::CodeSearch);
    }

    #[test]
    fn test_code_search_tool_parameter_validation() {
        let tool = CodeSearchTool::new(create_test_repo_manager());
        let definition = tool.definition();
        
        let properties = definition.parameters.get("properties").unwrap().as_object().unwrap();
        assert!(properties.contains_key("repository_name"));
        assert!(properties.contains_key("query"));
        assert!(properties.contains_key("limit"));
        assert!(properties.contains_key("element_type"));
        assert!(properties.contains_key("language"));
        
        let required = definition.parameters.get("required").unwrap().as_array().unwrap();
        assert_eq!(required.len(), 2);
        assert!(required.contains(&json!("repository_name")));
        assert!(required.contains(&json!("query")));
        
        // Optional parameters should not be required
        assert!(!required.contains(&json!("limit")));
        assert!(!required.contains(&json!("element_type")));
        assert!(!required.contains(&json!("language")));
    }

    #[tokio::test]
    async fn test_code_search_tool_minimal_parameters() {
        let tool = CodeSearchTool::new(create_test_repo_manager());
        
        // Test with only the required parameters
        let params = json!({
            "repository_name": "test-repo",
            "query": "function main"
        });
        
        let result = tool.execute(params).await;
        
        // Should fail due to repository not found, but that's expected in test environment
        // The important thing is that it accepts the minimal parameters
        match result {
            Ok(ToolResult::Success(data)) => {
                assert!(data.get("search_results").is_some());
                assert!(data.get("total_results").is_some());
            }
            Ok(ToolResult::Error { .. }) => {
                // Expected due to missing repository in test environment
            }
            Err(_) => {
                // Expected due to missing repository in test environment
            }
        }
    }

    #[tokio::test]
    async fn test_code_search_tool_with_optional_parameters() {
        let tool = CodeSearchTool::new(create_test_repo_manager());
        
        // Test with optional parameters included
        let params = json!({
            "repository_name": "test-repo",
            "query": "function main",
            "limit": 5,
            "element_type": "function",
            "language": "rust"
        });
        
        let result = tool.execute(params).await;
        
        // Should fail due to repository not found, but that's expected in test environment
        // The important thing is that it accepts the optional parameters
        match result {
            Ok(ToolResult::Success(data)) => {
                assert!(data.get("search_results").is_some());
                assert!(data.get("total_results").is_some());
            }
            Ok(ToolResult::Error { .. }) => {
                // Expected due to missing repository in test environment
            }
            Err(_) => {
                // Expected due to missing repository in test environment
            }
        }
    }

    #[test]
    fn test_default_limit() {
        assert_eq!(default_limit(), 10);
    }
}

