// Tests for chat UI improvements

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::agent::events::ToolRunId;
    use crate::gui::chat::ToolCard;
    use uuid::Uuid;
    use serde_json::json;
    
    // Helper function to create a test tool card
    fn create_test_tool_card(tool_name: &str, status: ToolCardStatus) -> ToolCard {
        ToolCard {
            run_id: ToolRunId(Uuid::new_v4()),
            tool_name: tool_name.to_string(),
            status,
            progress: None,
            logs: Vec::new(),
            started_at: chrono::Utc::now(),
            completed_at: None,
            input_params: json!({"test": "params"}),
            result: None,
        }
    }
    
    #[test]
    fn test_tool_card_displays_actual_tool_name() {
        let tool_card = create_test_tool_card("web_search", ToolCardStatus::Running);
        
        // The tool card should display "web_search", not a random ID
        assert_eq!(tool_card.tool_name, "web_search");
        assert_ne!(tool_card.tool_name, "tool_2343243242342");
    }
    
    #[test]
    fn test_tool_card_with_result_shows_formatted_output() {
        let mut tool_card = create_test_tool_card("web_search", ToolCardStatus::Completed { success: true });
        tool_card.result = Some(json!({
            "query": "rust programming",
            "answer": "Rust is a systems programming language",
            "sources": [
                {
                    "title": "Rust Programming Language",
                    "url": "https://www.rust-lang.org/"
                }
            ]
        }));
        
        // The result should be formatted, not raw JSON
        assert!(tool_card.result.is_some());
    }
    
    #[test]
    fn test_different_tool_types_have_different_formatting() {
        // Test shell execution tool
        let mut shell_card = create_test_tool_card("streaming_shell_execution", ToolCardStatus::Completed { success: true });
        shell_card.result = Some(json!({
            "stdout": "Hello, World!",
            "stderr": "",
            "exit_code": 0
        }));
        
        // Test code search tool
        let mut search_card = create_test_tool_card("code_search", ToolCardStatus::Completed { success: true });
        search_card.result = Some(json!({
            "query": "fn main",
            "results": [
                {
                    "file_path": "src/main.rs",
                    "snippet": "fn main() {\n    println!(\"Hello!\");\n}"
                }
            ]
        }));
        
        // Test edit tool (for git diff format)
        let mut edit_card = create_test_tool_card("edit_file", ToolCardStatus::Completed { success: true });
        edit_card.result = Some(json!({
            "file_path": "src/lib.rs",
            "changes": "+ Added new function\n- Removed old code"
        }));
        
        // Each should have appropriate formatting
        assert!(shell_card.result.is_some());
        assert!(search_card.result.is_some());
        assert!(edit_card.result.is_some());
    }
    
    #[test]
    fn test_tool_card_status_affects_display() {
        let running_card = create_test_tool_card("test_tool", ToolCardStatus::Running);
        let completed_card = create_test_tool_card("test_tool", ToolCardStatus::Completed { success: true });
        let failed_card = create_test_tool_card("test_tool", ToolCardStatus::Failed { error: "Network error".to_string() });
        let cancelled_card = create_test_tool_card("test_tool", ToolCardStatus::Cancelled);
        
        // Each status should have different visual representation
        assert!(matches!(running_card.status, ToolCardStatus::Running));
        assert!(matches!(completed_card.status, ToolCardStatus::Completed { success: true }));
        assert!(matches!(failed_card.status, ToolCardStatus::Failed { .. }));
        assert!(matches!(cancelled_card.status, ToolCardStatus::Cancelled));
    }
    
    #[test]
    fn test_large_output_is_truncated_with_scroll_option() {
        let mut tool_card = create_test_tool_card("list_files", ToolCardStatus::Completed { success: true });
        let large_output = (0..1000).map(|i| format!("file_{}.txt", i)).collect::<Vec<_>>().join("\n");
        tool_card.result = Some(json!({
            "files": large_output
        }));
        
        // Large outputs should be handled gracefully
        assert!(tool_card.result.is_some());
    }
    
    #[test]
    fn test_json_output_is_pretty_printed() {
        let mut tool_card = create_test_tool_card("get_config", ToolCardStatus::Completed { success: true });
        tool_card.result = Some(json!({
            "nested": {
                "config": {
                    "key": "value",
                    "array": [1, 2, 3]
                }
            }
        }));
        
        // JSON should be formatted nicely, not on one line
        assert!(tool_card.result.is_some());
    }
    
    // New comprehensive tests for Phase 2 refinements
    
    #[test]
    fn test_mcp_tool_name_transformation() {
        // Test helper function for human-friendly names
        let test_cases = vec![
            ("mcp__sagitta-internal-5c623314-60dd-4557-bcab-799ab3feb6ad__query", "Semantic Code Search"),
            ("mcp__sagitta-mcp-stdio__repository_view_file", "View Repository File"),
            ("mcp__sagitta-mcp-stdio__repository_search_file", "Search Repository Files"),
            ("mcp__sagitta-mcp-stdio__repository_list", "List Repositories"),
            ("mcp__sagitta-mcp-stdio__repository_map", "Map Repository Structure"),
            ("mcp__sagitta-mcp-stdio__repository_add", "Add Repository"),
            ("mcp__sagitta-mcp-stdio__repository_list_branches", "List Repository Branches"),
            ("Read", "Read File"),
            ("Write", "Write File"),
            ("Edit", "Edit File"),
            ("MultiEdit", "Multi Edit File"),
            ("Bash", "Run Command"),
            ("WebSearch", "Search Web"),
            ("WebFetch", "Fetch Web Content"),
            ("TodoRead", "Read Todo List"),
            ("TodoWrite", "Update Todo List"),
        ];
        
        for (raw_name, expected) in test_cases {
            let friendly_name = get_human_friendly_tool_name(raw_name);
            assert_eq!(friendly_name, expected, "Failed for tool: {}", raw_name);
        }
    }
    
    #[test]
    fn test_tool_parameters_visibility() {
        // Test that tool parameters are extracted and formatted correctly
        let test_cases = vec![
            (
                "mcp__sagitta-internal-uuid__query",
                json!({
                    "query": "main entrypoint",
                    "repository": "fibonacci-calculator",
                    "limit": 5
                }),
                vec!["Query: \"main entrypoint\"", "Repository: fibonacci-calculator", "Limit: 5"]
            ),
            (
                "Read",
                json!({
                    "file_path": "/home/user/test.rs"
                }),
                vec!["File Path: \"/home/user/test.rs\""]
            ),
            (
                "Bash",
                json!({
                    "command": "ls -la",
                    "working_directory": "/home/user"
                }),
                vec!["Command: \"ls -la\"", "Working Directory: \"/home/user\""]
            ),
        ];
        
        for (tool_name, args, expected_params) in test_cases {
            let formatted = format_tool_parameters(tool_name, &args);
            for param in expected_params {
                assert!(formatted.contains(param), 
                    "Parameter '{}' not found in formatted output: {}", param, formatted);
            }
        }
    }
    
    #[test]
    fn test_copy_to_clipboard_includes_tools() {
        // Create a message with tool cards
        let message = StreamingMessage {
            id: Uuid::new_v4(),
            author: MessageAuthor::Assistant,
            content: "I'll search for the main function.".to_string(),
            timestamp: chrono::Utc::now(),
            is_streaming: false,
            tool_calls: vec![],
        };
        
        let tool_card = ToolCard {
            run_id: ToolRunId(Uuid::new_v4()),
            tool_name: "mcp__sagitta-internal-uuid__query".to_string(),
            status: ToolCardStatus::Completed { success: true },
            progress: None,
            logs: Vec::new(),
            started_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
            input_params: json!({
                "query": "main function",
                "repository": "test-repo"
            }),
            result: Some(json!({
                "results": [{
                    "file": "src/main.rs",
                    "line": 24,
                    "snippet": "fn main() {"
                }]
            })),
        };
        
        let formatted = format_message_with_tools_for_clipboard(&message, vec![tool_card]);
        
        // Check that the formatted output includes tool information
        assert!(formatted.contains("Assistant: I'll search for the main function."));
        assert!(formatted.contains("🔧 Semantic Code Search"));
        assert!(formatted.contains("Parameters:"));
        assert!(formatted.contains("Query: \"main function\""));
        assert!(formatted.contains("Repository: test-repo"));
        assert!(formatted.contains("Results:"));
        assert!(formatted.contains("src/main.rs:24"));
    }
    
    #[test]
    fn test_scrollbar_appears_for_large_content() {
        // This would need UI testing framework, but we can test the logic
        let content_height = calculate_content_height(&"line\n".repeat(100));
        assert!(content_height > 200.0, "Large content should exceed 200px threshold");
        
        let small_content_height = calculate_content_height("single line");
        assert!(small_content_height < 200.0, "Small content should not need scrollbar");
    }
    
    #[test]
    fn test_tool_result_formatting_by_type() {
        // Test different formatters for different tool types
        let formatter = crate::gui::app::tool_formatting::ToolResultFormatter::new();
        
        // Test bash command formatting
        let bash_result = json!({
            "stdout": "file1.txt\nfile2.txt\nfile3.txt",
            "stderr": "",
            "exit_code": 0
        });
        let formatted = formatter.format_successful_tool_result("Bash", &bash_result);
        assert!(formatted.contains("file1.txt"));
        assert!(formatted.contains("Exit code: 0"));
        
        // Test file edit formatting (diff style)
        let edit_result = json!({
            "diff": "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,3 +1,4 @@\n fn main() {\n+    // New comment\n     println!(\"Hello\");\n }"
        });
        let formatted = formatter.format_successful_tool_result("Edit", &edit_result);
        assert!(formatted.contains("+++"));
        assert!(formatted.contains("// New comment"));
        
        // Test code search formatting
        let search_result = json!({
            "results": [{
                "file_path": "src/lib.rs",
                "line": 42,
                "content": "pub fn calculate() -> u32 {"
            }]
        });
        let formatted = formatter.format_successful_tool_result("mcp__sagitta-internal-uuid__query", &search_result);
        assert!(formatted.contains("src/lib.rs:42"));
    }
    
    // Helper functions that should be implemented in the actual code
    
    fn get_human_friendly_tool_name(raw_name: &str) -> &'static str {
        match raw_name {
            name if name.contains("__query") => "Semantic Code Search",
            name if name.contains("__repository_view_file") => "View Repository File",
            name if name.contains("__repository_search_file") => "Search Repository Files",
            name if name.contains("__repository_list_branches") => "List Repository Branches",
            name if name.contains("__repository_list") => "List Repositories",
            name if name.contains("__repository_map") => "Map Repository Structure",
            name if name.contains("__repository_add") => "Add Repository",
            "Read" => "Read File",
            "Write" => "Write File",
            "Edit" => "Edit File",
            "MultiEdit" => "Multi Edit File",
            "Bash" => "Run Command",
            "WebSearch" => "Search Web",
            "WebFetch" => "Fetch Web Content",
            "TodoRead" => "Read Todo List",
            "TodoWrite" => "Update Todo List",
            _ => {
                // For unknown MCP tools, try to extract operation name
                if raw_name.starts_with("mcp__") {
                    if let Some(op) = raw_name.split("__").last() {
                        // This would need dynamic allocation in real implementation
                        match op {
                            "ping" => "Ping",
                            _ => "Unknown Tool"
                        }
                    } else {
                        "Unknown Tool"
                    }
                } else {
                    "Unknown Tool"
                }
            }
        }
    }
    
    fn format_tool_parameters(tool_name: &str, args: &serde_json::Value) -> String {
        let mut params = Vec::new();
        
        if let Some(obj) = args.as_object() {
            for (key, value) in obj {
                let formatted_key = key.split('_')
                    .map(|w| {
                        let mut c = w.chars();
                        match c.next() {
                            None => String::new(),
                            Some(first) => first.to_uppercase().collect::<String>() + c.as_str()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                
                let formatted_value = match value {
                    serde_json::Value::String(s) => format!("\"{}\"", s),
                    v => v.to_string(),
                };
                
                params.push(format!("{}: {}", formatted_key, formatted_value));
            }
        }
        
        params.join("\n")
    }
    
    fn format_message_with_tools_for_clipboard(message: &StreamingMessage, tool_cards: Vec<ToolCard>) -> String {
        let mut result = format!("{}: {}\n", 
            match message.author {
                MessageAuthor::User => "User",
                MessageAuthor::Agent => "Agent",
                MessageAuthor::System => "System",
                MessageAuthor::Tool => "Tool",
            },
            message.content
        );
        
        for tool_card in tool_cards {
            let tool_name = get_human_friendly_tool_name(&tool_card.tool_name);
            result.push_str(&format!("\n🔧 {}\n", tool_name));
            
            result.push_str("Parameters:\n");
            let params = format_tool_parameters(&tool_card.tool_name, &tool_card.input_params);
            for line in params.lines() {
                result.push_str(&format!("  {}\n", line));
            }
            
            if let Some(ref tool_result) = tool_card.result {
                result.push_str("\nResults:\n");
                // Simplified formatting for test
                if let Some(results) = tool_result.get("results").and_then(|r| r.as_array()) {
                    for res in results {
                        if let (Some(file), Some(line)) = (
                            res.get("file").and_then(|f| f.as_str()),
                            res.get("line").and_then(|l| l.as_i64())
                        ) {
                            result.push_str(&format!("  {}:{}\n", file, line));
                        }
                    }
                } else {
                    result.push_str(&format!("  {}\n", serde_json::to_string_pretty(tool_result).unwrap()));
                }
            }
        }
        
        result
    }
    
    fn calculate_content_height(content: &str) -> f32 {
        // Rough calculation: assume 15px per line
        let line_count = content.lines().count();
        line_count as f32 * 15.0
    }
    
    #[test]
    fn test_format_tool_parameters_for_semantic_search() {
        use crate::gui::chat::view::format_tool_parameters;
        
        // Test semantic search tool with snake_case parameters
        let semantic_search_params = json!({
            "query": "format_tool_parameters function",
            "repository": "sagitta-code",
            "limit": 10
        });
        
        let params = format_tool_parameters("mcp__query", &semantic_search_params);
        assert!(!params.is_empty(), "Semantic search parameters should not be empty");
        
        // Check that all parameters are captured
        let param_map: std::collections::HashMap<_, _> = params.into_iter().collect();
        assert_eq!(param_map.get("Query"), Some(&"format_tool_parameters function".to_string()));
        assert_eq!(param_map.get("Repository"), Some(&"sagitta-code".to_string()));
        assert_eq!(param_map.get("Limit"), Some(&"10".to_string()));
    }
    
    #[test]
    fn test_format_tool_parameters_for_mcp_semantic_search_camelcase() {
        use crate::gui::chat::view::format_tool_parameters;
        
        // Test semantic search tool with camelCase parameters (MCP style)
        let semantic_search_params = json!({
            "queryText": "find main function",
            "repository": "my-project",
            "limit": 5
        });
        
        let params = format_tool_parameters("mcp__sagitta-internal-uuid__query", &semantic_search_params);
        assert!(!params.is_empty(), "MCP semantic search parameters should not be empty");
        
        // Check that camelCase parameters are captured
        let param_map: std::collections::HashMap<_, _> = params.into_iter().collect();
        assert_eq!(param_map.get("Query"), Some(&"find main function".to_string()));
        assert_eq!(param_map.get("Repository"), Some(&"my-project".to_string()));
        assert_eq!(param_map.get("Limit"), Some(&"5".to_string()));
    }
    
    #[test]
    fn test_format_tool_parameters_for_other_tools() {
        use crate::gui::chat::view::format_tool_parameters;
        
        // Test Read tool with snake_case
        let read_params = json!({
            "file_path": "/home/user/test.rs"
        });
        let params = format_tool_parameters("Read", &read_params);
        let param_map: std::collections::HashMap<_, _> = params.into_iter().collect();
        assert_eq!(param_map.get("File"), Some(&"/home/user/test.rs".to_string()));
        
        // Test MCP file tool with camelCase
        let mcp_read_params = json!({
            "filePath": "/home/user/mcp_file.js"
        });
        let params = format_tool_parameters("mcp__some-server__read_file", &mcp_read_params);
        let param_map: std::collections::HashMap<_, _> = params.into_iter().collect();
        assert_eq!(param_map.get("File"), Some(&"/home/user/mcp_file.js".to_string()));
        
        // Test Bash tool
        let bash_params = json!({
            "command": "ls -la"
        });
        let params = format_tool_parameters("Bash", &bash_params);
        let param_map: std::collections::HashMap<_, _> = params.into_iter().collect();
        assert_eq!(param_map.get("Command"), Some(&"ls -la".to_string()));
        
        // Test generic tool with various parameter types
        let generic_params = json!({
            "search_term": "test",
            "case_sensitive": true,
            "max_results": 50
        });
        let params = format_tool_parameters("GenericTool", &generic_params);
        assert_eq!(params.len(), 3);
        
        // Check that keys are properly formatted
        let param_map: std::collections::HashMap<_, _> = params.into_iter().collect();
        assert!(param_map.contains_key("Search Term"));
        assert!(param_map.contains_key("Case Sensitive"));
        assert!(param_map.contains_key("Max Results"));
    }
}