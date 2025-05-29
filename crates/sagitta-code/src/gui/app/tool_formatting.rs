// Tool result formatting for the Fred Agent application

/// Tool result formatter for displaying tool outputs in a human-readable way
pub struct ToolResultFormatter {
    // Future: could add configuration options here
}

impl ToolResultFormatter {
    pub fn new() -> Self {
        Self {}
    }

    /// Format tool results in a human-readable way for the preview pane
    pub fn format_tool_result_for_preview(&self, tool_name: &str, result: &crate::tools::types::ToolResult) -> String {
        match result {
            crate::tools::types::ToolResult::Success(value) => {
                self.format_successful_tool_result(tool_name, value)
            },
            crate::tools::types::ToolResult::Error { error } => {
                format!("ERROR\n\n{}", error)
            }
        }
    }
    
    /// Format successful tool results based on tool type
    fn format_successful_tool_result(&self, tool_name: &str, value: &serde_json::Value) -> String {
        match tool_name {
            "web_search" => self.format_web_search_result(value),
            "view_file" | "read_file" => self.format_file_result(value),
            "code_search" => self.format_code_search_result(value),
            "list_repositories" => self.format_repository_list_result(value),
            "add_repository" | "sync_repository" | "remove_repository" => self.format_repository_operation_result(value),
            "search_file_in_repository" => self.format_file_search_result(value),
            "edit" | "semantic_edit" | "validate" => self.format_edit_result(value),
            _ => {
                // Fallback: try to extract key information from any JSON
                self.format_generic_result(value)
            }
        }
    }
    
    /// Format web search results in a human-readable way
    fn format_web_search_result(&self, value: &serde_json::Value) -> String {
        // Check if we have a pre-formatted summary
        if let Some(formatted_summary) = value.get("formatted_summary").and_then(|v| v.as_str()) {
            let mut result = formatted_summary.to_string();
            
            // Add extracted actionable information if available
            if let Some(extracted_info) = value.get("extracted_info") {
                result.push_str("\n\n---\n\n");
                result.push_str("🤖 **Extracted Information for Agent:**\n\n");
                
                // Git repositories
                if let Some(git_repos) = extracted_info.get("git_repositories").and_then(|v| v.as_array()) {
                    result.push_str("**Git Repositories:**\n");
                    for repo in git_repos {
                        if let Some(url) = repo.get("url").and_then(|v| v.as_str()) {
                            result.push_str(&format!("• {}", url));
                            if let Some(clone_url) = repo.get("clone_url").and_then(|v| v.as_str()) {
                                result.push_str(&format!(" (clone: {})", clone_url));
                            }
                            if let Some(repo_type) = repo.get("type").and_then(|v| v.as_str()) {
                                result.push_str(&format!(" [{}]", repo_type));
                            }
                            result.push('\n');
                        }
                    }
                    result.push('\n');
                }
                
                // Default branch
                if let Some(branch) = extracted_info.get("default_branch").and_then(|v| v.as_str()) {
                    result.push_str(&format!("**Default Branch:** {}\n\n", branch));
                }
                
                // Documentation
                if let Some(docs) = extracted_info.get("documentation").and_then(|v| v.as_array()) {
                    result.push_str("**Documentation:**\n");
                    for doc in docs {
                        if let Some(url) = doc.get("url").and_then(|v| v.as_str()) {
                            result.push_str(&format!("• {}\n", url));
                        }
                    }
                    result.push('\n');
                }
                
                // Installation commands
                if let Some(commands) = extracted_info.get("installation_commands").and_then(|v| v.as_array()) {
                    result.push_str("**Installation Commands:**\n");
                    for cmd in commands {
                        if let Some(command) = cmd.as_str() {
                            result.push_str(&format!("• `{}`\n", command));
                        }
                    }
                    result.push('\n');
                }
                
                // Versions
                if let Some(versions) = extracted_info.get("versions").and_then(|v| v.as_array()) {
                    result.push_str("**Versions Found:**\n");
                    for version in versions {
                        if let Some(v) = version.as_str() {
                            result.push_str(&format!("• {}\n", v));
                        }
                    }
                    result.push('\n');
                }
            }
            
            return result;
        }
        
        // Fallback to manual formatting
        let mut result = String::new();
        result.push_str("🔍 **Web Search Results**\n\n");
        
        if let Some(query) = value.get("query").and_then(|v| v.as_str()) {
            result.push_str(&format!("**Query:** {}\n\n", query));
        }
        
        if let Some(answer) = value.get("answer").and_then(|v| v.as_str()) {
            result.push_str("**Answer:**\n");
            result.push_str(answer);
            result.push_str("\n\n");
        } else if let Some(response) = value.get("response").and_then(|v| v.as_str()) {
            result.push_str("**Response:**\n");
            result.push_str(response);
            result.push_str("\n\n");
        }
        
        if let Some(sources) = value.get("sources").and_then(|v| v.as_array()) {
            if !sources.is_empty() {
                result.push_str("**Sources:**\n");
                for (i, source) in sources.iter().enumerate() {
                    if let Some(title) = source.get("title").and_then(|v| v.as_str()) {
                        result.push_str(&format!("{}. **{}**\n", i + 1, title));
                        if let Some(url) = source.get("url").and_then(|v| v.as_str()) {
                            result.push_str(&format!("   {}\n", url));
                        } else if let Some(uri) = source.get("uri").and_then(|v| v.as_str()) {
                            // Clean up the URI if it's a redirect
                            let clean_uri = if uri.contains("grounding-api-redirect") {
                                "Source URL (via search)"
                            } else {
                                uri
                            };
                            result.push_str(&format!("   {}\n", clean_uri));
                        }
                        result.push('\n');
                    }
                }
            }
        }
        
        if let Some(grounded) = value.get("grounded").and_then(|v| v.as_bool()) {
            result.push_str(&format!("*Search was {}*\n", if grounded { "grounded with web results" } else { "not grounded" }));
        }
        
        result
    }
    
