use serde_json;

/// Get human-friendly name for a tool
pub fn get_human_friendly_tool_name(tool_name: &str) -> String {
    // Clean up mcp__ prefix if present
    let clean_name = if tool_name.starts_with("mcp__") {
        // Extract just the tool name part after the second underscore
        if let Some(parts) = tool_name.strip_prefix("mcp__") {
            if let Some((_provider, actual_tool)) = parts.split_once("__") {
                actual_tool
            } else {
                parts
            }
        } else {
            tool_name
        }
    } else {
        tool_name
    };
    
    match clean_name {
        // File operations
        "read_file" | "Read" => "Read File".to_string(),
        "write_file" | "Write" => "Write File".to_string(),
        "edit_file" | "Edit" => "Edit File".to_string(),
        "multi_edit_file" | "MultiEdit" => "Multi-Edit File".to_string(),
        
        // Search operations
        "search_file" | "Glob" => "Search Files".to_string(),
        "semantic_code_search" | "Search" | "query" => "Semantic Code Search".to_string(),
        "repository_search" => "Search Repository".to_string(),
        "grep" | "Grep" => "Grep".to_string(),
        
        // Repository operations
        "repository_add" => "Add Repository".to_string(),
        "repository_list" => "List Repositories".to_string(),
        "repository_sync" => "Sync Repository".to_string(),
        "repository_switch_branch" => "Switch Branch".to_string(),
        "repository_list_branches" => "List Branches".to_string(),
        "repository_view_file" => "View Repository File".to_string(),
        
        // Shell and system
        "shell_execute" | "Bash" => "Shell Command".to_string(),
        "streaming_shell_execution" => "Streaming Shell".to_string(),
        
        // Task management
        "todo_read" | "TodoRead" => "Read TODOs".to_string(),
        "todo_write" | "TodoWrite" => "Write TODOs".to_string(),
        "Task" => "Run Task".to_string(),
        
        // Web operations
        "web_search" | "WebSearch" => "Web Search".to_string(),
        "web_fetch" | "WebFetch" => "Fetch Web Content".to_string(),
        
        // Other tools
        "ping" => "Ping".to_string(),
        "exit_plan_mode" => "Exit Plan Mode".to_string(),
        "NotebookRead" => "Read Notebook".to_string(),
        "NotebookEdit" => "Edit Notebook".to_string(),
        "LS" => "List Directory".to_string(),
        
        // OpenAI format
        "run_python" => "Run Python".to_string(),
        "analyze_data" => "Analyze Data".to_string(),
        
        _ => {
            // Convert snake_case or kebab-case to Title Case
            clean_name
                .replace('_', " ")
                .replace('-', " ")
                .split_whitespace()
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        }
    }
}

/// Get appropriate icon for a tool
pub fn get_tool_icon(tool_name: &str) -> &'static str {
    // Clean up mcp__ prefix if present
    let clean_name = if tool_name.starts_with("mcp__") {
        if let Some(parts) = tool_name.strip_prefix("mcp__") {
            if let Some((_provider, actual_tool)) = parts.split_once("__") {
                actual_tool
            } else {
                parts
            }
        } else {
            tool_name
        }
    } else {
        tool_name
    };
    
    match clean_name {
        // File operations
        "read_file" | "Read" => "[R]",
        "write_file" | "Write" => "[W]",
        "edit_file" | "Edit" => "[E]",
        "multi_edit_file" | "MultiEdit" => "[M]",
        
        // Search operations
        "search_file" | "Glob" => "[?]",
        "semantic_code_search" | "Search" | "query" => "[S]",
        "repository_search" => "[?]",
        "grep" | "Grep" => "[G]",
        
        // Repository operations
        "repository_add" => "+",
        "repository_list" => "#",
        "repository_sync" => "@",
        "repository_switch_branch" => "~",
        "repository_list_branches" => "#",
        "repository_view_file" => "[F]",
        
        // Shell and system
        "shell_execute" | "Bash" => "$",
        "streaming_shell_execution" => "$",
        
        // Task management
        "todo_read" | "TodoRead" => "[]",
        "todo_write" | "TodoWrite" => "[+]",
        "Task" => "[T]",
        
        // Web operations
        "web_search" | "WebSearch" => "[W]",
        "web_fetch" | "WebFetch" => "[W]",
        
        // Other tools
        "ping" => ".",
        "exit_plan_mode" => "[X]",
        "NotebookRead" => "[N]",
        "NotebookEdit" => "[N]",
        "LS" => "[D]",
        
        // OpenAI format
        "run_python" => "[P]",
        "analyze_data" => "[A]",
        
        _ => "[*]", // Default tool icon
    }
}

