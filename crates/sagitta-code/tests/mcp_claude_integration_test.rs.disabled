use sagitta_code::config::SagittaCodeConfig;
use sagitta_code::llm::claude_code::client::ClaudeCodeClient;
use sagitta_code::tools::registry::ToolRegistry;
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn test_mcp_integration_with_claude_client() {
    // Create a temporary directory for test files
    let temp_dir = tempdir().unwrap();
    
    // Create a test config
    let config = SagittaCodeConfig::default();
    
    // Create Claude client
    let mut client = ClaudeCodeClient::new(&config).unwrap();
    
    // Create and populate tool registry
    let tool_registry = Arc::new(ToolRegistry::new());
    
    // Register a test tool
    use sagitta_code::tools::shell_execution::ShellExecutionTool;
    tool_registry.register(Arc::new(ShellExecutionTool::new(temp_dir.path().to_path_buf())))
        .await
        .unwrap();
    
    // Initialize MCP integration
    let result = client.initialize_mcp(tool_registry).await;
    assert!(result.is_ok(), "MCP initialization failed: {:?}", result.err());
    
    // Get the MCP integration details
    let integration = client.get_mcp_integration();
    assert!(integration.is_some(), "MCP integration should be present after initialization");
    
    let mcp_info = integration.unwrap();
    assert!(mcp_info["mcp_config_path"].is_string(), "Should have MCP config path");
    assert!(mcp_info["server_name"].is_string(), "Should have server name");
    
    // Read the generated MCP config file
    let config_path = mcp_info["mcp_config_path"].as_str().unwrap();
    let config_content = std::fs::read_to_string(config_path).unwrap();
    println!("MCP config content: {}", config_content);
    let config_json: serde_json::Value = serde_json::from_str(&config_content).unwrap();
    
    // Verify the config structure
    assert!(config_json["mcpServers"].is_object(), "mcpServers should be an object");
    let server_name = mcp_info["server_name"].as_str().unwrap();
    println!("Server name: {}", server_name);
    assert!(config_json["mcpServers"][server_name].is_object(), "Server config should exist for {}", server_name);
    
    let server_config = &config_json["mcpServers"][server_name];
    assert!(server_config["command"].is_string());
    assert!(server_config["args"].is_array());
    assert!(server_config["env"].is_object());
    assert_eq!(server_config["stdin"], "pipe");
    assert_eq!(server_config["stdout"], "pipe");
    assert_eq!(server_config["stderr"], "pipe");
    
    // The command should be the current executable
    let command = server_config["command"].as_str().unwrap();
    println!("Command: {}", command);
    // In test context, it will be the test binary, which is fine
    assert!(!command.is_empty(), "Command should not be empty");
    
    // Args should contain --mcp-internal
    let args = server_config["args"].as_array().unwrap();
    assert!(args.iter().any(|arg| arg.as_str() == Some("--mcp-internal")));
}

#[tokio::test]
async fn test_mcp_integration_provides_claude_cli_args() {
    let temp_dir = tempdir().unwrap();
    let config = SagittaCodeConfig::default();
    
    let mut client = ClaudeCodeClient::new(&config).unwrap();
    let tool_registry = Arc::new(ToolRegistry::new());
    
    // Initialize MCP
    client.initialize_mcp(tool_registry).await.unwrap();
    
    // Get command line args for Claude CLI
    let claude_args = client.get_claude_cli_args();
    assert!(claude_args.is_some(), "Should provide Claude CLI args when MCP is initialized");
    
    let args = claude_args.unwrap();
    assert!(args.iter().any(|arg| arg == "--mcp-config"), "Should have --mcp-config flag");
    
    // Find the config path in args
    let mut found_config = false;
    for (i, arg) in args.iter().enumerate() {
        if arg == "--mcp-config" && i + 1 < args.len() {
            let config_path = &args[i + 1];
            assert!(std::path::Path::new(config_path).exists(), "MCP config file should exist");
            found_config = true;
            break;
        }
    }
    assert!(found_config, "Should have found MCP config path in args");
}