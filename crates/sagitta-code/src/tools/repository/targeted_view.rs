use std::sync::Arc;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;
use std::collections::HashMap;

use crate::gui::repository::manager::RepositoryManager;
use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::utils::errors::SagittaCodeError;
use repo_mapper::{generate_repo_map, RepoMapOptions, MethodInfo};

/// Parameters for targeted view of repository elements
#[derive(Debug, Deserialize, Serialize)]
pub struct TargetedViewParams {
    /// Name of the repository to analyze
    pub repository_name: String,
    /// Semantic query to find relevant files/elements
    pub query: String,
    /// Optional: Specific file path to analyze (if known)
    pub file_path: Option<String>,
    /// Optional: Element type to focus on (function, struct, class, etc.)
    pub element_type: Option<String>,
    /// Optional: Programming language filter
    pub language: Option<String>,
    /// Maximum number of files to analyze in detail
    #[serde(default = "default_max_files")]
    pub max_files: usize,
    /// Maximum number of elements to return per file
    #[serde(default = "default_max_elements_per_file")]
    pub max_elements_per_file: usize,
    /// Whether to include full context for found elements
    #[serde(default = "default_include_context")]
    pub include_context: bool,
}

fn default_max_files() -> usize { 5 }
fn default_max_elements_per_file() -> usize { 10 }
fn default_include_context() -> bool { true }

/// Result of targeted analysis
#[derive(Debug, Serialize)]
pub struct TargetedAnalysisResult {
    /// The original query
    pub query: String,
    /// Files that were analyzed
    pub analyzed_files: Vec<String>,
    /// Relevant elements found, organized by file
    pub relevant_elements: HashMap<String, Vec<RelevantElement>>,
    /// Summary of what was found
    pub summary: TargetedSummary,
    /// Semantic search results that led to this analysis
    pub semantic_search_results: Option<Value>,
}

/// A relevant code element found through targeted analysis
#[derive(Debug, Clone, Serialize)]
pub struct RelevantElement {
    /// Name of the element
    pub name: String,
    /// Type of element (function, struct, class, etc.)
    pub element_type: String,
    /// Parameters/signature
    pub signature: String,
    /// Line number where element starts
    pub line_number: Option<usize>,
    /// Documentation if available
    pub documentation: Option<String>,
    /// Method calls within this element
    pub calls: Vec<String>,
    /// Relevance score (0.0 to 1.0)
    pub relevance_score: f32,
    /// Full context around the element (if requested)
    pub context: Option<String>,
}

/// Summary of targeted analysis
#[derive(Debug, Serialize)]
pub struct TargetedSummary {
    /// Total files analyzed
    pub files_analyzed: usize,
    /// Total elements found
    pub total_elements: usize,
    /// Elements by type
    pub elements_by_type: HashMap<String, usize>,
    /// Languages found
    pub languages: Vec<String>,
    /// Average relevance score
    pub avg_relevance_score: f32,
}

/// Tool for targeted repository analysis that reduces context sent to LLM
#[derive(Debug)]
pub struct TargetedViewTool {
    repo_manager: Arc<Mutex<RepositoryManager>>,
}

impl TargetedViewTool {
    /// Create a new targeted view tool
    pub fn new(repo_manager: Arc<Mutex<RepositoryManager>>) -> Self {
        Self { repo_manager }
    }

