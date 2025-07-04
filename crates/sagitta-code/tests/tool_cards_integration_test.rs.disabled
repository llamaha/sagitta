#[cfg(test)]
mod tool_cards_integration_tests {
    use sagitta_code::gui::app::{AppState, RunningToolInfo};
    use sagitta_code::agent::events::ToolRunId;
    use uuid::Uuid;
    
    #[test]
    fn test_running_tools_lifecycle() {
        let mut app_state = AppState::new();
        let run_id: ToolRunId = Uuid::new_v4();
        
        // Add a running tool
        let tool_info = RunningToolInfo {
            tool_name: "test_tool".to_string(),
            progress: None,
            message_id: "msg123".to_string(),
            start_time: std::time::Instant::now(),
        };
        
        app_state.running_tools.insert(run_id, tool_info.clone());
        
        // Verify tool is tracked
        assert_eq!(app_state.running_tools.len(), 1);
        assert_eq!(app_state.running_tools.get(&run_id).unwrap().tool_name, "test_tool");
        
        // Update progress
        app_state.running_tools.get_mut(&run_id).unwrap().progress = Some(0.5);
        assert_eq!(app_state.running_tools.get(&run_id).unwrap().progress, Some(0.5));
        
        // Remove on completion
        app_state.running_tools.remove(&run_id);
        assert_eq!(app_state.running_tools.len(), 0);
    }
    
    #[test]
    fn test_multiple_running_tools() {
        let mut app_state = AppState::new();
        
        // Add multiple tools
        for i in 0..3 {
            let run_id: ToolRunId = Uuid::new_v4();
            let tool_info = RunningToolInfo {
                tool_name: format!("tool_{}", i),
                progress: Some(i as f32 * 0.25),
                message_id: format!("msg_{}", i),
                start_time: std::time::Instant::now(),
            };
            app_state.running_tools.insert(run_id, tool_info);
        }
        
        assert_eq!(app_state.running_tools.len(), 3);
        
        // Verify each tool has correct progress
        for (_, tool_info) in &app_state.running_tools {
            if tool_info.tool_name == "tool_0" {
                assert_eq!(tool_info.progress, Some(0.0));
            } else if tool_info.tool_name == "tool_1" {
                assert_eq!(tool_info.progress, Some(0.25));
            } else if tool_info.tool_name == "tool_2" {
                assert_eq!(tool_info.progress, Some(0.5));
            }
        }
    }
}