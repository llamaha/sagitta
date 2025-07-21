// Tool result formatting for the Sagitta Code application

/// Tool result formatter for displaying tool outputs in a human-readable way
pub struct ToolResultFormatter {
    // Future: could add configuration options here
}

impl Default for ToolResultFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolResultFormatter {
    pub fn new() -> Self {
        Self {}
    }

    /// Format tool results in a human-readable way for the preview pane
    pub fn format_tool_result_for_preview(&self, tool_name: &str, result: &crate::agent::events::ToolResult) -> String {
        use crate::agent::events::ToolResult;
        match result {
            ToolResult::Success { output } => {
                let value: serde_json::Value = serde_json::from_str(output).unwrap_or(serde_json::Value::String(output.clone()));
                // Check if this is actually an error wrapped as success (from MCP)
                if let Some(obj) = value.as_object() {
                    if obj.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false) {
                        let error_msg = obj.get("error")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Tool execution failed");
                        return format!("❌ **ERROR**\n\n{error_msg}");
                    }
                }
                self.format_successful_tool_result(tool_name, &value)
            },
            ToolResult::Error { error } => {
                // Format error with better structure
                let mut result = String::new();
                result.push_str("❌ **Tool Execution Failed**\n\n");
                
                // Try to parse error as JSON for better formatting
                if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(error) {
                    if let Some(message) = error_json.get("message").and_then(|v| v.as_str()) {
                        result.push_str(&format!("**Error:** {message}\n"));
                    } else {
                        result.push_str(&format!("**Error:** {error}\n"));
                    }
                    
                    // Add any additional error details
                    if let Some(code) = error_json.get("code") {
                        result.push_str(&format!("\n**Error Code:** {code}"));
                    }
                } else {
                    // Plain text error
                    result.push_str(&format!("**Error:** {error}"));
                }
                
                result
            }
        }
    }
    
    /// Format successful tool results based on tool type
    fn format_successful_tool_result(&self, tool_name: &str, value: &serde_json::Value) -> String {
        log::debug!("Formatting tool result for: {tool_name}");
        
        // Handle both native tools and MCP tools
        match tool_name {
            "web_search" | "WebSearch" => self.format_web_search_result(value),
            "view_file" | "read_file" | "Read" => self.format_file_result(value),
            "code_search" => self.format_code_search_result(value),
            "list_repositories" => self.format_repository_list_result(value),
            "add_existing_repository" | "sync_repository" | "remove_repository" => self.format_repository_operation_result(value),
            "search_file_in_repository" => self.format_file_search_result(value),
            "edit" | "semantic_edit" | "validate" | "Edit" | "MultiEdit" => self.format_edit_result(value),
            "Bash" => self.format_bash_result(value),
            "TodoWrite" => self.format_todo_result(value),
            "Task" => self.format_task_agent_result(value),
            name if name.contains("__todo_read") => self.format_mcp_todo_read_result(value),
            name if name.contains("__todo_write") => self.format_mcp_todo_write_result(value),
            name if name.contains("__edit_file") => self.format_mcp_edit_file_result(value),
            name if name.contains("__multi_edit_file") => self.format_mcp_multi_edit_file_result(value),
            name if name.contains("__shell_execute") => self.format_mcp_shell_execute_result(value),
            name if name.contains("__read_file") => self.format_mcp_read_file_result(value),
            name if name.contains("__write_file") => self.format_mcp_write_file_result(value),
            name if name.contains("__repository_view_file") => self.format_mcp_file_view_result(value),
            name if name.contains("__query") => self.format_mcp_search_result(value),
            name if name.contains("__repository_map") => self.format_mcp_repo_map_result(value),
            name if name.contains("__repository_list_branches") => self.format_mcp_repo_list_branches_result(value),
            name if name.contains("__repository_list") => self.format_mcp_repo_list_result(value),
            name if name.contains("__repository_search_file") => self.format_mcp_file_search_result(value),
            name if name.contains("__web_search") => self.format_web_search_result(value),
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
                            result.push_str(&format!("• {url}"));
                            if let Some(clone_url) = repo.get("clone_url").and_then(|v| v.as_str()) {
                                result.push_str(&format!(" (clone: {clone_url})"));
                            }
                            if let Some(repo_type) = repo.get("type").and_then(|v| v.as_str()) {
                                result.push_str(&format!(" [{repo_type}]"));
                            }
                            result.push('\n');
                        }
                    }
                    result.push('\n');
                }
                
                // Default branch
                if let Some(branch) = extracted_info.get("default_branch").and_then(|v| v.as_str()) {
                    result.push_str(&format!("**Default Branch:** {branch}\n\n"));
                }
                
                // Documentation
                if let Some(docs) = extracted_info.get("documentation").and_then(|v| v.as_array()) {
                    result.push_str("**Documentation:**\n");
                    for doc in docs {
                        if let Some(url) = doc.get("url").and_then(|v| v.as_str()) {
                            result.push_str(&format!("• {url}\n"));
                        }
                    }
                    result.push('\n');
                }
                
                // Installation commands
                if let Some(commands) = extracted_info.get("installation_commands").and_then(|v| v.as_array()) {
                    result.push_str("**Installation Commands:**\n");
                    for cmd in commands {
                        if let Some(command) = cmd.as_str() {
                            result.push_str(&format!("• `{command}`\n"));
                        }
                    }
                    result.push('\n');
                }
                
                // Versions
                if let Some(versions) = extracted_info.get("versions").and_then(|v| v.as_array()) {
                    result.push_str("**Versions Found:**\n");
                    for version in versions {
                        if let Some(v) = version.as_str() {
                            result.push_str(&format!("• {v}\n"));
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
            result.push_str(&format!("**Query:** {query}\n\n"));
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
                            result.push_str(&format!("   {url}\n"));
                        } else if let Some(uri) = source.get("uri").and_then(|v| v.as_str()) {
                            // Clean up the URI if it's a redirect
                            let clean_uri = if uri.contains("grounding-api-redirect") {
                                "Source URL (via search)"
                            } else {
                                uri
                            };
                            result.push_str(&format!("   {clean_uri}\n"));
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
        
        let mut file_path_for_lang = None;
        
        if let Some(file_path) = value.get("file_path").and_then(|v| v.as_str()) {
            result.push_str(&format!("**File:** {file_path}\n\n"));
            file_path_for_lang = Some(file_path);
        }
        
        if let Some(repo_name) = value.get("repository_name").and_then(|v| v.as_str()) {
            result.push_str(&format!("**Repository:** {repo_name}\n\n"));
        }
        
        if let Some(file_type) = value.get("file_type").and_then(|v| v.as_str()) {
            result.push_str(&format!("**Type:** {file_type}\n\n"));
        }
        
        if let Some(start_line) = value.get("start_line") {
            if let Some(end_line) = value.get("end_line") {
                result.push_str(&format!("**Lines:** {start_line} - {end_line}\n\n"));
            }
        }
        
        if let Some(content) = value.get("content").and_then(|v| v.as_str()) {
            result.push_str("**Content:**\n");
            
            // Detect language from file extension for syntax highlighting
            let language = file_path_for_lang
                .and_then(|path| std::path::Path::new(path).extension())
                .and_then(|ext| ext.to_str())
                .map(|ext| {
                    // Map common extensions to their language identifiers
                    match ext {
                        "ts" | "tsx" => "typescript",
                        "js" | "jsx" => "javascript",
                        "rs" => "rust",
                        "py" => "python",
                        "rb" => "ruby",
                        "go" => "go",
                        "java" => "java",
                        "cpp" | "cc" | "cxx" => "cpp",
                        "c" => "c",
                        "h" | "hpp" => "cpp",
                        "cs" => "csharp",
                        "php" => "php",
                        "swift" => "swift",
                        "kt" => "kotlin",
                        "scala" => "scala",
                        "r" => "r",
                        "sh" | "bash" => "bash",
                        "yaml" | "yml" => "yaml",
                        "json" => "json",
                        "xml" => "xml",
                        "html" | "htm" => "html",
                        "css" => "css",
                        "scss" | "sass" => "scss",
                        "sql" => "sql",
                        "md" | "markdown" => "markdown",
                        "toml" => "toml",
                        "ini" => "ini",
                        "dockerfile" => "dockerfile",
                        "makefile" => "makefile",
                        _ => ext // Use the extension as-is for other cases
                    }
                })
                .unwrap_or("");
            
            result.push_str(&format!("```{language}\n"));
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
            result.push_str(&format!("**Query:** {query}\n\n"));
        }
        
        if let Some(results) = value.get("results").and_then(|v| v.as_array()) {
            result.push_str(&format!("**Found {} results:**\n\n", results.len()));
            
            for (i, search_result) in results.iter().enumerate() {
                if let Some(file_path) = search_result.get("file_path").and_then(|v| v.as_str()) {
                    result.push_str(&format!("{}. **{}**\n", i + 1, file_path));
                    
                    if let Some(repo) = search_result.get("repository").and_then(|v| v.as_str()) {
                        result.push_str(&format!("   Repository: {repo}\n"));
                    }
                    
                    if let Some(score) = search_result.get("score").and_then(|v| v.as_f64()) {
                        result.push_str(&format!("   Relevance: {:.1}%\n", score * 100.0));
                    }
                    
                    if let Some(snippet) = search_result.get("snippet").and_then(|v| v.as_str()) {
                        result.push_str("   Preview:\n");
                        
                        // Detect language from file path for syntax highlighting
                        let language = search_result.get("file_path")
                            .and_then(|v| v.as_str())
                            .and_then(|path| std::path::Path::new(path).extension())
                            .and_then(|ext| ext.to_str())
                            .map(|ext| {
                                match ext {
                                    "ts" | "tsx" => "typescript",
                                    "js" | "jsx" => "javascript",
                                    "rs" => "rust",
                                    "py" => "python",
                                    "rb" => "ruby",
                                    "go" => "go",
                                    "java" => "java",
                                    "cpp" | "cc" | "cxx" => "cpp",
                                    "c" => "c",
                                    "h" | "hpp" => "cpp",
                                    "cs" => "csharp",
                                    "php" => "php",
                                    "swift" => "swift",
                                    "kt" => "kotlin",
                                    "scala" => "scala",
                                    "r" => "r",
                                    "sh" | "bash" => "bash",
                                    "yaml" | "yml" => "yaml",
                                    "json" => "json",
                                    "xml" => "xml",
                                    "html" | "htm" => "html",
                                    "css" => "css",
                                    "scss" | "sass" => "scss",
                                    "sql" => "sql",
                                    "md" | "markdown" => "markdown",
                                    "toml" => "toml",
                                    "ini" => "ini",
                                    "dockerfile" => "dockerfile",
                                    "makefile" => "makefile",
                                    _ => ext
                                }
                            })
                            .unwrap_or("");
                        
                        result.push_str(&format!("   ```{language}\n"));
                        // Limit snippet length
                        let limited_snippet = if snippet.len() > 200 {
                            format!("{}...", &snippet[..200])
                        } else {
                            snippet.to_string()
                        };
                        result.push_str(&format!("   {limited_snippet}\n"));
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
                    result.push_str(&format!("   📁 Existing repositories: {existing_count}\n"));
                }
                if let Some(needs_sync) = summary.get("needs_sync_count").and_then(|v| v.as_u64()) {
                    result.push_str(&format!("   🔄 Need syncing: {needs_sync}\n"));
                }
                if let Some(dirty_count) = summary.get("dirty_count").and_then(|v| v.as_u64()) {
                    result.push_str(&format!("   ⚠️  With uncommitted changes: {dirty_count}\n"));
                }
                if let Some(total_files) = summary.get("total_files").and_then(|v| v.as_u64()) {
                    result.push_str(&format!("   📊 Total files: {total_files}\n"));
                }
                if let Some(total_size) = summary.get("total_size_bytes").and_then(|v| v.as_u64()) {
                    result.push_str(&format!("   💾 Total size: {}\n", format_bytes(total_size)));
                }
                result.push('\n');
            }
            
            result.push_str(&format!("**Found {} repositories:**\n\n", repos.len()));
            
            for (i, repo) in repos.iter().enumerate() {
                if let Some(name) = repo.get("name").and_then(|v| v.as_str()) {
                    result.push_str(&format!("{}. **{}**\n", i + 1, name));
                    
                    // Basic information
                    if let Some(url) = repo.get("url").and_then(|v| v.as_str()) {
                        result.push_str(&format!("   🔗 URL: {url}\n"));
                    }
                    
                    if let Some(path) = repo.get("local_path").and_then(|v| v.as_str()) {
                        result.push_str(&format!("   📁 Path: {path}\n"));
                    }
                    
                    // Branch information
                    if let Some(branch) = repo.get("active_branch").and_then(|v| v.as_str()) {
                        result.push_str(&format!("   🌿 Branch: {branch}\n"));
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
                        result.push_str(&format!("   📍 Status: {status_text}\n"));
                        
                        if let Some(file_count) = fs_status.get("total_files").and_then(|v| v.as_u64()) {
                            if let Some(size) = fs_status.get("size_bytes").and_then(|v| v.as_u64()) {
                                result.push_str(&format!("   📊 Files: {} ({})\n", file_count, format_bytes(size)));
                            } else {
                                result.push_str(&format!("   📊 Files: {file_count}\n"));
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
                            result.push_str(&format!("   🔄 Sync: {sync_text}\n"));
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
                            result.push_str(&format!("   📍 Commit: {short_commit} ({clean_text})\n"));
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
                                    Some(format!("{name} ({count})"))
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
                result.push_str(&format!("**Active Repository:** {active}\n"));
            }
        }
        
        result
    }
    
    /// Format repository operation results
    fn format_repository_operation_result(&self, value: &serde_json::Value) -> String {
        let mut result = String::new();
        result.push_str("📦 Repository Operation\n\n");
        
        if let Some(message) = value.get("message").and_then(|v| v.as_str()) {
            result.push_str(&format!("**Result:** {message}\n\n"));
        }
        
        if let Some(repo_name) = value.get("repository_name").and_then(|v| v.as_str()) {
            result.push_str(&format!("**Repository:** {repo_name}\n\n"));
        }
        
        if let Some(details) = value.get("details").and_then(|v| v.as_str()) {
            result.push_str(&format!("**Details:** {details}\n\n"));
        }
        
        result
    }
    
    /// Format file search results
    fn format_file_search_result(&self, value: &serde_json::Value) -> String {
        let mut result = String::new();
        result.push_str("📁 File Search Results\n\n");
        
        if let Some(pattern) = value.get("pattern").and_then(|v| v.as_str()) {
            result.push_str(&format!("**Pattern:** {pattern}\n\n"));
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
            result.push_str(&format!("**Result:** {message}\n\n"));
        }
        
        if let Some(file_path) = value.get("file_path").and_then(|v| v.as_str()) {
            result.push_str(&format!("**File:** {file_path}\n\n"));
        }
        
        if let Some(changes) = value.get("changes_made").and_then(|v| v.as_str()) {
            result.push_str(&format!("**Changes:** {changes}\n\n"));
        }
        
        result
    }
    
    /// Format generic tool results
    /// Format MCP file view results
    fn format_mcp_file_view_result(&self, value: &serde_json::Value) -> String {
        let mut result = String::new();
        
        let mut file_path_for_lang = None;
        
        if let Some(file_path) = value.get("relativePath").and_then(|v| v.as_str()) {
            result.push_str(&format!("📄 **{file_path}**\n\n"));
            file_path_for_lang = Some(file_path);
            log::debug!("MCP file view: Found relativePath: {file_path}");
        }
        
        // Show the actual file content
        if let Some(content) = value.get("content").and_then(|v| v.as_str()) {
            if content.is_empty() {
                result.push_str("*Empty file*\n");
            } else {
                // Detect language from file extension for syntax highlighting
                let extension = file_path_for_lang
                    .and_then(|path| std::path::Path::new(path).extension())
                    .and_then(|ext| ext.to_str());
                
                log::debug!("MCP file view: Detected extension: {extension:?}");
                
                let language = extension
                    .map(|ext| {
                        // Map common extensions to their language identifiers
                        let lang = match ext {
                            // Try using shorter identifiers that might be more compatible
                            "ts" | "tsx" => "ts",  // Try 'ts' instead of 'typescript'
                            "js" | "jsx" => "js",  // Try 'js' instead of 'javascript'
                            "rs" => "rust",  // Keep rust as-is since it works
                            "py" => "python",
                            "rb" => "ruby",
                            "go" => "go",
                            "java" => "java",
                            "cpp" | "cc" | "cxx" => "cpp",
                            "c" => "c",
                            "h" | "hpp" => "cpp",
                            "cs" => "csharp",
                            "php" => "php",
                            "swift" => "swift",
                            "kt" => "kotlin",
                            "scala" => "scala",
                            "r" => "r",
                            "sh" | "bash" => "bash",
                            "yaml" | "yml" => "yaml",
                            "json" => "json",
                            "xml" => "xml",
                            "html" | "htm" => "html",
                            "css" => "css",
                            "scss" | "sass" => "scss",
                            "sql" => "sql",
                            "md" | "markdown" => "md",  // Try 'md' instead of 'markdown'
                            "toml" => "toml",
                            "ini" => "ini",
                            "dockerfile" => "dockerfile",
                            "makefile" => "makefile",
                            _ => ext // Use the extension as-is for other cases
                        };
                        log::debug!("MCP file view: Mapped {ext} to language: {lang}");
                        lang
                    })
                    .unwrap_or("");
                
                log::debug!("MCP file view: Using language identifier: '{language}'");
                result.push_str(&format!("```{language}\n{content}\n```\n"));
            }
        }
        
        result
    }
    
    /// Format MCP search results
    fn format_mcp_search_result(&self, value: &serde_json::Value) -> String {
        let mut result = String::new();
        
        if let Some(results) = value.get("results").and_then(|v| v.as_array()) {
            result.push_str(&format!("🔍 **Found {} results**\n\n", results.len()));
            
            for (i, item) in results.iter().enumerate() {
                if i > 0 {
                    result.push_str("\n---\n\n");
                }
                
                // Extract file path and line range - fields are in camelCase
                if let Some(file_path) = item.get("filePath").and_then(|v| v.as_str()) {
                    result.push_str(&format!("**{file_path}**"));
                    
                    // Show line range
                    let start_line = item.get("startLine").and_then(|v| v.as_i64()).unwrap_or(0);
                    let end_line = item.get("endLine").and_then(|v| v.as_i64()).unwrap_or(start_line);
                    
                    if start_line > 0 {
                        if end_line > start_line {
                            result.push_str(&format!(":{start_line}-{end_line}"));
                        } else {
                            result.push_str(&format!(":{start_line}"));
                        }
                    }
                    
                    // Show score if available
                    if let Some(score) = item.get("score").and_then(|v| v.as_f64()) {
                        result.push_str(&format!(" (score: {score:.2})"));
                    }
                    
                    result.push('\n');
                }
                
                // Show preview if available
                if let Some(preview) = item.get("preview").and_then(|v| v.as_str()) {
                    // Detect language from file path for syntax highlighting
                    let language = item.get("filePath")
                        .and_then(|v| v.as_str())
                        .and_then(|path| std::path::Path::new(path).extension())
                        .and_then(|ext| ext.to_str())
                        .map(|ext| {
                            match ext {
                                "ts" | "tsx" => "typescript",
                                "js" | "jsx" => "javascript",
                                "rs" => "rust",
                                "py" => "python",
                                "rb" => "ruby",
                                "go" => "go",
                                "java" => "java",
                                "cpp" | "cc" | "cxx" => "cpp",
                                "c" => "c",
                                "h" | "hpp" => "cpp",
                                "cs" => "csharp",
                                "php" => "php",
                                "swift" => "swift",
                                "kt" => "kotlin",
                                "scala" => "scala",
                                "r" => "r",
                                "sh" | "bash" => "bash",
                                "yaml" | "yml" => "yaml",
                                "json" => "json",
                                "xml" => "xml",
                                "html" | "htm" => "html",
                                "css" => "css",
                                "scss" | "sass" => "scss",
                                "sql" => "sql",
                                "md" | "markdown" => "markdown",
                                "toml" => "toml",
                                "ini" => "ini",
                                "dockerfile" => "dockerfile",
                                "makefile" => "makefile",
                                _ => ext
                            }
                        })
                        .unwrap_or("");
                    
                    result.push_str(&format!("```{language}\n{preview}\n```\n"));
                } else if let Some(content) = item.get("content").and_then(|v| v.as_str()) {
                    // Fall back to full content if available
                    if content != "[Content not included]" {
                        let language = item.get("filePath")
                            .and_then(|v| v.as_str())
                            .and_then(|path| std::path::Path::new(path).extension())
                            .and_then(|ext| ext.to_str())
                            .map(|ext| {
                                match ext {
                                    "ts" | "tsx" => "typescript",
                                    "js" | "jsx" => "javascript",
                                    "rs" => "rust",
                                    "py" => "python",
                                    "rb" => "ruby",
                                    "go" => "go",
                                    _ => ext
                                }
                            })
                            .unwrap_or("");
                        
                        result.push_str(&format!("```{}\n{}\n```\n", language, content.trim()));
                    }
                }
            }
        } else {
            result.push_str("No results found.\n");
        }
        
        result
    }
    
    /// Format MCP repository map results
    fn format_mcp_repo_map_result(&self, value: &serde_json::Value) -> String {
        let mut result = String::new();
        
        if let Some(map_content) = value.get("mapContent").and_then(|v| v.as_str()) {
            result.push_str(map_content);
        } else if let Some(content) = value.get("content").and_then(|v| v.as_str()) {
            result.push_str(content);
        } else {
            result.push_str("No map content available.\n");
        }
        
        result
    }
    
    /// Format MCP repository list results
    fn format_mcp_repo_list_result(&self, value: &serde_json::Value) -> String {
        let mut result = String::new();
        
        if let Some(repos) = value.get("repositories").and_then(|v| v.as_array()) {
            result.push_str(&format!("📚 **{} repositories**\n\n", repos.len()));
            
            for repo in repos {
                if let Some(name) = repo.get("name").and_then(|v| v.as_str()) {
                    result.push_str(&format!("• **{name}**"));
                    
                    if let Some(branch) = repo.get("activeBranch").and_then(|v| v.as_str()) {
                        result.push_str(&format!(" ({branch})"));
                    }
                    
                    result.push('\n');
                    
                    if let Some(path) = repo.get("path").and_then(|v| v.as_str()) {
                        result.push_str(&format!("  📁 {path}\n"));
                    }
                }
            }
        } else {
            result.push_str("No repositories found.\n");
        }
        
        result
    }
    
    /// Format MCP repository list branches results
    fn format_mcp_repo_list_branches_result(&self, value: &serde_json::Value) -> String {
        let mut result = String::new();
        
        // Get repository name if available
        if let Some(repo_name) = value.get("repositoryName").and_then(|v| v.as_str()) {
            result.push_str(&format!("🌳 **Branches for `{repo_name}`**\n\n"));
        } else {
            result.push_str("🌳 **Repository Branches**\n\n");
        }
        
        // Format branches
        if let Some(branches) = value.get("branches").and_then(|v| v.as_array()) {
            if !branches.is_empty() {
                result.push_str("**Branches:**\n");
                for branch in branches {
                    if let Some(name) = branch.get("name").and_then(|v| v.as_str()) {
                        let is_current = branch.get("current").and_then(|v| v.as_bool()).unwrap_or(false);
                        let prefix = if is_current { "• **" } else { "• " };
                        let suffix = if is_current { "** (current)" } else { "" };
                        result.push_str(&format!("{prefix}{name}{suffix}"));
                        
                        // Add last commit info if available
                        if let Some(last_commit) = branch.get("lastCommit") {
                            if let Some(message) = last_commit.get("message").and_then(|v| v.as_str()) {
                                let first_line = message.lines().next().unwrap_or(message);
                                result.push_str(&format!(" - {}", first_line));
                            }
                        }
                        result.push('\n');
                    }
                }
                result.push('\n');
            }
        }
        
        // Format tags
        if let Some(tags) = value.get("tags").and_then(|v| v.as_array()) {
            if !tags.is_empty() {
                result.push_str("**Tags:**\n");
                for tag in tags {
                    if let Some(tag_name) = tag.as_str() {
                        result.push_str(&format!("• {tag_name}\n"));
                    }
                }
            }
        }
        
        if result.trim().ends_with("**Repository Branches**") || result.trim().ends_with('*') {
            result.push_str("No branches or tags found.\n");
        }
        
        result
    }
    
    /// Format MCP file search results
    fn format_mcp_file_search_result(&self, value: &serde_json::Value) -> String {
        let mut result = String::new();
        
        // Extract the matchingFiles array
        if let Some(files) = value.get("matchingFiles").and_then(|v| v.as_array()) {
            result.push_str(&format!("📁 **Found {} matching files:**\n\n", files.len()));
            
            // Format as a nicely indented JSON array
            result.push_str("```json\n[\n");
            for (i, file) in files.iter().enumerate() {
                if let Some(file_path) = file.as_str() {
                    result.push_str(&format!("  \"{file_path}\""));
                    if i < files.len() - 1 {
                        result.push(',');
                    }
                    result.push('\n');
                }
            }
            result.push_str("]\n```\n");
        } else {
            result.push_str("No matching files found.\n");
        }
        
        result
    }
    
    /// Format bash command results
    fn format_bash_result(&self, value: &serde_json::Value) -> String {
        let mut result = String::new();
        
        if let Some(stdout) = value.get("stdout").and_then(|v| v.as_str()) {
            if !stdout.is_empty() {
                result.push_str(&format!("```\n{stdout}\n```\n"));
            }
        }
        
        if let Some(stderr) = value.get("stderr").and_then(|v| v.as_str()) {
            if !stderr.is_empty() {
                result.push_str(&format!("\n**Error output:**\n```\n{stderr}\n```\n"));
            }
        }
        
        if let Some(exit_code) = value.get("exit_code").and_then(|v| v.as_i64()) {
            if exit_code != 0 {
                result.push_str(&format!("\n**Exit code:** {exit_code}\n"));
            }
        }
        
        if result.is_empty() {
            result.push_str("*Command completed successfully*\n");
        }
        
        result
    }
    
    /// Format todo list updates
    fn format_todo_result(&self, value: &serde_json::Value) -> String {
        // Try to parse the result string if it's a simple message
        if let Some(message) = value.as_str() {
            if message.contains("modified successfully") {
                return "✅ Todo list updated".to_string();
            }
            return message.to_string();
        }
        
        // Otherwise show what changed
        let mut result = String::new();
        result.push_str("✅ **Todo List Updated**\n\n");
        
        // Try to extract todo items if available
        if let Some(todos) = value.get("todos").and_then(|v| v.as_array()) {
            for todo in todos {
                if let Some(content) = todo.get("content").and_then(|v| v.as_str()) {
                    let status = todo.get("status").and_then(|v| v.as_str()).unwrap_or("pending");
                    let priority = todo.get("priority").and_then(|v| v.as_str()).unwrap_or("medium");
                    
                    let status_icon = match status {
                        "completed" => "✅",
                        "in_progress" => "🔄",
                        _ => "⬜"
                    };
                    
                    let priority_icon = match priority {
                        "high" => "🔴",
                        "low" => "🟢",
                        _ => "🟡"
                    };
                    
                    result.push_str(&format!("{status_icon} {priority_icon} {content}\n"));
                }
            }
        }
        
        result
    }
    
    /// Format Task agent results properly handling newlines
    fn format_task_agent_result(&self, value: &serde_json::Value) -> String {
        // If the value is a string (common for Task agent output), unescape it
        if let Some(str_value) = value.as_str() {
            // Replace escaped newlines with actual newlines
            return str_value
                .replace("\\n", "\n")
                .replace("\\\"", "\"")
                .replace("\\\\", "\\");
        }
        
        // If it's an object with a result field
        if let Some(result_str) = value.get("result").and_then(|v| v.as_str()) {
            return result_str
                .replace("\\n", "\n")
                .replace("\\\"", "\"")
                .replace("\\\\", "\\");
        }
        
        // Fallback to generic formatting
        self.format_generic_result(value)
    }
    
    /// Format MCP todo_read results
    fn format_mcp_todo_read_result(&self, value: &serde_json::Value) -> String {
        let mut result = String::new();
        
        // Add summary if available
        if let Some(summary) = value.get("summary").and_then(|v| v.as_str()) {
            result.push_str(&format!("📋 **{summary}**\n\n"));
        }
        
        // Format todos
        if let Some(todos) = value.get("todos").and_then(|v| v.as_array()) {
            for todo in todos {
                if let Some(content) = todo.get("content").and_then(|v| v.as_str()) {
                    let status = todo.get("status").and_then(|v| v.as_str()).unwrap_or("pending");
                    let priority = todo.get("priority").and_then(|v| v.as_str()).unwrap_or("medium");
                    
                    let status_icon = match status {
                        "completed" => "✅",
                        "in_progress" => "🔄",
                        _ => "⬜"
                    };
                    
                    let priority_indicator = match priority {
                        "high" => "🔴",
                        "low" => "🟢",
                        _ => "🟡"
                    };
                    
                    result.push_str(&format!("{status_icon} {priority_indicator} {content}\n"));
                }
            }
        }
        
        result
    }
    
    /// Format MCP todo_write results
    fn format_mcp_todo_write_result(&self, value: &serde_json::Value) -> String {
        let mut result = String::new();
        result.push_str("✅ **Todo List Updated**\n\n");
        
        if let Some(summary) = value.get("summary").and_then(|v| v.as_str()) {
            result.push_str(summary);
        }
        
        result
    }
    
    /// Format MCP edit_file results
    fn format_mcp_edit_file_result(&self, value: &serde_json::Value) -> String {
        let mut result = String::new();
        
        if let Some(file_path) = value.get("file_path").and_then(|v| v.as_str()) {
            result.push_str(&format!("📝 **Edited:** `{file_path}`\n\n"));
        }
        
        if let Some(summary) = value.get("changes_summary").and_then(|v| v.as_str()) {
            result.push_str(&format!("{summary}\n\n"));
        }
        
        // The diff will be shown in the special diff rendering
        if value.get("diff").is_some() {
            result.push_str("*Diff view available in the details*");
        }
        
        result
    }
    
    /// Format MCP multi_edit_file results
    fn format_mcp_multi_edit_file_result(&self, value: &serde_json::Value) -> String {
        let mut result = String::new();
        
        if let Some(file_path) = value.get("file_path").and_then(|v| v.as_str()) {
            result.push_str(&format!("📝 **Multi-edited:** `{file_path}`\n\n"));
        }
        
        if let Some(edits_applied) = value.get("edits_applied").and_then(|v| v.as_u64()) {
            result.push_str(&format!("Applied {edits_applied} edits\n"));
        }
        
        if let Some(summary) = value.get("changes_summary").and_then(|v| v.as_str()) {
            result.push_str(&format!("{summary}\n\n"));
        }
        
        // The diff will be shown in the special diff rendering
        if value.get("diff").is_some() {
            result.push_str("*Diff view available in the details*");
        }
        
        result
    }
    
    /// Format MCP shell_execute results
    fn format_mcp_shell_execute_result(&self, value: &serde_json::Value) -> String {
        // This will be handled by the terminal output renderer
        // Just provide a summary here
        let mut result = String::new();
        
        if let Some(command) = value.get("command").and_then(|v| v.as_str()) {
            result.push_str(&format!("🖥️ **Command:** `{command}`\n"));
        }
        
        if let Some(exit_code) = value.get("exit_code").and_then(|v| v.as_i64()) {
            let status = if exit_code == 0 { "✅ Success" } else { "❌ Failed" };
            result.push_str(&format!("**Status:** {status} (exit code: {exit_code})\n"));
        }
        
        if let Some(timed_out) = value.get("timed_out").and_then(|v| v.as_bool()) {
            if timed_out {
                result.push_str("⏱️ **Command timed out**\n");
            }
        }
        
        result.push_str("\n*Terminal output available in the details*");
        result
    }
    
    /// Format MCP read_file results
    fn format_mcp_read_file_result(&self, value: &serde_json::Value) -> String {
        let mut result = String::new();
        
        let mut file_path_for_lang = None;
        
        if let Some(file_path) = value.get("file_path").and_then(|v| v.as_str()) {
            result.push_str(&format!("📄 **File:** `{file_path}`\n"));
            file_path_for_lang = Some(file_path);
        }
        
        if let Some(line_count) = value.get("line_count").and_then(|v| v.as_u64()) {
            result.push_str(&format!("**Lines:** {line_count}"));
        }
        
        if let Some(file_size) = value.get("file_size").and_then(|v| v.as_u64()) {
            result.push_str(&format!(" | **Size:** {file_size} bytes\n"));
        } else {
            result.push('\n');
        }
        
        // Check if we're reading a specific range
        if let (Some(start), Some(end)) = (
            value.get("start_line").and_then(|v| v.as_u64()),
            value.get("end_line").and_then(|v| v.as_u64())
        ) {
            result.push_str(&format!("**Range:** Lines {start}-{end}\n"));
        }
        
        // Detect language from file extension for syntax highlighting
        let language = file_path_for_lang
            .and_then(|path| std::path::Path::new(path).extension())
            .and_then(|ext| ext.to_str())
            .map(|ext| {
                // Map common extensions to their language identifiers
                match ext {
                    "ts" | "tsx" => "typescript",
                    "js" | "jsx" => "javascript",
                    "rs" => "rust",
                    "py" => "python",
                    "rb" => "ruby",
                    "go" => "go",
                    "java" => "java",
                    "cpp" | "cc" | "cxx" => "cpp",
                    "c" => "c",
                    "h" | "hpp" => "cpp",
                    "cs" => "csharp",
                    "php" => "php",
                    "swift" => "swift",
                    "kt" => "kotlin",
                    "scala" => "scala",
                    "r" => "r",
                    "sh" | "bash" => "bash",
                    "yaml" | "yml" => "yaml",
                    "json" => "json",
                    "xml" => "xml",
                    "html" | "htm" => "html",
                    "css" => "css",
                    "scss" | "sass" => "scss",
                    "sql" => "sql",
                    "md" | "markdown" => "markdown",
                    "toml" => "toml",
                    "ini" => "ini",
                    "dockerfile" => "dockerfile",
                    "makefile" => "makefile",
                    _ => ext // Use the extension as-is for other cases
                }
            })
            .unwrap_or("");
        
        result.push_str(&format!("\n```{language}\n"));
        if let Some(content) = value.get("content").and_then(|v| v.as_str()) {
            result.push_str(content);
        }
        result.push_str("\n```");
        
        result
    }
    
    /// Format MCP write_file results
    fn format_mcp_write_file_result(&self, value: &serde_json::Value) -> String {
        let mut result = String::new();
        
        let file_path = value.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
        
        if !file_path.is_empty() {
            let action = if value.get("created").and_then(|v| v.as_bool()).unwrap_or(false) {
                "✨ **Created:**"
            } else {
                "💾 **Updated:**"
            };
            result.push_str(&format!("{action} `{file_path}`\n"));
        }
        
        if let Some(bytes) = value.get("bytes_written").and_then(|v| v.as_u64()) {
            result.push_str(&format!("**Size:** {} bytes\n", bytes));
        }
        
        // Display the file content if available
        if let Some(content) = value.get("content").and_then(|v| v.as_str()) {
            // Detect language from file extension
            let language = file_path.split('.').last()
                .and_then(|ext| match ext {
                    "rs" => Some("rust"),
                    "py" => Some("python"),
                    "js" | "mjs" | "cjs" => Some("javascript"),
                    "ts" | "tsx" => Some("typescript"),
                    "jsx" => Some("jsx"),
                    "java" => Some("java"),
                    "c" => Some("c"),
                    "cpp" | "cc" | "cxx" => Some("cpp"),
                    "h" | "hpp" => Some("cpp"),
                    "cs" => Some("csharp"),
                    "go" => Some("go"),
                    "rb" => Some("ruby"),
                    "php" => Some("php"),
                    "swift" => Some("swift"),
                    "kt" | "kts" => Some("kotlin"),
                    "scala" => Some("scala"),
                    "r" => Some("r"),
                    "m" => Some("matlab"),
                    "sql" => Some("sql"),
                    "sh" | "bash" => Some("bash"),
                    "yml" | "yaml" => Some("yaml"),
                    "toml" => Some("toml"),
                    "json" => Some("json"),
                    "xml" => Some("xml"),
                    "html" | "htm" => Some("html"),
                    "css" => Some("css"),
                    "scss" | "sass" => Some("scss"),
                    "less" => Some("less"),
                    "md" | "markdown" => Some("markdown"),
                    "tex" => Some("latex"),
                    "vim" => Some("vim"),
                    "lua" => Some("lua"),
                    "dart" => Some("dart"),
                    "elm" => Some("elm"),
                    "clj" | "cljs" => Some("clojure"),
                    "ex" | "exs" => Some("elixir"),
                    "erl" | "hrl" => Some("erlang"),
                    "fs" | "fsx" | "fsi" => Some("fsharp"),
                    "ml" | "mli" => Some("ocaml"),
                    "pas" | "pp" => Some("pascal"),
                    "pl" | "pm" => Some("perl"),
                    "ps1" => Some("powershell"),
                    "purs" => Some("purescript"),
                    "rkt" => Some("racket"),
                    "scm" => Some("scheme"),
                    "zig" => Some("zig"),
                    "v" => Some("v"),
                    "nim" => Some("nim"),
                    "jl" => Some("julia"),
                    "d" => Some("d"),
                    "hs" => Some("haskell"),
                    _ => None,
                })
                .unwrap_or("");
            
            result.push_str(&format!("\n```{language}\n"));
            result.push_str(content);
            result.push_str("\n```");
        }
        
        result
    }
    
    fn format_generic_result(&self, value: &serde_json::Value) -> String {
        // Pretty-print the JSON directly without any header
        let pretty_json = serde_json::to_string_pretty(value)
            .unwrap_or_else(|_| value.to_string());
        
        // Wrap in markdown code block for proper formatting and selection
        format!("```json\n{}\n```", pretty_json)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::types::ToolResult;
    use serde_json::json;

    #[test]
    fn test_formatter_creation() {
        let formatter = ToolResultFormatter::new();
        // Just ensure it can be created without panicking
        assert!(true);
    }

    #[test]
    fn test_format_tool_result_for_preview_success() {
        let formatter = ToolResultFormatter::new();
        let result = ToolResult::Success { 
            output: json!({
                "message": "Operation completed successfully"
            }).to_string()
        };
        
        let formatted = formatter.format_tool_result_for_preview("test_tool", &result);
        assert!(formatted.contains("Operation completed successfully"));
    }

    #[test]
    fn test_format_tool_result_for_preview_error() {
        let formatter = ToolResultFormatter::new();
        let result = ToolResult::Error {
            error: "Something went wrong".to_string(),
        };
        
        let formatted = formatter.format_tool_result_for_preview("test_tool", &result);
        assert!(formatted.contains("❌"));
        assert!(formatted.contains("Tool Execution Failed"));
        assert!(formatted.contains("Error:"));
        assert!(formatted.contains("Something went wrong"));
    }

    #[test]
    fn test_format_web_search_result_with_formatted_summary() {
        let formatter = ToolResultFormatter::new();
        let value = json!({
            "formatted_summary": "Here are the search results for Rust programming",
            "extracted_info": {
                "git_repositories": [
                    {
                        "url": "https://github.com/rust-lang/rust",
                        "clone_url": "git@github.com:rust-lang/rust.git",
                        "type": "official"
                    }
                ],
                "default_branch": "master",
                "documentation": [
                    {
                        "url": "https://doc.rust-lang.org/"
                    }
                ],
                "installation_commands": ["curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"],
                "versions": ["1.70.0", "1.69.0"]
            }
        });
        
        let formatted = formatter.format_web_search_result(&value);
        assert!(formatted.contains("Here are the search results"));
        assert!(formatted.contains("Git Repositories"));
        assert!(formatted.contains("https://github.com/rust-lang/rust"));
        assert!(formatted.contains("**Default Branch:** master"));
        assert!(formatted.contains("Documentation"));
        assert!(formatted.contains("Installation Commands"));
        assert!(formatted.contains("Versions Found"));
    }

    #[test]
    fn test_format_web_search_result_fallback() {
        let formatter = ToolResultFormatter::new();
        let value = json!({
            "query": "rust programming",
            "answer": "Rust is a systems programming language",
            "sources": [
                {
                    "title": "Rust Programming Language",
                    "url": "https://www.rust-lang.org/"
                }
            ],
            "grounded": true
        });
        
        let formatted = formatter.format_web_search_result(&value);
        assert!(formatted.contains("🔍 **Web Search Results**"));
        assert!(formatted.contains("**Query:** rust programming"));
        assert!(formatted.contains("**Answer:**"));
        assert!(formatted.contains("Rust is a systems programming language"));
        assert!(formatted.contains("**Sources:**"));
        assert!(formatted.contains("Rust Programming Language"));
        assert!(formatted.contains("grounded with web results"));
    }

    #[test]
    fn test_format_file_result() {
        let formatter = ToolResultFormatter::new();
        let value = json!({
            "file_path": "src/main.rs",
            "repository_name": "my-project",
            "file_type": "rust",
            "start_line": 1,
            "end_line": 10,
            "content": "fn main() {\n    println!(\"Hello, world!\");\n}"
        });
        
        let formatted = formatter.format_file_result(&value);
        assert!(formatted.contains("FILE: File Content"));
        assert!(formatted.contains("**File:** src/main.rs"));
        assert!(formatted.contains("**Repository:** my-project"));
        assert!(formatted.contains("**Type:** rust"));
        assert!(formatted.contains("**Lines:** 1 - 10"));
        assert!(formatted.contains("**Content:**"));
        assert!(formatted.contains("fn main()"));
        assert!(formatted.contains("```"));
    }

    #[test]
    fn test_format_code_search_result() {
        let formatter = ToolResultFormatter::new();
        let value = json!({
            "query": "fn main",
            "results": [
                {
                    "file_path": "src/main.rs",
                    "repository": "my-project",
                    "score": 0.95,
                    "snippet": "fn main() {\n    println!(\"Hello, world!\");\n}"
                },
                {
                    "file_path": "examples/hello.rs",
                    "repository": "my-project",
                    "score": 0.85,
                    "snippet": "fn main() {\n    println!(\"Hello from example!\");\n}"
                }
            ]
        });
        
        let formatted = formatter.format_code_search_result(&value);
        assert!(formatted.contains("SEARCH: Code Search Results"));
        assert!(formatted.contains("**Query:** fn main"));
        assert!(formatted.contains("**Found 2 results:**"));
        assert!(formatted.contains("1. **src/main.rs**"));
        assert!(formatted.contains("2. **examples/hello.rs**"));
        assert!(formatted.contains("Repository: my-project"));
        assert!(formatted.contains("Relevance: 95.0%"));
        assert!(formatted.contains("Relevance: 85.0%"));
        assert!(formatted.contains("Preview:"));
    }

    #[test]
    fn test_format_code_search_result_with_long_snippet() {
        let formatter = ToolResultFormatter::new();
        let long_snippet = "a".repeat(250); // Create a snippet longer than 200 chars
        let value = json!({
            "query": "test",
            "results": [
                {
                    "file_path": "test.rs",
                    "snippet": long_snippet
                }
            ]
        });
        
        let formatted = formatter.format_code_search_result(&value);
        assert!(formatted.contains("test.rs"));
        assert!(formatted.contains("..."));
        // Should be truncated to 200 chars + "..."
        let lines: Vec<&str> = formatted.lines().collect();
        let snippet_line = lines.iter().find(|line| line.contains("aaa")).unwrap();
        assert!(snippet_line.len() < long_snippet.len() + 10); // Much shorter than original
    }

    #[test]
    fn test_format_repository_list_result() {
        let formatter = ToolResultFormatter::new();
        let value = json!({
            "summary": {
                "existing_count": 5,
                "needs_sync_count": 2,
                "dirty_count": 1,
                "total_files": 1000,
                "total_size_bytes": 1048576
            },
            "repositories": [
                {
                    "name": "my-project",
                    "url": "https://github.com/user/my-project",
                    "local_path": "/home/user/projects/my-project",
                    "active_branch": "main",
                    "filesystem_status": {
                        "exists": true,
                        "is_git_repository": true,
                        "total_files": 100,
                        "size_bytes": 204800
                    },
                    "sync_status": {
                        "state": "UpToDate",
                        "branches_needing_sync": []
                    },
                    "git_status": {
                        "current_commit": "abc123def456",
                        "is_clean": true
                    },
                    "indexed_languages": ["rust", "toml"],
                    "file_extensions": [
                        {"extension": ".rs", "count": 50},
                        {"extension": ".toml", "count": 5},
                        {"extension": ".md", "count": 3}
                    ]
                }
            ],
            "active_repository": "my-project"
        });
        
        let formatted = formatter.format_repository_list_result(&value);
        assert!(formatted.contains("📚 Enhanced Repository List"));
        assert!(formatted.contains("**Summary:**"));
        assert!(formatted.contains("📁 Existing repositories: 5"));
        assert!(formatted.contains("🔄 Need syncing: 2"));
        assert!(formatted.contains("⚠️  With uncommitted changes: 1"));
        assert!(formatted.contains("📊 Total files: 1000"));
        assert!(formatted.contains("💾 Total size: 1.0 MB"));
        assert!(formatted.contains("**Found 1 repositories:**"));
        assert!(formatted.contains("1. **my-project**"));
        assert!(formatted.contains("🔗 URL: https://github.com/user/my-project"));
        assert!(formatted.contains("📁 Path: /home/user/projects/my-project"));
        assert!(formatted.contains("🌿 Branch: main"));
        assert!(formatted.contains("📍 Status: ✅ Git repository"));
        assert!(formatted.contains("📊 Files: 100 (200.0 KB)"));
        assert!(formatted.contains("🔄 Sync: ✅ Up to date"));
        assert!(formatted.contains("📍 Commit: abc123de (clean)"));
        assert!(formatted.contains("🔤 Languages: rust, toml"));
        assert!(formatted.contains("📄 Extensions: .rs (50), .toml (5), .md (3)"));
        assert!(formatted.contains("**Active Repository:** my-project"));
    }

    #[test]
    fn test_format_repository_list_result_with_sync_needed() {
        let formatter = ToolResultFormatter::new();
        let value = json!({
            "repositories": [
                {
                    "name": "outdated-project",
                    "filesystem_status": {
                        "exists": false,
                        "is_git_repository": false
                    },
                    "sync_status": {
                        "state": "NeedsSync",
                        "branches_needing_sync": ["main", "develop"]
                    },
                    "git_status": {
                        "current_commit": "xyz789abc123",
                        "is_clean": false
                    }
                }
            ]
        });
        
        let formatted = formatter.format_repository_list_result(&value);
        assert!(formatted.contains("1. **outdated-project**"));
        assert!(formatted.contains("📍 Status: ❌ Missing from filesystem"));
        assert!(formatted.contains("🔄 Sync: 🔄 Needs sync"));
        assert!(formatted.contains("⚠️  Need sync: main, develop"));
        assert!(formatted.contains("📍 Commit: xyz789ab (dirty)"));
    }

    #[test]
    fn test_format_repository_operation_result() {
        let formatter = ToolResultFormatter::new();
        let value = json!({
            "message": "Repository successfully added",
            "repository_name": "new-project",
            "details": "Cloned from GitHub and indexed 150 files"
        });
        
        let formatted = formatter.format_repository_operation_result(&value);
        assert!(formatted.contains("📦 Repository Operation"));
        assert!(formatted.contains("**Result:** Repository successfully added"));
        assert!(formatted.contains("**Repository:** new-project"));
        assert!(formatted.contains("**Details:** Cloned from GitHub and indexed 150 files"));
    }

    #[test]
    fn test_format_file_search_result() {
        let formatter = ToolResultFormatter::new();
        let value = json!({
            "pattern": "*.rs",
            "files": [
                "src/main.rs",
                "src/lib.rs",
                "tests/integration_test.rs"
            ]
        });
        
        let formatted = formatter.format_file_search_result(&value);
        assert!(formatted.contains("📁 File Search Results"));
        assert!(formatted.contains("**Pattern:** *.rs"));
        assert!(formatted.contains("**Found 3 files:**"));
        assert!(formatted.contains("1. src/main.rs"));
        assert!(formatted.contains("2. src/lib.rs"));
        assert!(formatted.contains("3. tests/integration_test.rs"));
    }

    #[test]
    fn test_format_edit_result() {
        let formatter = ToolResultFormatter::new();
        let value = json!({
            "message": "File successfully edited",
            "file_path": "src/main.rs",
            "changes_made": "Added new function 'hello_world'"
        });
        
        let formatted = formatter.format_edit_result(&value);
        assert!(formatted.contains("✏️ Edit Operation"));
        assert!(formatted.contains("**Result:** File successfully edited"));
        assert!(formatted.contains("**File:** src/main.rs"));
        assert!(formatted.contains("**Changes:** Added new function 'hello_world'"));
    }

    #[test]
    fn test_format_generic_result_object() {
        let formatter = ToolResultFormatter::new();
        let value = json!({
            "status": "success",
            "count": 42,
            "items": ["item1", "item2", "item3"],
            "metadata": {
                "timestamp": "2023-01-01T00:00:00Z",
                "version": "1.0"
            }
        });
        
        let formatted = formatter.format_generic_result(&value);
        // Now we expect pretty-printed JSON in a markdown code block
        assert!(formatted.starts_with("```json\n"));
        assert!(formatted.ends_with("\n```"));
        assert!(formatted.contains("\"status\": \"success\""));
        assert!(formatted.contains("\"count\": 42"));
        assert!(formatted.contains("\"items\": [\n    \"item1\",\n    \"item2\",\n    \"item3\"\n  ]"));
        assert!(formatted.contains("\"metadata\": {\n    \"timestamp\": \"2023-01-01T00:00:00Z\",\n    \"version\": \"1.0\"\n  }"));
    }

    #[test]
    fn test_format_generic_result_long_string() {
        let formatter = ToolResultFormatter::new();
        let long_string = "a".repeat(250);
        let value = json!({
            "long_field": long_string
        });
        
        let formatted = formatter.format_generic_result(&value);
        // Should be pretty-printed JSON in markdown code block
        assert!(formatted.starts_with("```json\n"));
        assert!(formatted.ends_with("\n```"));
        assert!(formatted.contains("long_field"));
        // Should contain the full string (no truncation in JSON output)
        assert!(formatted.contains(&long_string));
    }

    #[test]
    fn test_format_generic_result_non_object() {
        let formatter = ToolResultFormatter::new();
        let value = json!("Simple string result");
        
        let formatted = formatter.format_generic_result(&value);
        // Should be pretty-printed JSON in markdown code block
        assert!(formatted.starts_with("```json\n"));
        assert!(formatted.ends_with("\n```"));
        assert!(formatted.contains("Simple string result"));
    }

    #[test]
    fn test_format_generic_result_empty_object() {
        let formatter = ToolResultFormatter::new();
        let value = json!({});
        
        let formatted = formatter.format_generic_result(&value);
        // Empty object should still be pretty-printed JSON
        assert!(formatted.starts_with("```json\n"));
        assert!(formatted.ends_with("\n```"));
        assert!(formatted.contains("{}"));
    }

    #[test]
    fn test_format_generic_result_very_long_non_object() {
        let formatter = ToolResultFormatter::new();
        let very_long_string = "x".repeat(600);
        let value = json!(very_long_string);
        
        let formatted = formatter.format_generic_result(&value);
        // Should be pretty-printed JSON, not truncated
        assert!(formatted.starts_with("```json\n"));
        assert!(formatted.ends_with("\n```"));
        assert!(formatted.contains(&very_long_string)); // Should contain the full string
    }

    #[test]
    fn test_format_successful_tool_result_routing() {
        let formatter = ToolResultFormatter::new();
        
        // Test different tool types get routed to correct formatters
        let web_search_result = json!({"query": "test"});
        let formatted = formatter.format_successful_tool_result("web_search", &web_search_result);
        assert!(formatted.contains("🔍 **Web Search Results**"));
        
        let file_result = json!({"file_path": "test.rs"});
        let formatted = formatter.format_successful_tool_result("view_file", &file_result);
        assert!(formatted.contains("FILE: File Content"));
        
        let code_search_result = json!({"query": "test"});
        let formatted = formatter.format_successful_tool_result("code_search", &code_search_result);
        assert!(formatted.contains("SEARCH: Code Search Results"));
        
        let repo_list_result = json!({"repositories": []});
        let formatted = formatter.format_successful_tool_result("list_repositories", &repo_list_result);
        assert!(formatted.contains("📚 Enhanced Repository List"));
        
        let repo_op_result = json!({"message": "done"});
        let formatted = formatter.format_successful_tool_result("add_existing_repository", &repo_op_result);
        assert!(formatted.contains("📦 Repository Operation"));
        
        let file_search_result = json!({"pattern": "*.rs"});
        let formatted = formatter.format_successful_tool_result("search_file_in_repository", &file_search_result);
        assert!(formatted.contains("📁 File Search Results"));
        
        let edit_result = json!({"message": "edited"});
        let formatted = formatter.format_successful_tool_result("edit", &edit_result);
        assert!(formatted.contains("✏️ Edit Operation"));
        
        // Test unknown tool falls back to generic
        let unknown_result = json!({"data": "test"});
        let formatted = formatter.format_successful_tool_result("unknown_tool", &unknown_result);
        // Should be pretty-printed JSON in markdown code block
        assert!(formatted.starts_with("```json\n"));
        assert!(formatted.ends_with("\n```"));
        assert!(formatted.contains("data"));
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1048576), "1.0 MB");
        assert_eq!(format_bytes(1073741824), "1.0 GB");
        assert_eq!(format_bytes(1099511627776), "1.0 TB");
        assert_eq!(format_bytes(2560), "2.5 KB");
        assert_eq!(format_bytes(5368709120), "5.0 GB");
    }

    #[test]
    fn test_format_bytes_edge_cases() {
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1025), "1.0 KB");
        
        // For very large numbers, just check it doesn't panic and produces reasonable output
        let very_large_result = format_bytes(u64::MAX);
        assert!(very_large_result.contains("TB"));
        assert!(very_large_result.len() > 5); // Should be some reasonable length
        assert!(very_large_result.len() < 50); // But not absurdly long
    }

    #[test]
    fn test_web_search_with_partial_data() {
        let formatter = ToolResultFormatter::new();
        let value = json!({
            "query": "rust programming",
            // Missing answer and sources
        });
        
        let formatted = formatter.format_web_search_result(&value);
        assert!(formatted.contains("🔍 **Web Search Results**"));
        assert!(formatted.contains("**Query:** rust programming"));
        // Should handle missing fields gracefully
    }

    #[test]
    fn test_file_result_minimal() {
        let formatter = ToolResultFormatter::new();
        let value = json!({
            "content": "Hello, world!"
        });
        
        let formatted = formatter.format_file_result(&value);
        assert!(formatted.contains("FILE: File Content"));
        assert!(formatted.contains("**Content:**"));
        assert!(formatted.contains("Hello, world!"));
    }

    #[test]
    fn test_code_search_no_results() {
        let formatter = ToolResultFormatter::new();
        let value = json!({
            "query": "nonexistent",
            "results": []
        });
        
        let formatted = formatter.format_code_search_result(&value);
        assert!(formatted.contains("SEARCH: Code Search Results"));
        assert!(formatted.contains("**Query:** nonexistent"));
        assert!(formatted.contains("**Found 0 results:**"));
    }

    #[test]
    fn test_repository_list_empty() {
        let formatter = ToolResultFormatter::new();
        let value = json!({
            "repositories": []
        });
        
        let formatted = formatter.format_repository_list_result(&value);
        assert!(formatted.contains("📚 Enhanced Repository List"));
        assert!(formatted.contains("**Found 0 repositories:**"));
    }

    #[test]
    fn test_edit_result_minimal() {
        let formatter = ToolResultFormatter::new();
        let value = json!({
            "message": "Success"
        });
        
        let formatted = formatter.format_edit_result(&value);
        assert!(formatted.contains("✏️ Edit Operation"));
        assert!(formatted.contains("**Result:** Success"));
    }

    #[test]
    fn test_all_tool_type_variants() {
        let formatter = ToolResultFormatter::new();
        let test_data = json!({"test": "data"});
        
        // Test all the specific tool types mentioned in format_successful_tool_result
        let tool_types = vec![
            "web_search", "view_file", "read_file", "code_search", "list_repositories",
            "add_existing_repository", "sync_repository", "remove_repository", "search_file_in_repository",
            "edit", "semantic_edit", "validate"
        ];
        
        for tool_type in tool_types {
            let formatted = formatter.format_successful_tool_result(tool_type, &test_data);
            assert!(!formatted.is_empty(), "Tool type {tool_type} should produce non-empty output");
        }
    }

    #[test]
    fn test_format_mcp_query_result() {
        let formatter = ToolResultFormatter::new();
        let query_result = json!({
            "results": [
                {
                    "filePath": "src/main.rs",
                    "startLine": 10,
                    "endLine": 20,
                    "score": 0.95,
                    "preview": "fn main() {\n    println!(\"Hello, world!\");\n}",
                    "elementType": "function",
                    "language": "rust"
                },
                {
                    "filePath": "src/lib.rs",
                    "startLine": 5,
                    "endLine": 15,
                    "score": 0.85,
                    "preview": "pub fn greet(name: &str) {\n    println!(\"Hello, {}!\", name);\n}",
                    "elementType": "function", 
                    "language": "rust"
                }
            ]
        });
        
        // Test that __query tool uses the proper formatter
        let formatted = formatter.format_successful_tool_result("mcp__sagitta__query", &query_result);
        
        // Should contain formatted search results, not raw JSON
        assert!(formatted.contains("Found 2 results"));
        assert!(formatted.contains("src/main.rs"));
        assert!(formatted.contains("src/lib.rs"));
        assert!(formatted.contains("10-20"));
        assert!(formatted.contains("5-15"));
        assert!(formatted.contains("score: 0.95"));
        assert!(formatted.contains("score: 0.85"));
        assert!(formatted.contains("```rust"));
        assert!(formatted.contains("fn main()"));
        assert!(formatted.contains("pub fn greet"));
        
        // Should NOT contain raw JSON formatting
        assert!(!formatted.contains("```json"));
        assert!(!formatted.contains("\"results\":"));
        assert!(!formatted.contains("\"filePath\":"));
    }
} 