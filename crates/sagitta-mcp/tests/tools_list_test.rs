use sagitta_mcp::handlers::tool::get_tool_definitions;
use sagitta_mcp::mcp::types::InitializeParams;
use sagitta_mcp::server::Server;
use sagitta_search::config::AppConfig;
use std::collections::HashSet;

#[test]
fn test_all_required_tools_are_exposed() {
    let tools = get_tool_definitions();
    let tool_names: HashSet<String> = tools.iter().map(|t| t.name.clone()).collect();
    
    // These are the tools that sagitta-code expects to be available
    let required_tools = vec![
        "ping",
        "repository_add",
        "repository_list",
        "repository_sync",
        "repository_switch_branch",
        "repository_list_branches",
        "semantic_code_search",
        "search_file",
        "todo_read",
        "todo_write",
        "read_file",  // This was missing!
        "write_file",
        "edit_file",
        "multi_edit_file",
        "shell_execute",
    ];
    
    for tool in required_tools {
        assert!(
            tool_names.contains(tool),
            "Required tool '{}' is not exposed by MCP server",
            tool
        );
    }
}

#[test]
fn test_read_file_tool_has_correct_schema() {
    let tools = get_tool_definitions();
    
    let read_file_tool = tools.iter()
        .find(|t| t.name == "read_file")
        .expect("read_file tool should be defined");
    
    // Verify description
    assert!(read_file_tool.description.is_some());
    let desc = read_file_tool.description.as_ref().unwrap();
    assert!(desc.contains("read") || desc.contains("Read"));
    
    // Verify input schema
    let schema = &read_file_tool.input_schema;
    assert_eq!(schema["type"], "object");
    assert!(schema["properties"]["file_path"].is_object());
    
    // Verify it's marked as read-only
    assert!(read_file_tool.annotations.is_some());
    let annotations = read_file_tool.annotations.as_ref().unwrap();
    assert_eq!(annotations.read_only_hint, Some(true));
}

#[tokio::test]
async fn test_mcp_server_initializes_with_all_tools() {
    // Create a test config
    let config = AppConfig::default();
    
    // Create the server
    let server = Server::new(config).await.expect("Failed to create server");
    
    // Get the initialize handler result
    let params = InitializeParams {
        protocol_version: "1.0".to_string(),
        capabilities: Default::default(),
        client_info: Some(sagitta_mcp::mcp::types::ClientInfo {
            name: Some("test".to_string()),
            version: Some("1.0".to_string()),
        }),
    };
    
    // Call the initialize handler
    let result = sagitta_mcp::handlers::initialize::handle_initialize(
        params
    ).await.expect("Initialize should succeed");
    
    // Check that tools are listed in capabilities
    assert!(!result.capabilities.tools.is_empty(), "Tools capability should be present");
    
    // The actual tool list is in the tools map
    let tools_count = result.capabilities.tools.len();
    assert!(tools_count > 0, "Should have at least one tool");
}

// Test to ensure no tools were accidentally removed
#[test]
fn test_no_tools_removed_regression() {
    let tools = get_tool_definitions();
    
    // Minimum expected tool count (should not decrease)
    let min_tool_count = 15; // Based on current tool list
    
    assert!(
        tools.len() >= min_tool_count,
        "Tool count ({}) is less than expected minimum ({}). Tools may have been accidentally removed.",
        tools.len(),
        min_tool_count
    );
}

// Test that tool names match what Claude Code expects
#[test]
fn test_tool_names_match_claude_code_expectations() {
    let tools = get_tool_definitions();
    
    // Map of tool names to their expected MCP prefixed versions
    // Claude Code expects these exact patterns
    let expected_patterns = vec![
        ("read_file", "mcp__*__read_file"),
        ("write_file", "mcp__*__write_file"),
        ("edit_file", "mcp__*__edit_file"),
        ("multi_edit_file", "mcp__*__multi_edit_file"),
        ("shell_execute", "mcp__*__shell_execute"),
        ("todo_read", "mcp__*__todo_read"),
        ("todo_write", "mcp__*__todo_write"),
        ("semantic_code_search", "mcp__*__semantic_code_search"),
        ("search_file", "mcp__*__search_file"),
        ("repository_add", "mcp__*__repository_add"),
        ("repository_list", "mcp__*__repository_list"),
        ("repository_sync", "mcp__*__repository_sync"),
    ];
    
    for (tool_name, _expected_pattern) in expected_patterns {
        assert!(
            tools.iter().any(|t| t.name == tool_name),
            "Tool '{}' is expected by Claude Code but not found in MCP tools",
            tool_name
        );
    }
}