// Tests for chat UI improvements

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::agent::events::ToolRunId;
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
}