    /// Format file operation results
    fn format_file_result(&self, value: &serde_json::Value) -> String {
        let mut result = String::new();
        result.push_str("FILE: File Content\n\n");
        
        if let Some(file_path) = value.get("file_path").and_then(|v| v.as_str()) {
            result.push_str(&format!("**File:** {}\n\n", file_path));
        }
        
        if let Some(repo_name) = value.get("repository_name").and_then(|v| v.as_str()) {
            result.push_str(&format!("**Repository:** {}\n\n", repo_name));
        }
        
        if let Some(file_type) = value.get("file_type").and_then(|v| v.as_str()) {
            result.push_str(&format!("**Type:** {}\n\n", file_type));
        }
        
        if let Some(start_line) = value.get("start_line") {
            if let Some(end_line) = value.get("end_line") {
                result.push_str(&format!("**Lines:** {} - {}\n\n", start_line, end_line));
            }
        }
        
        if let Some(content) = value.get("content").and_then(|v| v.as_str()) {
            result.push_str("**Content:**\n");
            result.push_str("```\n");
            result.push_str(content);
            result.push_str("\n```\n");
        }
        
        result
    }
    
    /// Format code search results
    fn format_code_search_result(&self, value: &serde_json::Value) -> String {
        let mut result = String::new();
        result.push_str("SEARCH: Code Search Results\n\n");
        
        if let Some(query) = value.get("query").and_then(|v| v.as_str()) {
            result.push_str(&format!("**Query:** {}\n\n", query));
        }
        
        if let Some(results) = value.get("results").and_then(|v| v.as_array()) {
            result.push_str(&format!("**Found {} results:**\n\n", results.len()));
            
            for (i, search_result) in results.iter().enumerate() {
                if let Some(file_path) = search_result.get("file_path").and_then(|v| v.as_str()) {
                    result.push_str(&format!("{}. **{}**\n", i + 1, file_path));
                    
                    if let Some(repo) = search_result.get("repository").and_then(|v| v.as_str()) {
                        result.push_str(&format!("   Repository: {}\n", repo));
                    }
                    
                    if let Some(score) = search_result.get("score").and_then(|v| v.as_f64()) {
                        result.push_str(&format!("   Relevance: {:.1}%\n", score * 100.0));
                    }
                    
                    if let Some(snippet) = search_result.get("snippet").and_then(|v| v.as_str()) {
                        result.push_str("   Preview:\n");
                        result.push_str("   ```\n");
                        // Limit snippet length
                        let limited_snippet = if snippet.len() > 200 {
                            format!("{}...", &snippet[..200])
                        } else {
                            snippet.to_string()
                        };
                        result.push_str(&format!("   {}\n", limited_snippet));
                        result.push_str("   ```\n");
                    }
                    
                    result.push('\n');
                }
            }
        }
        
        result
    }
    