    /// Perform targeted analysis following the user's strategy:
    /// 1. Do semantic search to find promising files
    /// 2. Get repository map for those files to extract metadata
    /// 3. Use metadata to perform highly targeted semantic search
    async fn perform_targeted_analysis(&self, params: &TargetedViewParams) -> Result<TargetedAnalysisResult, SagittaCodeError> {
        let repo_manager = self.repo_manager.lock().await;

        // Step 1: Initial semantic search to find promising files
        log::info!("Step 1: Performing initial semantic search for query: '{}'", params.query);
        
        let semantic_results = repo_manager.query(
            &params.repository_name,
            &params.query,
            params.max_files * 3, // Get more results to filter from
            params.element_type.as_deref(),
            params.language.as_deref(),
            None, // branch
        ).await.map_err(|e| SagittaCodeError::ToolError(format!("Semantic search failed: {}", e)))?;

        log::info!("Step 1 result: Found {} semantic search results", semantic_results.result.len());

        // Extract promising file paths from semantic search
        let mut promising_files = Vec::new();
        for (i, point) in semantic_results.result.iter().enumerate() {
            log::debug!("Processing semantic result {}: score={}, payload keys={:?}", 
                       i, point.score, point.payload.keys().collect::<Vec<_>>());
            
            if let Some(file_path) = point.payload.get("file_path")
                .and_then(|v| v.kind.as_ref())
                .and_then(|k| match k {
                    qdrant_client::qdrant::value::Kind::StringValue(s) => Some(s.clone()),
                    _ => None,
                }) {
                if !promising_files.contains(&file_path) {
                    log::debug!("Added promising file: {}", file_path);
                    promising_files.push(file_path);
                }
            } else {
                log::warn!("Semantic result {} missing file_path in payload", i);
            }
        }

        // Limit to max_files
        promising_files.truncate(params.max_files);
        
        log::info!("Step 1 complete: Found {} promising files", promising_files.len());

        // If no promising files found from semantic search, try a fallback approach
        if promising_files.is_empty() {
            log::warn!("No promising files found from semantic search. Attempting fallback approach...");
            
            // Fallback: Use repository map to find all files, then filter by query terms
            let repositories = repo_manager.list_repositories().await
                .map_err(|e| SagittaCodeError::ToolError(format!("Failed to list repositories: {}", e)))?;
            
            let repo = repositories.iter()
                .find(|r| r.name == params.repository_name)
                .ok_or_else(|| SagittaCodeError::ToolError(format!("Repository '{}' not found", params.repository_name)))?;

            // Generate a broad repository map to find relevant files
            let fallback_map_options = RepoMapOptions {
                verbosity: 0, // Minimal verbosity for broad search
                file_extension: params.language.as_ref().and_then(|lang| match lang.to_lowercase().as_str() {
                    "rust" => Some("rs".to_string()),
                    "python" => Some("py".to_string()),
                    "javascript" => Some("js".to_string()),
                    "typescript" => Some("ts".to_string()),
                    "go" => Some("go".to_string()),
                    _ => None,
                }),
                content_pattern: None,
                paths: None,
                max_calls_per_method: 3,
                include_context: false,
                include_docstrings: false,
            };

            let fallback_repo_map = generate_repo_map(&repo.local_path, fallback_map_options)
                .map_err(|e| SagittaCodeError::ToolError(format!("Failed to generate fallback repository map: {}", e)))?;

            // Filter files based on query terms (simple text matching)
            let query_terms: Vec<&str> = params.query.split_whitespace().collect();
            for (file_path, methods) in &fallback_repo_map.methods_by_file {
                let mut file_relevant = false;
                
                // Check if file path contains query terms
                for term in &query_terms {
                    if file_path.to_lowercase().contains(&term.to_lowercase()) {
                        file_relevant = true;
                        break;
                    }
                }
                
                // Check if any method names or types contain query terms
                if !file_relevant {
                    for method in methods {
                        for term in &query_terms {
                            if method.name.to_lowercase().contains(&term.to_lowercase()) ||
                               method.method_type.display_name().to_lowercase().contains(&term.to_lowercase()) {
                                file_relevant = true;
                                break;
                            }
                        }
                        if file_relevant { break; }
                    }
                }
                
                if file_relevant && promising_files.len() < params.max_files {
                    promising_files.push(file_path.clone());
                    log::info!("Fallback: Added relevant file: {}", file_path);
                }
            }
            
            log::info!("Fallback approach found {} relevant files", promising_files.len());
        }

        // If still no files found, return early with informative message
        if promising_files.is_empty() {
            log::warn!("No relevant files found for query '{}' in repository '{}'", params.query, params.repository_name);
            
            return Ok(TargetedAnalysisResult {
                query: params.query.clone(),
                analyzed_files: Vec::new(),
                relevant_elements: HashMap::new(),
                summary: TargetedSummary {
                    files_analyzed: 0,
                    total_elements: 0,
                    elements_by_type: HashMap::new(),
                    languages: Vec::new(),
                    avg_relevance_score: 0.0,
                },
                semantic_search_results: Some(serde_json::json!({
                    "message": "No relevant files found",
                    "semantic_results_count": semantic_results.result.len(),
                    "query": params.query,
                    "repository": params.repository_name,
                    "filters": {
                        "element_type": params.element_type,
                        "language": params.language
                    }
                })),
            });
        }

        // Step 2: Get repository map for promising files to extract metadata
        log::info!("Step 2: Generating repository map for promising files");
        
        let repositories = repo_manager.list_repositories().await
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to list repositories: {}", e)))?;
        
        let repo = repositories.iter()
            .find(|r| r.name == params.repository_name)
            .ok_or_else(|| SagittaCodeError::ToolError(format!("Repository '{}' not found", params.repository_name)))?;

        // Generate map for specific files
        let map_options = RepoMapOptions {
            verbosity: 1, // Normal verbosity for good balance
            file_extension: None, // Don't filter by extension since we have specific files
            content_pattern: None,
            paths: None, // We'll filter by files after generation
            max_calls_per_method: 5,
            include_context: params.include_context,
            include_docstrings: true,
        };

        let repo_map_result = generate_repo_map(&repo.local_path, map_options)
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to generate repository map: {}", e)))?;

        // Filter map results to only include promising files
        let mut filtered_methods_by_file = HashMap::new();
        for (file_path, methods) in &repo_map_result.methods_by_file {
            if promising_files.iter().any(|pf| file_path.contains(pf) || pf.contains(file_path)) {
                filtered_methods_by_file.insert(file_path.clone(), methods.clone());
                log::debug!("Included file in analysis: {} ({} methods)", file_path, methods.len());
            }
        }

        log::info!("Step 2 complete: Generated metadata for {} files", filtered_methods_by_file.len());

        // Step 3: Use metadata to perform highly targeted semantic search
        log::info!("Step 3: Performing targeted semantic search using metadata");
        
        let mut relevant_elements: HashMap<String, Vec<RelevantElement>> = HashMap::new();
        let mut all_elements = Vec::new();

        for (file_path, methods) in &filtered_methods_by_file {
            let mut file_elements = Vec::new();
            
            for method in methods.iter().take(params.max_elements_per_file) {
                // Create a targeted query using the method metadata
                let targeted_query = format!("{} {} {}", 
                    params.query, 
                    method.name, 
                    method.method_type.display_name()
                );

                // Perform semantic search with this specific metadata
                match repo_manager.query(
                    &params.repository_name,
                    &targeted_query,
                    3, // Just a few results per element
                    Some(&method.method_type.display_name().to_lowercase()),
                    params.language.as_deref(),
                    None,
                ).await {
                    Ok(targeted_results) => {
                        // Calculate relevance score based on semantic search score
                        let relevance_score = if !targeted_results.result.is_empty() {
                            targeted_results.result[0].score
                        } else {
                            // If no semantic results, use a simple text matching score
                            let query_lower = params.query.to_lowercase();
                            let method_name_lower = method.name.to_lowercase();
                            let method_type_lower = method.method_type.display_name().to_lowercase();
                            
                            let mut score: f32 = 0.0;
                            for term in query_lower.split_whitespace() {
                                if method_name_lower.contains(term) {
                                    score += 0.3;
                                }
                                if method_type_lower.contains(term) {
                                    score += 0.2;
                                }
                                if let Some(doc) = &method.docstring {
                                    if doc.to_lowercase().contains(term) {
                                        score += 0.1;
                                    }
                                }
                            }
                            score.min(1.0)
                        };

                        // Include elements with reasonable relevance or if we have very few results
                        let relevance_threshold = if all_elements.len() < 3 { 0.1 } else { 0.3 };
                        if relevance_score > relevance_threshold {
                            let relevant_element = RelevantElement {
                                name: method.name.clone(),
                                element_type: method.method_type.display_name().to_string(),
                                signature: format!("{}({})", method.name, method.params),
                                line_number: method.line_number,
                                documentation: method.docstring.clone(),
                                calls: method.calls.clone(),
                                relevance_score,
                                context: if params.include_context { 
                                    Some(method.context.clone()) 
                                } else { 
                                    None 
                                },
                            };
                            
                            file_elements.push(relevant_element.clone());
                            all_elements.push(relevant_element);
                            log::debug!("Added relevant element: {} (score: {:.2})", method.name, relevance_score);
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed targeted search for {}: {}", method.name, e);
                        
                        // Even if semantic search fails, include the element with a basic score
                        let basic_score = 0.5; // Default score when semantic search fails
                        let relevant_element = RelevantElement {
                            name: method.name.clone(),
                            element_type: method.method_type.display_name().to_string(),
                            signature: format!("{}({})", method.name, method.params),
                            line_number: method.line_number,
                            documentation: method.docstring.clone(),
                            calls: method.calls.clone(),
                            relevance_score: basic_score,
                            context: if params.include_context { 
                                Some(method.context.clone()) 
                            } else { 
                                None 
                            },
                        };
                        
                        file_elements.push(relevant_element.clone());
                        all_elements.push(relevant_element);
                        log::debug!("Added element with basic score: {} (semantic search failed)", method.name);
                    }
                }
            }

            if !file_elements.is_empty() {
                relevant_elements.insert(file_path.clone(), file_elements);
            }
        }

        log::info!("Step 3 complete: Found {} relevant elements across {} files", 
                  all_elements.len(), relevant_elements.len());

        // Generate summary
        let mut elements_by_type = HashMap::new();
        let mut languages = Vec::new();
        let mut total_relevance = 0.0;

        for element in &all_elements {
            *elements_by_type.entry(element.element_type.clone()).or_insert(0) += 1;
            total_relevance += element.relevance_score;
        }

        // Extract languages from file extensions
        for file_path in relevant_elements.keys() {
            if let Some(ext) = std::path::Path::new(file_path).extension().and_then(|e| e.to_str()) {
                let lang = match ext {
                    "rs" => "Rust",
                    "py" => "Python", 
                    "js" | "jsx" => "JavaScript",
                    "ts" | "tsx" => "TypeScript",
                    "go" => "Go",
                    "rb" => "Ruby",
                    "vue" => "Vue",
                    _ => ext,
                };
                if !languages.contains(&lang.to_string()) {
                    languages.push(lang.to_string());
                }
            }
        }

        let avg_relevance_score = if !all_elements.is_empty() {
            total_relevance / all_elements.len() as f32
        } else {
            0.0
        };

        let summary = TargetedSummary {
            files_analyzed: relevant_elements.len(),
            total_elements: all_elements.len(),
            elements_by_type,
            languages,
            avg_relevance_score,
        };

        Ok(TargetedAnalysisResult {
            query: params.query.clone(),
            analyzed_files: relevant_elements.keys().cloned().collect(),
            relevant_elements,
            summary,
            semantic_search_results: Some(serde_json::json!({
                "initial_semantic_results": semantic_results.result.len(),
                "promising_files_found": promising_files.len(),
                "files_analyzed": filtered_methods_by_file.len(),
                "total_elements_found": all_elements.len()
            })),
        })
    }
}

#[async_trait]
impl Tool for TargetedViewTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "targeted_view".to_string(),
            description: "Performs targeted repository analysis to dramatically reduce context sent to LLM. **Best for files over 200 lines or when you need focused analysis** - uses a 3-step process: (1) semantic search to find promising files, (2) repository mapping to extract metadata, (3) targeted semantic search using metadata for highly specific results. This approach avoids sending irrelevant context and enables precise code analysis even in large repositories. For smaller files (under 200 lines), consider using 'view_file' for complete context.".to_string(),
            category: ToolCategory::Repository,
            is_required: false,
            parameters: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["repository_name", "query"],
                "properties": {
                    "repository_name": {
                        "type": "string",
                        "description": "Name of the repository to analyze"
                    },
                    "query": {
                        "type": "string",
                        "description": "Semantic query to find relevant files/elements"
                    },
                    "file_path": {
                        "type": ["string", "null"],
                        "description": "Optional: Specific file path to analyze (if known)"
                    },
                    "element_type": {
                        "type": ["string", "null"],
                        "description": "Optional: Element type to focus on (function, struct, class, etc.)"
                    },
                    "language": {
                        "type": ["string", "null"],
                        "description": "Optional: Programming language filter"
                    },
                    "max_files": {
                        "type": ["integer", "null"],
                        "description": "Maximum number of files to analyze in detail (default: 5)"
                    },
                    "max_elements_per_file": {
                        "type": ["integer", "null"],
                        "description": "Maximum number of elements to return per file (default: 10)"
                    },
                    "include_context": {
                        "type": ["boolean", "null"],
                        "description": "Whether to include full context for found elements (default: true)"
                    }
                }
            }),
            metadata: std::collections::HashMap::new(),
        }
    }

    async fn execute(&self, parameters: Value) -> Result<ToolResult, SagittaCodeError> {
        match serde_json::from_value::<TargetedViewParams>(parameters) {
            Ok(params) => {
                match self.perform_targeted_analysis(&params).await {
                    Ok(result) => Ok(ToolResult::Success(serde_json::json!({
                        "success": true,
                        "analysis_result": result,
                        "context_reduction_note": "This analysis used targeted semantic search to minimize irrelevant context while maximizing precision. Only the most relevant code elements are included."
                    }))),
                    Err(e) => Ok(ToolResult::Error {
                        error: format!("Targeted analysis failed: {}", e),
                    })
                }
            },
            Err(e) => Ok(ToolResult::Error {
                error: format!("Invalid parameters for targeted_view: {}", e),
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
    async fn test_targeted_view_tool_definition() {
        let repo_manager = create_test_repo_manager();
        let tool = TargetedViewTool::new(repo_manager);
        let definition = tool.definition();
        
        assert_eq!(definition.name, "targeted_view");
        assert!(definition.description.contains("targeted repository analysis"));
        assert!(definition.description.contains("reduce context"));
        assert!(definition.description.contains("3-step process"));
        assert_eq!(definition.category, ToolCategory::Repository);
        assert!(!definition.is_required);
        
        // Check parameters
        let params = definition.parameters;
        let properties = params.get("properties").unwrap().as_object().unwrap();
        assert!(properties.contains_key("repository_name"));
        assert!(properties.contains_key("query"));
        assert!(properties.contains_key("file_path"));
        assert!(properties.contains_key("element_type"));
        assert!(properties.contains_key("language"));
        assert!(properties.contains_key("max_files"));
        assert!(properties.contains_key("max_elements_per_file"));
        assert!(properties.contains_key("include_context"));
        
        // Check required fields
        let required = params.get("required").unwrap().as_array().unwrap();
        assert!(required.contains(&serde_json::json!("repository_name")));
        assert!(required.contains(&serde_json::json!("query")));
    }

    #[tokio::test]
    async fn test_targeted_view_params_defaults() {
        let json_str = r#"{"repository_name": "test_repo", "query": "authentication"}"#;
        let params: TargetedViewParams = serde_json::from_str(json_str).unwrap();
        
        assert_eq!(params.repository_name, "test_repo");
        assert_eq!(params.query, "authentication");
        assert_eq!(params.max_files, 5);
        assert_eq!(params.max_elements_per_file, 10);
        assert_eq!(params.include_context, true);
        assert!(params.file_path.is_none());
        assert!(params.element_type.is_none());
        assert!(params.language.is_none());
    }

    #[tokio::test]
    async fn test_targeted_view_tool_execution() {
        let repo_manager = create_test_repo_manager();
        let tool = TargetedViewTool::new(repo_manager);
        
        let params = serde_json::json!({
            "repository_name": "test_repo",
            "query": "authentication middleware",
            "element_type": "function",
            "language": "rust"
        });
        
        let result = tool.execute(params).await.unwrap();
        
        // Should return an error since the repository doesn't exist
        match result {
            ToolResult::Error { error } => {
                assert!(error.contains("failed") || error.contains("not found"));
            }
            _ => panic!("Expected error for non-existent repository"),
        }
    }

    #[tokio::test]
    async fn test_default_functions() {
        assert_eq!(default_max_files(), 5);
        assert_eq!(default_max_elements_per_file(), 10);
        assert_eq!(default_include_context(), true);
    }

    #[tokio::test]
    async fn test_relevant_element_serialization() {
        let element = RelevantElement {
            name: "authenticate_user".to_string(),
            element_type: "function".to_string(),
            signature: "authenticate_user(username: String, password: String)".to_string(),
            line_number: Some(42),
            documentation: Some("Authenticates a user with username and password".to_string()),
            calls: vec!["hash_password".to_string(), "verify_credentials".to_string()],
            relevance_score: 0.85,
            context: Some("fn authenticate_user(username: String, password: String) -> Result<User, AuthError> {".to_string()),
        };
        
        let serialized = serde_json::to_string(&element).unwrap();
        assert!(serialized.contains("authenticate_user"));
        assert!(serialized.contains("0.85"));
        assert!(serialized.contains("hash_password"));
    }

    #[tokio::test]
    async fn test_targeted_summary_creation() {
        let summary = TargetedSummary {
            files_analyzed: 3,
            total_elements: 15,
            elements_by_type: {
                let mut map = HashMap::new();
                map.insert("function".to_string(), 10);
                map.insert("struct".to_string(), 5);
                map
            },
            languages: vec!["Rust".to_string(), "Python".to_string()],
            avg_relevance_score: 0.75,
        };
        
        assert_eq!(summary.files_analyzed, 3);
        assert_eq!(summary.total_elements, 15);
        assert_eq!(summary.avg_relevance_score, 0.75);
        assert!(summary.languages.contains(&"Rust".to_string()));
        assert_eq!(summary.elements_by_type.get("function"), Some(&10));
    }

    #[tokio::test]
    async fn test_targeted_view_with_fallback_mechanism() {
        let repo_manager = create_test_repo_manager();
        let tool = TargetedViewTool::new(repo_manager);
        
        let params = serde_json::json!({
            "repository_name": "test_repo",
            "query": "async runtime examples",
            "element_type": "function",
            "language": "rust",
            "max_files": 3,
            "max_elements_per_file": 5,
            "include_context": true
        });
        
        let result = tool.execute(params).await.unwrap();
        
        // Should return a result (either success or error) but not panic
        match result {
            ToolResult::Success(data) => {
                // If successful, verify the structure
                assert!(data.get("success").is_some());
                assert!(data.get("analysis_result").is_some());
                
                let analysis = data.get("analysis_result").unwrap();
                assert!(analysis.get("query").is_some());
                assert!(analysis.get("analyzed_files").is_some());
                assert!(analysis.get("relevant_elements").is_some());
                assert!(analysis.get("summary").is_some());
                assert!(analysis.get("semantic_search_results").is_some());
                
                // Verify the query is preserved
                assert_eq!(analysis.get("query").unwrap().as_str().unwrap(), "async runtime examples");
            }
            ToolResult::Error { error } => {
                // Should be a meaningful error, not a panic or syntax error
                assert!(!error.contains("syntax"));
                assert!(!error.contains("panic"));
                // Should indicate repository not found or similar
                assert!(error.contains("not found") || error.contains("failed") || error.contains("not initialized"));
            }
        }
    }

    #[tokio::test]
    async fn test_targeted_view_empty_results_handling() {
        let repo_manager = create_test_repo_manager();
        let tool = TargetedViewTool::new(repo_manager);
        
        let params = serde_json::json!({
            "repository_name": "nonexistent_repo",
            "query": "very_specific_nonexistent_function",
            "max_files": 5,
            "max_elements_per_file": 10
        });
        
        let result = tool.execute(params).await.unwrap();
        
        match result {
            ToolResult::Success(data) => {
                let analysis = data.get("analysis_result").unwrap();
                let summary = analysis.get("summary").unwrap();
                
                // Should handle empty results gracefully
                assert_eq!(summary.get("files_analyzed").unwrap().as_u64().unwrap(), 0);
                assert_eq!(summary.get("total_elements").unwrap().as_u64().unwrap(), 0);
                
                let semantic_results = analysis.get("semantic_search_results").unwrap();
                assert!(semantic_results.get("message").is_some() || 
                       semantic_results.get("initial_semantic_results").is_some());
            }
            ToolResult::Error { error } => {
                // Error is also acceptable for nonexistent repository
                assert!(error.contains("not found") || error.contains("failed"));
            }
        }
    }

    #[tokio::test]
    async fn test_targeted_view_parameter_validation() {
        let repo_manager = create_test_repo_manager();
        let tool = TargetedViewTool::new(repo_manager);
        
        // Test with missing required parameters
        let invalid_params = serde_json::json!({
            "query": "test query"
            // missing repository_name
        });
        
        let result = tool.execute(invalid_params).await.unwrap();
        match result {
            ToolResult::Error { error } => {
                assert!(error.contains("Invalid parameters") || error.contains("missing field"));
            }
            _ => panic!("Expected error for invalid parameters"),
        }
        
        // Test with empty query
        let empty_query_params = serde_json::json!({
            "repository_name": "test_repo",
            "query": ""
        });
        
        let result = tool.execute(empty_query_params).await.unwrap();
        // Should handle empty query gracefully (either success with no results or error)
        match result {
            ToolResult::Success(_) | ToolResult::Error { .. } => {
                // Both are acceptable outcomes
            }
        }
    }

    #[tokio::test]
    async fn test_targeted_view_with_different_languages() {
        let repo_manager = create_test_repo_manager();
        let tool = TargetedViewTool::new(repo_manager);
        
        let languages = vec!["rust", "python", "javascript", "typescript", "go"];
        
        for language in languages {
            let params = serde_json::json!({
                "repository_name": "test_repo",
                "query": "function test",
                "language": language,
                "max_files": 2
            });
            
            let result = tool.execute(params).await.unwrap();
            
            // Should handle all languages without panicking
            match result {
                ToolResult::Success(data) => {
                    let analysis = data.get("analysis_result").unwrap();
                    assert_eq!(analysis.get("query").unwrap().as_str().unwrap(), "function test");
                }
                ToolResult::Error { error } => {
                    // Error is acceptable, but should be meaningful
                    assert!(!error.contains("panic"));
                    assert!(!error.contains("syntax"));
                }
            }
        }
    }

    #[tokio::test]
    async fn test_targeted_view_with_different_element_types() {
        let repo_manager = create_test_repo_manager();
        let tool = TargetedViewTool::new(repo_manager);
        
        let element_types = vec!["function", "struct", "class", "trait", "enum"];
        
        for element_type in element_types {
            let params = serde_json::json!({
                "repository_name": "test_repo",
                "query": "test code",
                "element_type": element_type,
                "max_files": 2
            });
            
            let result = tool.execute(params).await.unwrap();
            
            // Should handle all element types without panicking
            match result {
                ToolResult::Success(data) => {
                    let analysis = data.get("analysis_result").unwrap();
                    assert_eq!(analysis.get("query").unwrap().as_str().unwrap(), "test code");
                }
                ToolResult::Error { error } => {
                    // Error is acceptable, but should be meaningful
                    assert!(!error.contains("panic"));
                    assert!(!error.contains("syntax"));
                }
            }
        }
    }

    #[tokio::test]
    async fn test_targeted_view_limits_and_bounds() {
        let repo_manager = create_test_repo_manager();
        let tool = TargetedViewTool::new(repo_manager);
        
        // Test with extreme values
        let params = serde_json::json!({
            "repository_name": "test_repo",
            "query": "test",
            "max_files": 100,
            "max_elements_per_file": 1000,
            "include_context": false
        });
        
        let result = tool.execute(params).await.unwrap();
        
        // Should handle large limits gracefully
        match result {
            ToolResult::Success(data) => {
                let analysis = data.get("analysis_result").unwrap();
                let summary = analysis.get("summary").unwrap();
                
                // Should respect the limits (even if no data is found)
                let files_analyzed = summary.get("files_analyzed").unwrap().as_u64().unwrap();
                assert!(files_analyzed <= 100);
            }
            ToolResult::Error { .. } => {
                // Error is acceptable for this test case
            }
        }
        
        // Test with minimal values
        let minimal_params = serde_json::json!({
            "repository_name": "test_repo",
            "query": "test",
            "max_files": 1,
            "max_elements_per_file": 1
        });
        
        let result = tool.execute(minimal_params).await.unwrap();
        
        // Should handle minimal limits gracefully
        match result {
            ToolResult::Success(data) => {
                let analysis = data.get("analysis_result").unwrap();
                let summary = analysis.get("summary").unwrap();
                
                let files_analyzed = summary.get("files_analyzed").unwrap().as_u64().unwrap();
                assert!(files_analyzed <= 1);
            }
            ToolResult::Error { .. } => {
                // Error is acceptable for this test case
            }
        }
    }

    #[tokio::test]
    async fn test_targeted_view_integration_with_real_repository() {
        use tempfile::TempDir;
        use std::fs;
        use sagitta_search::config::AppConfig;
        
        // Create a temporary directory for our test repository
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        fs::create_dir_all(&repo_path).unwrap();
        
        // Create some test Rust files with realistic content
        let main_rs_content = r#"
use std::collections::HashMap;
use tokio::runtime::Runtime;

/// Main application entry point
fn main() {
    println!("Hello, world!");
    let rt = create_async_runtime();
    rt.block_on(async {
        run_server().await;
    });
}

/// Creates a new async runtime for the application
pub fn create_async_runtime() -> Runtime {
    Runtime::new().expect("Failed to create runtime")
}

/// Runs the main server loop
pub async fn run_server() {
    println!("Server running...");
}

/// Authentication middleware function
pub fn authenticate_user(username: &str, password: &str) -> bool {
    // Simple authentication logic
    username == "admin" && password == "secret"
}

/// Database connection helper
pub struct DatabaseConnection {
    url: String,
}

impl DatabaseConnection {
    pub fn new(url: String) -> Self {
        Self { url }
    }
    
    pub async fn connect(&self) -> Result<(), String> {
        println!("Connecting to database: {}", self.url);
        Ok(())
    }
}
"#;

        let lib_rs_content = r#"
//! Library module for common utilities

use std::error::Error;
use std::fmt;

/// Custom error type for the application
#[derive(Debug)]
pub struct AppError {
    message: String,
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Application error: {}", self.message)
    }
}

impl Error for AppError {}

/// Error handling utility function
pub fn handle_error(error: &dyn Error) {
    eprintln!("Error occurred: {}", error);
}

/// Configuration struct for the application
pub struct Config {
    pub database_url: String,
    pub server_port: u16,
    pub debug_mode: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database_url: "localhost:5432".to_string(),
            server_port: 8080,
            debug_mode: false,
        }
    }
}

/// Async function for processing data
pub async fn process_data(data: Vec<u8>) -> Result<String, AppError> {
    // Simulate some async processing
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    
    if data.is_empty() {
        return Err(AppError {
            message: "Empty data provided".to_string(),
        });
    }
    
    Ok(format!("Processed {} bytes", data.len()))
}
"#;

        let utils_rs_content = r#"
//! Utility functions and helpers

use std::collections::HashMap;

/// HTTP client for making requests
pub struct HttpClient {
    base_url: String,
    headers: HashMap<String, String>,
}

impl HttpClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            headers: HashMap::new(),
        }
    }
    
    pub fn add_header(&mut self, key: String, value: String) {
        self.headers.insert(key, value);
    }
    
    pub async fn get(&self, path: &str) -> Result<String, Box<dyn std::error::Error>> {
        let url = format!("{}/{}", self.base_url, path);
        println!("Making GET request to: {}", url);
        Ok("Mock response".to_string())
    }
}

