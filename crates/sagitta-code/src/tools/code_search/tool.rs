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
                        "type": "integer",
                        "description": "Maximum number of results to return",
                        "default": 10
                    },
                    "element_type": {
                        "type": "string",
                        "description": "Optional filter by code element type (function, struct, class, enum, etc.)",
                        "enum": ["function", "struct", "class", "interface", "enum", "method", "variable"]
                    },
                    "language": {
                        "type": "string",
                        "description": "Optional filter by programming language (rust, python, javascript, etc.)"
                    }
                },
                "required": ["repository_name", "query"]
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
}