    /// Format repository list results
    fn format_repository_list_result(&self, value: &serde_json::Value) -> String {
        let mut result = String::new();
        result.push_str("📚 Repository List\n\n");
        
        if let Some(repos) = value.get("repositories").and_then(|v| v.as_array()) {
            result.push_str(&format!("**Found {} repositories:**\n\n", repos.len()));
            
            for (i, repo) in repos.iter().enumerate() {
                if let Some(name) = repo.get("name").and_then(|v| v.as_str()) {
                    result.push_str(&format!("{}. **{}**\n", i + 1, name));
                    
                    if let Some(url) = repo.get("url").and_then(|v| v.as_str()) {
                        result.push_str(&format!("   URL: {}\n", url));
                    }
                    
                    if let Some(branch) = repo.get("branch").and_then(|v| v.as_str()) {
                        result.push_str(&format!("   Branch: {}\n", branch));
                    }
                    
                    if let Some(status) = repo.get("status").and_then(|v| v.as_str()) {
                        result.push_str(&format!("   Status: {}\n", status));
                    }
                    
                    result.push('\n');
                }
            }
        }
        
        result
    }
    
    /// Format repository operation results
    fn format_repository_operation_result(&self, value: &serde_json::Value) -> String {
        let mut result = String::new();
        result.push_str("📦 Repository Operation\n\n");
        
        if let Some(message) = value.get("message").and_then(|v| v.as_str()) {
            result.push_str(&format!("**Result:** {}\n\n", message));
        }
        
        if let Some(repo_name) = value.get("repository_name").and_then(|v| v.as_str()) {
            result.push_str(&format!("**Repository:** {}\n\n", repo_name));
        }
        
        if let Some(details) = value.get("details").and_then(|v| v.as_str()) {
            result.push_str(&format!("**Details:** {}\n\n", details));
        }
        
        result
    }
    
    /// Format file search results
    fn format_file_search_result(&self, value: &serde_json::Value) -> String {
        let mut result = String::new();
        result.push_str("📁 File Search Results\n\n");
        
        if let Some(pattern) = value.get("pattern").and_then(|v| v.as_str()) {
            result.push_str(&format!("**Pattern:** {}\n\n", pattern));
        }
        
        if let Some(files) = value.get("files").and_then(|v| v.as_array()) {
            result.push_str(&format!("**Found {} files:**\n\n", files.len()));
            
            for (i, file) in files.iter().enumerate() {
                if let Some(file_path) = file.as_str() {
                    result.push_str(&format!("{}. {}\n", i + 1, file_path));
                }
            }
        }
        
        result
    }
    
    /// Format edit operation results
    fn format_edit_result(&self, value: &serde_json::Value) -> String {
        let mut result = String::new();
        result.push_str("✏️ Edit Operation\n\n");
        
        if let Some(message) = value.get("message").and_then(|v| v.as_str()) {
            result.push_str(&format!("**Result:** {}\n\n", message));
        }
        
        if let Some(file_path) = value.get("file_path").and_then(|v| v.as_str()) {
            result.push_str(&format!("**File:** {}\n\n", file_path));
        }
        
        if let Some(changes) = value.get("changes_made").and_then(|v| v.as_str()) {
            result.push_str(&format!("**Changes:** {}\n\n", changes));
        }
        
        result
    }
    
    /// Format generic tool results
    fn format_generic_result(&self, value: &serde_json::Value) -> String {
        let mut result = String::new();
        result.push_str("RESULT: Tool Result\n\n");
        
        // Try to extract meaningful information from the JSON
        if let Some(obj) = value.as_object() {
            for (key, val) in obj {
                // Skip very long values or binary data
                let val_str = match val {
                    serde_json::Value::String(s) => {
                        if s.len() > 200 {
                            format!("{}...", &s[..197])
                        } else {
                            s.clone()
                        }
                    },
                    serde_json::Value::Array(arr) => {
                        format!("Array with {} items", arr.len())
                    },
                    serde_json::Value::Object(obj) => {
                        format!("Object with {} fields", obj.len())
                    },
                    _ => val.to_string(),
                };
                
                result.push_str(&format!("**{}:** {}\n", key, val_str));
            }
        } else {
            // If it's not an object, just show the value
            let val_str = value.to_string();
            if val_str.len() > 500 {
                result.push_str(&format!("{}...", &val_str[..497]));
            } else {
                result.push_str(&val_str);
            }
        }
        
        // Check if result is still just the header
        if result == "RESULT: Tool Result\n\n" {
            result.push_str("No detailed information available.");
        }
        
        result
    }
} 