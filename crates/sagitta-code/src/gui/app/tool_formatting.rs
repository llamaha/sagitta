// Tool result formatting for the Sagitta Code application

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
        result.push_str("📚 Enhanced Repository List\n\n");
        
        if let Some(repos) = value.get("repositories").and_then(|v| v.as_array()) {
            // Add summary information if available
            if let Some(summary) = value.get("summary") {
                result.push_str("**Summary:**\n");
                if let Some(existing_count) = summary.get("existing_count").and_then(|v| v.as_u64()) {
                    result.push_str(&format!("   📁 Existing repositories: {}\n", existing_count));
                }
                if let Some(needs_sync) = summary.get("needs_sync_count").and_then(|v| v.as_u64()) {
                    result.push_str(&format!("   🔄 Need syncing: {}\n", needs_sync));
                }
                if let Some(dirty_count) = summary.get("dirty_count").and_then(|v| v.as_u64()) {
                    result.push_str(&format!("   ⚠️  With uncommitted changes: {}\n", dirty_count));
                }
                if let Some(total_files) = summary.get("total_files").and_then(|v| v.as_u64()) {
                    result.push_str(&format!("   📊 Total files: {}\n", total_files));
                }
                if let Some(total_size) = summary.get("total_size_bytes").and_then(|v| v.as_u64()) {
                    result.push_str(&format!("   💾 Total size: {}\n", format_bytes(total_size)));
                }
                result.push_str("\n");
            }
            
            result.push_str(&format!("**Found {} repositories:**\n\n", repos.len()));
            
            for (i, repo) in repos.iter().enumerate() {
                if let Some(name) = repo.get("name").and_then(|v| v.as_str()) {
                    result.push_str(&format!("{}. **{}**\n", i + 1, name));
                    
                    // Basic information
                    if let Some(url) = repo.get("url").and_then(|v| v.as_str()) {
                        result.push_str(&format!("   🔗 URL: {}\n", url));
                    }
                    
                    if let Some(path) = repo.get("local_path").and_then(|v| v.as_str()) {
                        result.push_str(&format!("   📁 Path: {}\n", path));
                    }
                    
                    // Branch information
                    if let Some(branch) = repo.get("active_branch").and_then(|v| v.as_str()) {
                        result.push_str(&format!("   🌿 Branch: {}\n", branch));
                    }
                    
                    // Filesystem status
                    if let Some(fs_status) = repo.get("filesystem_status") {
                        let exists = fs_status.get("exists").and_then(|v| v.as_bool()).unwrap_or(false);
                        let is_git = fs_status.get("is_git_repository").and_then(|v| v.as_bool()).unwrap_or(false);
                        
                        let status_text = match (exists, is_git) {
                            (true, true) => "✅ Git repository",
                            (true, false) => "📂 Directory (no git)",
                            (false, _) => "❌ Missing from filesystem",
                        };
                        result.push_str(&format!("   📍 Status: {}\n", status_text));
                        
                        if let Some(file_count) = fs_status.get("total_files").and_then(|v| v.as_u64()) {
                            if let Some(size) = fs_status.get("size_bytes").and_then(|v| v.as_u64()) {
                                result.push_str(&format!("   📊 Files: {} ({})\n", file_count, format_bytes(size)));
                            } else {
                                result.push_str(&format!("   📊 Files: {}\n", file_count));
                            }
                        }
                    }
                    
                    // Sync status
                    if let Some(sync_status) = repo.get("sync_status") {
                        if let Some(state) = sync_status.get("state").and_then(|v| v.as_str()) {
                            let sync_text = match state {
                                "UpToDate" => "✅ Up to date",
                                "NeedsSync" => "🔄 Needs sync",
                                "NeverSynced" => "❌ Never synced",
                                _ => "❓ Unknown",
                            };
                            result.push_str(&format!("   🔄 Sync: {}\n", sync_text));
                        }
                        
                        if let Some(branches_needing_sync) = sync_status.get("branches_needing_sync").and_then(|v| v.as_array()) {
                            if !branches_needing_sync.is_empty() {
                                let branch_names: Vec<String> = branches_needing_sync
                                    .iter()
                                    .filter_map(|b| b.as_str().map(|s| s.to_string()))
                                    .collect();
                                result.push_str(&format!("   ⚠️  Need sync: {}\n", branch_names.join(", ")));
                            }
                        }
                    }
                    
                    // Git status
                    if let Some(git_status) = repo.get("git_status") {
                        if let Some(commit) = git_status.get("current_commit").and_then(|v| v.as_str()) {
                            let short_commit = if commit.len() >= 8 { &commit[..8] } else { commit };
                            let is_clean = git_status.get("is_clean").and_then(|v| v.as_bool()).unwrap_or(true);
                            let clean_text = if is_clean { "clean" } else { "dirty" };
                            result.push_str(&format!("   📍 Commit: {} ({})\n", short_commit, clean_text));
                        }
                    }
                    
                    // Languages
                    if let Some(languages) = repo.get("indexed_languages").and_then(|v| v.as_array()) {
                        if !languages.is_empty() {
                            let lang_names: Vec<String> = languages
                                .iter()
                                .filter_map(|l| l.as_str().map(|s| s.to_string()))
                                .collect();
                            result.push_str(&format!("   🔤 Languages: {}\n", lang_names.join(", ")));
                        }
                    }
                    
                    // File extensions (top 3)
                    if let Some(extensions) = repo.get("file_extensions").and_then(|v| v.as_array()) {
                        if !extensions.is_empty() {
                            let ext_strs: Vec<String> = extensions
                                .iter()
                                .take(3)
                                .filter_map(|ext| {
                                    let name = ext.get("extension").and_then(|v| v.as_str())?;
                                    let count = ext.get("count").and_then(|v| v.as_u64())?;
                                    Some(format!("{} ({})", name, count))
                                })
                                .collect();
                            if !ext_strs.is_empty() {
                                result.push_str(&format!("   📄 Extensions: {}\n", ext_strs.join(", ")));
                            }
                        }
                    }
                    
                    result.push('\n');
                }
            }
            
            // Active repository
            if let Some(active) = value.get("active_repository").and_then(|v| v.as_str()) {
                result.push_str(&format!("**Active Repository:** {}\n", active));
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

// Helper function to format bytes
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
} 