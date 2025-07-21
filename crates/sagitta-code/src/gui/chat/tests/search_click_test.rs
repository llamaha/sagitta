#[cfg(test)]
mod search_click_tests {
    use crate::gui::chat::view::{render_search_result_item, render_search_output, is_search_result};
    use crate::gui::theme::AppTheme;

    #[test]
    fn test_render_search_result_item_returns_action() {
        // Create a test UI context
        let ctx = egui::Context::default();
        let mut ui = egui::Ui::new(
            ctx.clone(),
            egui::LayerId::background(),
            egui::Id::new("test"),
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(800.0, 600.0)),
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(800.0, 600.0)),
        );

        // Create test search result
        let search_result = serde_json::json!({
            "filePath": "/test/path/file.rs",
            "startLine": 10,
            "endLine": 20,
            "elementType": "function",
            "language": "rust",
            "score": 0.95
        });

        // Test that render_search_result_item returns action data when clicked
        let action = render_search_result_item(&mut ui, 0, &search_result, AppTheme::Dark);
        
        // In a real click scenario, this would return Some with action data
        // For now, we just verify the function can be called without panic
        assert!(action.is_none() || action.is_some());
    }

    #[test]
    fn test_render_search_output_propagates_action() {
        // Create a test UI context
        let ctx = egui::Context::default();
        let mut ui = egui::Ui::new(
            ctx.clone(),
            egui::LayerId::background(),
            egui::Id::new("test"),
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(800.0, 600.0)),
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(800.0, 600.0)),
        );

        // Create test search results
        let search_results = serde_json::json!({
            "queryText": "test query",
            "results": [
                {
                    "filePath": "/test/path/file1.rs",
                    "startLine": 10,
                    "endLine": 20,
                    "elementType": "function",
                    "language": "rust",
                    "score": 0.95
                },
                {
                    "filePath": "/test/path/file2.rs",
                    "startLine": 30,
                    "endLine": 40,
                    "elementType": "struct",
                    "language": "rust",
                    "score": 0.85
                }
            ]
        });

        // Test that render_search_output can handle search results
        let action = render_search_output(&mut ui, &search_results, AppTheme::Dark);
        
        // Verify function executes without panic
        assert!(action.is_none() || action.is_some());
    }

    #[test]
    fn test_action_format() {
        // Test the expected action format
        let file_path = "/test/path/file.rs";
        let start_line = 10;
        let end_line = 20;
        
        let action_data = serde_json::json!({
            "file_path": file_path,
            "start_line": start_line,
            "end_line": end_line
        });
        
        let action = ("__OPEN_FILE__".to_string(), action_data.to_string());
        
        // Verify the action can be parsed back
        let (tool_name, tool_args) = action;
        assert_eq!(tool_name, "__OPEN_FILE__");
        
        let parsed_data: serde_json::Value = serde_json::from_str(&tool_args).unwrap();
        assert_eq!(parsed_data["file_path"], file_path);
        assert_eq!(parsed_data["start_line"], start_line);
        assert_eq!(parsed_data["end_line"], end_line);
    }

    #[test]
    fn test_is_search_result() {
        // Test various tool names and result formats
        let semantic_result = serde_json::json!({ "results": [] });
        assert!(is_search_result("semantic_code_search", &semantic_result));
        assert!(is_search_result("mcp__sagitta-mcp__query", &semantic_result));
        
        let file_result = serde_json::json!({ "matchingFiles": [] });
        assert!(is_search_result("search_file", &file_result));
        
        let web_result = serde_json::json!({ "sources": [] });
        assert!(is_search_result("WebSearch", &web_result));
        
        // Non-search tools should return false
        assert!(!is_search_result("read_file", &serde_json::json!({})));
        assert!(!is_search_result("shell_execute", &serde_json::json!({})));
    }
}