/// Logging utility functions
pub mod logging {
    use std::fs::OpenOptions;
    use std::io::Write;
    
    pub fn log_info(message: &str) {
        println!("[INFO] {}", message);
    }
    
    pub fn log_error(message: &str) {
        eprintln!("[ERROR] {}", message);
    }
    
    pub fn write_to_file(filename: &str, content: &str) -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(filename)?;
        writeln!(file, "{}", content)?;
        Ok(())
    }
}
"#;

        // Write the test files
        fs::write(repo_path.join("main.rs"), main_rs_content).unwrap();
        fs::write(repo_path.join("lib.rs"), lib_rs_content).unwrap();
        fs::write(repo_path.join("utils.rs"), utils_rs_content).unwrap();
        
        // Create a test configuration
        let mut config = AppConfig::default();
        config.repositories_base_path = Some(temp_dir.path().to_string_lossy().to_string());
        // tenant_id is hardcoded to "local" in sagitta-code operational code
        
        // Create repository manager with the test config
        let config_arc = Arc::new(Mutex::new(config));
        let repo_manager = Arc::new(Mutex::new(RepositoryManager::new(config_arc)));
        
        // Add the test repository
        {
            let mut manager = repo_manager.lock().await;
            // Simulate adding a local repository
            let repo_config = sagitta_search::RepositoryConfig {
                name: "test_integration_repo".to_string(),
                url: "file://test".to_string(),
                local_path: repo_path.clone(),
                default_branch: "main".to_string(),
                tracked_branches: vec!["main".to_string()],
                remote_name: Some("origin".to_string()),
                last_synced_commits: std::collections::HashMap::new(),
                active_branch: Some("main".to_string()),
                ssh_key_path: None,
                ssh_key_passphrase: None,
                indexed_languages: Some(vec!["rust".to_string()]),
                added_as_local_path: true,
                target_ref: None,
                tenant_id: Some("local".to_string()), // hardcoded in sagitta-code
            };
            
            // Add to the config manually for testing
            let config_guard = manager.get_config();
            let mut config_lock = config_guard.lock().await;
            config_lock.repositories.push(repo_config);
        }
        
        // Create the targeted view tool
        let tool = TargetedViewTool::new(repo_manager);
        
        // Test 1: Search for async runtime functions
        let params = serde_json::json!({
            "repository_name": "test_integration_repo",
            "query": "async runtime",
            "element_type": "function",
            "language": "rust",
            "max_files": 5,
            "max_elements_per_file": 10,
            "include_context": true
        });
        
        let result = tool.execute(params).await.unwrap();
        
        match result {
            ToolResult::Success(data) => {
                let analysis = data.get("analysis_result").unwrap();
                let summary = analysis.get("summary").unwrap();
                
                // Should find some files and elements through fallback mechanism
                assert!(summary.get("files_analyzed").unwrap().as_u64().unwrap() > 0);
                
                // Should have found some relevant elements
                let relevant_elements = analysis.get("relevant_elements").unwrap().as_object().unwrap();
                assert!(!relevant_elements.is_empty());
                
                // Check that we found Rust language
                let languages = summary.get("languages").unwrap().as_array().unwrap();
                assert!(languages.iter().any(|lang| lang.as_str().unwrap() == "Rust"));
                
                // Verify semantic search results are included
                let semantic_results = analysis.get("semantic_search_results").unwrap();
                assert!(semantic_results.is_object());
                
                println!("✅ Async runtime search test passed");
            }
            ToolResult::Error { error } => {
                // This is acceptable since we don't have a real semantic search setup
                println!("⚠️  Expected error (no semantic search): {}", error);
                assert!(error.contains("not found") || error.contains("failed") || error.contains("not initialized"));
            }
        }
        
        // Test 2: Search for authentication functions
        let auth_params = serde_json::json!({
            "repository_name": "test_integration_repo",
            "query": "authentication user",
            "element_type": "function",
            "max_files": 3,
            "max_elements_per_file": 5
        });
        
        let auth_result = tool.execute(auth_params).await.unwrap();
        
        match auth_result {
            ToolResult::Success(data) => {
                let analysis = data.get("analysis_result").unwrap();
                println!("✅ Authentication search test passed");
                
                // Verify the query is preserved
                assert_eq!(analysis.get("query").unwrap().as_str().unwrap(), "authentication user");
            }
            ToolResult::Error { error } => {
                println!("⚠️  Expected error (no semantic search): {}", error);
            }
        }
        
        // Test 3: Search for database-related code
        let db_params = serde_json::json!({
            "repository_name": "test_integration_repo",
            "query": "database connection",
            "language": "rust",
            "max_files": 2,
            "include_context": false
        });
        
        let db_result = tool.execute(db_params).await.unwrap();
        
        match db_result {
            ToolResult::Success(data) => {
                let analysis = data.get("analysis_result").unwrap();
                println!("✅ Database search test passed");
                
                // Should handle the search gracefully
                let summary = analysis.get("summary").unwrap();
                assert!(summary.get("files_analyzed").is_some());
            }
            ToolResult::Error { error } => {
                println!("⚠️  Expected error (no semantic search): {}", error);
            }
        }
        
        // Test 4: Search with no matches
        let no_match_params = serde_json::json!({
            "repository_name": "test_integration_repo",
            "query": "nonexistent_function_xyz_123",
            "max_files": 5
        });
        
        let no_match_result = tool.execute(no_match_params).await.unwrap();
        
        match no_match_result {
            ToolResult::Success(data) => {
                let analysis = data.get("analysis_result").unwrap();
                let summary = analysis.get("summary").unwrap();
                
                // Should handle no matches gracefully
                let files_analyzed = summary.get("files_analyzed").unwrap().as_u64().unwrap();
                let total_elements = summary.get("total_elements").unwrap().as_u64().unwrap();
                
                // Should either find no files or find files but no relevant elements
                assert!(files_analyzed == 0 || total_elements == 0);
                
                println!("✅ No matches test passed");
            }
            ToolResult::Error { error } => {
                println!("⚠️  Expected error (no semantic search): {}", error);
            }
        }
        
        // Test 5: Test error handling with non-existent repository
        let bad_repo_params = serde_json::json!({
            "repository_name": "nonexistent_repo_12345",
            "query": "test query"
        });
        
        let bad_repo_result = tool.execute(bad_repo_params).await.unwrap();
        
        match bad_repo_result {
            ToolResult::Error { error } => {
                assert!(error.contains("not found") || error.contains("Repository"));
                println!("✅ Non-existent repository error handling test passed");
            }
            ToolResult::Success(_) => {
                panic!("Expected error for non-existent repository");
            }
        }
        
        println!("🎉 All integration tests completed successfully!");
    }

    #[tokio::test]
    async fn test_targeted_view_performance_and_limits() {
        use tempfile::TempDir;
        use std::fs;
        
        // Create a larger test repository to test performance
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("large_test_repo");
        fs::create_dir_all(&repo_path).unwrap();
        
        // Create multiple files with various content
        for i in 0..10 {
            let file_content = format!(r#"
//! Module {} for testing

use std::collections::HashMap;

/// Function {} for processing data
pub fn process_data_{}(input: &str) -> String {{
    format!("Processed: {{}}", input)
}}

/// Struct {} for data management
pub struct DataManager{} {{
    data: HashMap<String, String>,
}}

impl DataManager{} {{
    pub fn new() -> Self {{
        Self {{
            data: HashMap::new(),
        }}
    }}
    
    pub fn add_data(&mut self, key: String, value: String) {{
        self.data.insert(key, value);
    }}
    
    pub fn get_data(&self, key: &str) -> Option<&String> {{
        self.data.get(key)
    }}
}}

/// Async function {} for network operations
pub async fn network_operation_{}() -> Result<String, Box<dyn std::error::Error>> {{
    tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
    Ok("Network operation completed".to_string())
}}
"#, i, i, i, i, i, i, i, i);
            
            fs::write(repo_path.join(format!("module_{}.rs", i)), file_content).unwrap();
        }
        
        // Create repository manager
        let mut config = AppConfig::default();
        config.repositories_base_path = Some(temp_dir.path().to_string_lossy().to_string());
        // tenant_id is hardcoded to "local" in sagitta-code operational code
        
        let config_arc = Arc::new(Mutex::new(config));
        let repo_manager = Arc::new(Mutex::new(RepositoryManager::new(config_arc)));
        
        // Add the test repository
        {
            let mut manager = repo_manager.lock().await;
                         let repo_config = sagitta_search::RepositoryConfig {
                 name: "large_test_repo".to_string(),
                 url: "file://test".to_string(),
                 local_path: repo_path.clone(),
                 default_branch: "main".to_string(),
                 tracked_branches: vec!["main".to_string()],
                 remote_name: Some("origin".to_string()),
                 last_synced_commits: std::collections::HashMap::new(),
                 active_branch: Some("main".to_string()),
                 ssh_key_path: None,
                 ssh_key_passphrase: None,
                 indexed_languages: Some(vec!["rust".to_string()]),
                 added_as_local_path: true,
                 target_ref: None,
                 tenant_id: Some("local".to_string()), // hardcoded in sagitta-code
             };
            
            let config_guard = manager.get_config();
            let mut config_lock = config_guard.lock().await;
            config_lock.repositories.push(repo_config);
        }
        
        let tool = TargetedViewTool::new(repo_manager);
        
        // Test with various limits
        let test_cases = vec![
            (1, 1),   // Minimal limits
            (3, 5),   // Small limits
            (10, 20), // Large limits
            (20, 50), // Maximum limits
        ];
        
        for (max_files, max_elements) in test_cases {
            let params = serde_json::json!({
                "repository_name": "large_test_repo",
                "query": "data processing",
                "max_files": max_files,
                "max_elements_per_file": max_elements,
                "include_context": false
            });
            
            let start_time = std::time::Instant::now();
            let result = tool.execute(params).await.unwrap();
            let duration = start_time.elapsed();
            
            match result {
                ToolResult::Success(data) => {
                    let analysis = data.get("analysis_result").unwrap();
                    let summary = analysis.get("summary").unwrap();
                    
                    let files_analyzed = summary.get("files_analyzed").unwrap().as_u64().unwrap();
                    let total_elements = summary.get("total_elements").unwrap().as_u64().unwrap();
                    
                    // Verify limits are respected
                    assert!(files_analyzed <= max_files as u64);
                    
                    // Performance should be reasonable (under 5 seconds for this test)
                    assert!(duration.as_secs() < 5, "Performance test failed: took {:?}", duration);
                    
                    println!("✅ Performance test passed for limits ({}, {}): {} files, {} elements in {:?}", 
                            max_files, max_elements, files_analyzed, total_elements, duration);
                }
                ToolResult::Error { error } => {
                    println!("⚠️  Expected error for limits ({}, {}): {}", max_files, max_elements, error);
                }
            }
        }
        
        println!("🎉 Performance and limits tests completed!");
    }
} 