/// Extract tool parameters that should be shown inline
pub fn format_tool_parameters_for_inline(tool_name: &str, args: &serde_json::Value) -> Vec<(String, String)> {
    let mut params = Vec::new();
    
    // Clean up tool name for consistent matching
    let clean_name = if tool_name.starts_with("mcp__") {
        if let Some(parts) = tool_name.strip_prefix("mcp__") {
            if let Some((_provider, actual_tool)) = parts.split_once("__") {
                actual_tool
            } else {
                parts
            }
        } else {
            tool_name
        }
    } else {
        tool_name
    };
    
    match clean_name {
        // File operations - show path
        "read_file" | "Read" | "write_file" | "Write" | "repository_view_file" => {
            if let Some(path) = args.get("file_path").or_else(|| args.get("path")).and_then(|v| v.as_str()) {
                // Truncate long paths but keep the filename
                let truncated = if path.len() > 40 {
                    if let Some(filename) = path.split('/').last() {
                        format!(".../{}", filename)
                    } else {
                        format!("...{}", &path[path.len()-30..])
                    }
                } else {
                    path.to_string()
                };
                params.push(("path".to_string(), truncated));
            }
        },
        
        // Edit operations - show file and summary
        "edit_file" | "Edit" | "multi_edit_file" | "MultiEdit" => {
            if let Some(path) = args.get("file_path").or_else(|| args.get("path")).and_then(|v| v.as_str()) {
                let truncated = if path.len() > 30 {
                    if let Some(filename) = path.split('/').last() {
                        format!(".../{}", filename)
                    } else {
                        format!("...{}", &path[path.len()-25..])
                    }
                } else {
                    path.to_string()
                };
                params.push(("file".to_string(), truncated));
            }
            
            // For multi-edit, show edit count
            if clean_name == "multi_edit_file" || clean_name == "MultiEdit" {
                if let Some(edits) = args.get("edits").and_then(|v| v.as_array()) {
                    params.push(("edits".to_string(), format!("{} changes", edits.len())));
                }
            }
        },
        
        // Search operations - show query/pattern
        "semantic_code_search" | "Search" | "repository_search" | "query" => {
            if let Some(query) = args.get("queryText").or_else(|| args.get("query")).and_then(|v| v.as_str()) {
                let truncated = if query.len() > 40 {
                    format!("{}...", &query[..37])
                } else {
                    query.to_string()
                };
                params.push(("query".to_string(), truncated));
            }
            
            // Add repository name if present
            if let Some(repo) = args.get("repositoryName").or_else(|| args.get("repository")).and_then(|v| v.as_str()) {
                params.push(("repo".to_string(), repo.to_string()));
            }
            
            // Add element type if present
            if let Some(elem_type) = args.get("elementType").and_then(|v| v.as_str()) {
                params.push(("type".to_string(), elem_type.to_string()));
            }
            
            // Add language if present
            if let Some(lang) = args.get("lang").or_else(|| args.get("language")).and_then(|v| v.as_str()) {
                params.push(("lang".to_string(), lang.to_string()));
            }
            
            // Add limit if present and not default
            if let Some(limit) = args.get("limit").and_then(|v| v.as_u64()) {
                if limit != 10 { // Only show if not default value
                    params.push(("limit".to_string(), limit.to_string()));
                }
            }
        },
        
        "search_file" | "Glob" | "grep" | "Grep" => {
            if let Some(pattern) = args.get("pattern").and_then(|v| v.as_str()) {
                let truncated = if pattern.len() > 30 {
                    format!("{}...", &pattern[..27])
                } else {
                    pattern.to_string()
                };
                params.push(("pattern".to_string(), truncated));
            }
        },
        
        // Shell commands - show command preview
        "shell_execute" | "Bash" | "streaming_shell_execution" => {
            if let Some(cmd) = args.get("command").and_then(|v| v.as_str()) {
                let truncated = if cmd.len() > 50 {
                    format!("{}...", &cmd[..47])
                } else {
                    cmd.to_string()
                };
                params.push(("cmd".to_string(), truncated));
            }
        },
        
        // Repository operations
        "repository_add" => {
            if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
                params.push(("name".to_string(), name.to_string()));
            }
        },
        
        "repository_switch_branch" => {
            if let Some(branch) = args.get("branchName").and_then(|v| v.as_str()) {
                params.push(("branch".to_string(), branch.to_string()));
            }
        },
        
        // Web operations
        "web_search" | "WebSearch" => {
            if let Some(query) = args.get("query").and_then(|v| v.as_str()) {
                let truncated = if query.len() > 40 {
                    format!("{}...", &query[..37])
                } else {
                    query.to_string()
                };
                params.push(("query".to_string(), truncated));
            }
        },
        
        "web_fetch" | "WebFetch" => {
            if let Some(url) = args.get("url").and_then(|v| v.as_str()) {
                // Show just the domain for URLs
                let display_url = if let Some(start) = url.find("://") {
                    if let Some(domain_end) = url[start+3..].find('/') {
                        &url[start+3..start+3+domain_end]
                    } else {
                        &url[start+3..]
                    }
                } else {
                    url
                };
                params.push(("url".to_string(), display_url.to_string()));
            }
        },
        
        _ => {
            // For unknown tools, don't show any inline parameters
        }
    }
    
    params
}

/// Format tool parameters for display
pub fn format_tool_parameters(tool_name: &str, args: &serde_json::Value) -> Vec<(String, String)> {
    let mut params = Vec::new();
    
    // For all tools, extract all parameters
    if let Some(obj) = args.as_object() {
        for (key, value) in obj {
            let formatted_value = match value {
                serde_json::Value::String(s) => {
                    // For file content and similar large strings, truncate
                    if s.len() > 200 {
                        format!("{}... ({} chars)", &s[..200], s.len())
                    } else {
                        s.clone()
                    }
                },
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                serde_json::Value::Array(arr) => format!("[{} items]", arr.len()),
                serde_json::Value::Object(obj) => format!("{{...}} ({} fields)", obj.len()),
                serde_json::Value::Null => "null".to_string(),
            };
            params.push((key.clone(), formatted_value));
        }
    }
    
    params
}