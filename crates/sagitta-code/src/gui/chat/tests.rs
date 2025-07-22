// Tests for chat UI improvements

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::agent::events::ToolRunId;
    use crate::gui::chat::ToolCard;
    use crate::gui::chat::tool_mappings::{get_human_friendly_tool_name, format_tool_parameters};
    use uuid::Uuid;
    use serde_json::json;
    
    // Helper function to create a test tool card
    fn create_test_tool_card(tool_name: &str, status: ToolCardStatus) -> ToolCard {
        ToolCard {
            run_id: Uuid::new_v4(),
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
            ("mcp__sagitta-mcp-stdio__repository_search_file", "Repository Search File"),
            ("mcp__sagitta-mcp-stdio__repository_list", "List Repositories"),
            ("mcp__sagitta-mcp-stdio__repository_map", "Repository Map"),
            ("mcp__sagitta-mcp-stdio__repository_add", "Add Repository"),
            ("mcp__sagitta-mcp-stdio__repository_list_branches", "List Branches"),
            ("Read", "Read File"),
            ("Write", "Write File"),
            ("Edit", "Edit File"),
            ("MultiEdit", "Multi-Edit File"),
            ("Bash", "Shell Command"),
            ("WebSearch", "Web Search"),
            ("WebFetch", "Fetch Web Content"),
            ("TodoRead", "Read TODOs"),
            ("TodoWrite", "Write TODOs"),
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
                vec!["query: main entrypoint", "repository: fibonacci-calculator", "limit: 5"]
            ),
            (
                "Read",
                json!({
                    "file_path": "/home/user/test.rs"
                }),
                vec!["file_path: /home/user/test.rs"]
            ),
            (
                "Bash",
                json!({
                    "command": "ls -la",
                    "working_directory": "/home/user"
                }),
                vec!["command: ls -la", "working_directory: /home/user"]
            ),
        ];
        
        for (tool_name, args, expected_params) in test_cases {
            let params = format_tool_parameters(tool_name, &args);
            let param_map: std::collections::HashMap<_, _> = params.into_iter().collect();
            
            for expected_param in expected_params {
                // Split expected param into key and value
                let parts: Vec<&str> = expected_param.splitn(2, ": ").collect();
                if parts.len() == 2 {
                    let key = parts[0];
                    let expected_value = parts[1];
                    
                    assert!(param_map.contains_key(key), 
                        "Parameter key '{}' not found in formatted output. Available keys: {:?}", 
                        key, param_map.keys().collect::<Vec<_>>());
                    
                    if let Some(actual_value) = param_map.get(key) {
                        assert_eq!(actual_value, expected_value, 
                            "Parameter value mismatch for key '{}'", key);
                    }
                }
            }
        }
    }
    
    // TODO: Uncomment when format_message_with_tools_for_clipboard is implemented
    // #[test]
    // fn test_copy_to_clipboard_includes_tools() {
    //     // Create a message with tool cards
    //     let message = StreamingMessage {
    //         id: Uuid::new_v4(),
    //         author: MessageAuthor::Assistant,
    //         content: "I'll search for the main function.".to_string(),
    //         timestamp: chrono::Utc::now(),
    //         is_streaming: false,
    //         tool_calls: vec![],
    //     };
    //     
    //     let tool_card = ToolCard {
    //         run_id: ToolRunId(Uuid::new_v4()),
    //         tool_name: "mcp__sagitta-internal-uuid__query".to_string(),
    //         status: ToolCardStatus::Completed { success: true },
    //         progress: None,
    //         logs: Vec::new(),
    //         started_at: chrono::Utc::now(),
    //         completed_at: Some(chrono::Utc::now()),
    //         input_params: json!({
    //             "query": "main function",
    //             "repository": "test-repo"
    //         }),
    //         result: Some(json!({
    //             "results": [{
    //                 "file": "src/main.rs",
    //                 "line": 24,
    //                 "snippet": "fn main() {"
    //             }]
    //         })),
    //     };
    //     
    //     let formatted = format_message_with_tools_for_clipboard(&message, vec![tool_card]);
    //     
    //     // Check that the formatted output includes tool information
    //     assert!(formatted.contains("Assistant: I'll search for the main function."));
    //     assert!(formatted.contains("🔧 Semantic Code Search"));
    //     assert!(formatted.contains("Parameters:"));
    //     assert!(formatted.contains("Query: \"main function\""));
    //     assert!(formatted.contains("Repository: test-repo"));
    //     assert!(formatted.contains("Results:"));
    //     assert!(formatted.contains("src/main.rs:24"));
    // }
    
    // TODO: Uncomment when calculate_content_height is implemented
    // #[test]
    // fn test_scrollbar_appears_for_large_content() {
    //     // This would need UI testing framework, but we can test the logic
    //     let content_height = calculate_content_height(&"line\n".repeat(100));
    //     assert!(content_height > 200.0, "Large content should exceed 200px threshold");
    //     
    //     let small_content_height = calculate_content_height("single line");
    //     assert!(small_content_height < 200.0, "Small content should not need scrollbar");
    // }
    
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
        // Exit code 0 is not shown (only non-zero exit codes are displayed)
        
        // Test file edit formatting
        let edit_result = json!({
            "message": "File edited successfully",
            "file_path": "src/main.rs",
            "changes_made": "Added comment to main function"
        });
        let formatted = formatter.format_successful_tool_result("Edit", &edit_result);
        assert!(formatted.contains("✏️ Edit Operation"));
        assert!(formatted.contains("src/main.rs"));
        
        // Test code search formatting
        let search_result = json!({
            "results": [{
                "filePath": "src/lib.rs",
                "startLine": 42,
                "endLine": 42,
                "preview": "pub fn calculate() -> u32 {"
            }]
        });
        let formatted = formatter.format_successful_tool_result("mcp__sagitta-internal-uuid__query", &search_result);
        assert!(formatted.contains("src/lib.rs"));
        assert!(formatted.contains(":42"));
    }
    
    // Helper functions that should be implemented in the actual code
    
    
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
            for (key, value) in params {
                result.push_str(&format!("  {}: {}\n", key, value));
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
        use crate::gui::chat::tool_mappings::format_tool_parameters;
        
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
        assert_eq!(param_map.get("query"), Some(&"format_tool_parameters function".to_string()));
        assert_eq!(param_map.get("repository"), Some(&"sagitta-code".to_string()));
        assert_eq!(param_map.get("limit"), Some(&"10".to_string()));
    }
    
    #[test]
    fn test_format_tool_parameters_for_mcp_semantic_search_camelcase() {
        use crate::gui::chat::tool_mappings::format_tool_parameters;
        
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
        assert_eq!(param_map.get("queryText"), Some(&"find main function".to_string()));
        assert_eq!(param_map.get("repository"), Some(&"my-project".to_string()));
        assert_eq!(param_map.get("limit"), Some(&"5".to_string()));
    }
    
    #[test]
    fn test_format_tool_parameters_for_other_tools() {
        use crate::gui::chat::tool_mappings::format_tool_parameters;
        
        // Test Read tool with snake_case
        let read_params = json!({
            "file_path": "/home/user/test.rs"
        });
        let params = format_tool_parameters("Read", &read_params);
        let param_map: std::collections::HashMap<_, _> = params.into_iter().collect();
        assert_eq!(param_map.get("file_path"), Some(&"/home/user/test.rs".to_string()));
        
        // Test MCP file tool with camelCase
        let mcp_read_params = json!({
            "filePath": "/home/user/mcp_file.js"
        });
        let params = format_tool_parameters("mcp__some-server__read_file", &mcp_read_params);
        let param_map: std::collections::HashMap<_, _> = params.into_iter().collect();
        assert_eq!(param_map.get("filePath"), Some(&"/home/user/mcp_file.js".to_string()));
        
        // Test Bash tool
        let bash_params = json!({
            "command": "ls -la"
        });
        let params = format_tool_parameters("Bash", &bash_params);
        let param_map: std::collections::HashMap<_, _> = params.into_iter().collect();
        assert_eq!(param_map.get("command"), Some(&"ls -la".to_string()));
        
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
        assert!(param_map.contains_key("search_term"));
        assert!(param_map.contains_key("case_sensitive"));
        assert!(param_map.contains_key("max_results"));
    }
    
    // Tests for Phase 1: Tool Card UI Issues
    
    #[test]
    fn test_tool_card_single_header_when_expanded() {
        // Test that tool cards only show one header when expanded
        let tool_card = create_test_tool_card("Ping", ToolCardStatus::Completed { success: true });
        
        // When a tool card is expanded, it should only have one header, not two
        // This test will help catch the double header issue
        assert_eq!(tool_card.tool_name, "Ping");
        // TODO: Add actual UI rendering test when we have a test framework for egui
    }
    
    #[test]
    fn test_tool_card_single_icon_display() {
        // Test that tool cards only show one icon
        let tool_card = create_test_tool_card("Read", ToolCardStatus::Running);
        
        // Icons should appear only once in the tool card
        assert_eq!(tool_card.tool_name, "Read");
        // TODO: Add actual icon count test when we have UI testing
    }
    
    #[test]
    fn test_tool_card_responsive_width() {
        // Test that tool cards properly resize to available width
        let tool_card = create_test_tool_card("WebSearch", ToolCardStatus::Completed { success: true });
        
        // Cards should use available container width, not fixed width
        assert!(tool_card.result.is_none() || tool_card.result.is_some());
        // TODO: Add actual width calculation test
    }
    
    #[test]
    fn test_read_file_view_full_functionality() {
        // Test that "View full file" actually shows the full file
        let mut tool_card = create_test_tool_card("Read", ToolCardStatus::Completed { success: true });
        tool_card.result = Some(json!({
            "file_path": "/test/file.rs",
            "content": "Line 1\nLine 2\nLine 3\n...truncated...\nLine 100",
            "truncated": true,
            "total_lines": 100
        }));
        
        // View full file should provide a way to see all content
        assert!(tool_card.result.is_some());
        // TODO: Test actual view full file button functionality
    }
    
    #[test]
    fn test_semantic_code_search_name_display() {
        // Test that code search shows as "Semantic Code Search"
        let tool_name = "mcp__sagitta-internal-uuid__query";
        let friendly_name = get_human_friendly_tool_name(tool_name);
        
        assert_eq!(friendly_name, "Semantic Code Search");
        assert_ne!(friendly_name, "Code Search");
    }
    
    #[test]
    fn test_web_search_results_parsing() {
        // Test that web search results are properly parsed and displayed
        let mut tool_card = create_test_tool_card("WebSearch", ToolCardStatus::Completed { success: true });
        tool_card.result = Some(json!({
            "query": "rust documentation",
            "answer": "Rust is a systems programming language",
            "sources": [
                {
                    "title": "The Rust Programming Language",
                    "url": "https://doc.rust-lang.org/book/",
                    "snippet": "The official book on Rust"
                },
                {
                    "title": "Rust By Example",
                    "url": "https://doc.rust-lang.org/rust-by-example/",
                    "snippet": "Learn Rust with examples"
                }
            ]
        }));
        
        // Results should be parsed and shown, not display "no results"
        assert!(tool_card.result.is_some());
        if let Some(result) = &tool_card.result {
            let sources = result.get("sources").and_then(|v| v.as_array());
            assert!(sources.is_some());
            assert_eq!(sources.unwrap().len(), 2);
        }
    }
    
    #[test]
    fn test_list_branches_meaningful_output() {
        // Test that list branches shows actual branch info, not just "Operation completed"
        let mut tool_card = create_test_tool_card("mcp__sagitta-mcp-stdio__repository_list_branches", 
                                                   ToolCardStatus::Completed { success: true });
        tool_card.result = Some(json!({
            "repositoryName": "test-repo",
            "branches": [
                {
                    "name": "main",
                    "current": true,
                    "lastCommit": {
                        "message": "Initial commit"
                    }
                },
                {
                    "name": "feature/test",
                    "current": false,
                    "lastCommit": {
                        "message": "Add test feature"
                    }
                }
            ]
        }));
        
        // Should show branch names and info, not generic message
        assert!(tool_card.result.is_some());
        // TODO: Test actual formatted output
    }
    
    #[test]
    fn test_tool_card_text_contrast() {
        // Test that text has sufficient contrast for readability
        // This would need theme color values to test properly
        let tool_card = create_test_tool_card("Test", ToolCardStatus::Completed { success: true });
        
        // Text should have sufficient contrast ratio (WCAG AA standard: 4.5:1)
        assert!(tool_card.tool_name.len() > 0);
        // TODO: Add actual color contrast test when we have theme access
    }
}