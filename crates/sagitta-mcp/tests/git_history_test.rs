#[cfg(test)]
mod git_history_tests {
    use sagitta_mcp::handlers::tool::get_tool_definitions;
    
    #[test]
    fn test_git_history_tool_is_exposed() {
        let tools = get_tool_definitions();
        
        // Find the git history tool
        let git_history_tool = tools.iter()
            .find(|tool| tool.name == "repository_git_history")
            .expect("repository_git_history tool should be defined");
        
        // Verify the tool has proper description
        assert!(git_history_tool.description.is_some());
        let desc = git_history_tool.description.as_ref().unwrap();
        assert!(desc.contains("git") && desc.contains("history"));
        
        // Verify the tool has proper input schema
        let schema = &git_history_tool.input_schema;
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["repositoryName"].is_object());
        assert!(schema["properties"]["maxCommits"].is_object());
        assert!(schema["properties"]["branchName"].is_object());
        assert!(schema["properties"]["since"].is_object());
        assert!(schema["properties"]["until"].is_object());
        assert!(schema["properties"]["author"].is_object());
        assert!(schema["properties"]["path"].is_object());
        
        // Verify required fields
        assert!(schema["required"].as_array().unwrap().contains(&serde_json::Value::String("repositoryName".to_string())));
        
        // Verify annotations
        assert!(git_history_tool.annotations.is_some());
        let annotations = git_history_tool.annotations.as_ref().unwrap();
        assert_eq!(annotations.read_only_hint, Some(true));
        assert_eq!(annotations.destructive_hint, Some(false));
        assert_eq!(annotations.idempotent_hint, Some(true));
        assert_eq!(annotations.open_world_hint, Some(false));
    }
    
    #[test]
    fn test_all_tools_include_git_history() {
        let tools = get_tool_definitions();
        let tool_names: Vec<&String> = tools.iter().map(|t| &t.name).collect();
        
        // Verify git history tool is present
        assert!(
            tool_names.contains(&&"repository_git_history".to_string()),
            "Tool 'repository_git_history' should be exposed in tools list"
        );
    }
}