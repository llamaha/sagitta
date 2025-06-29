use std::process::{Command, Stdio};
use std::io::{Write, BufRead, BufReader};
use tempfile::NamedTempFile;
use serde_json::json;

#[test]
fn test_mcp_internal_server_starts() {
    // Get the path to our binary
    let exe_path = env!("CARGO_BIN_EXE_sagitta-code");
    
    // Test that --mcp-internal flag works
    let mut child = Command::new(exe_path)
        .arg("--mcp-internal")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn MCP server");
    
    // Check stderr for any immediate errors
    let stderr = child.stderr.take().unwrap();
    let stderr_reader = std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(line) = line {
                eprintln!("STDERR: {}", line);
            }
        }
    });
    
    // Send an initialize request
    let stdin = child.stdin.as_mut().unwrap();
    let request = json!({
        "jsonrpc": "2.0",
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            }
        },
        "id": 1
    });
    
    writeln!(stdin, "{}", request).expect("Failed to write request");
    stdin.flush().expect("Failed to flush stdin");
    
    // Read the response
    let stdout = child.stdout.as_mut().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut response_line = String::new();
    
    // Set a timeout for reading
    let start = std::time::Instant::now();
    loop {
        if start.elapsed() > std::time::Duration::from_secs(5) {
            panic!("Timeout waiting for MCP response");
        }
        
        match reader.read_line(&mut response_line) {
            Ok(0) => panic!("EOF reached without response"),
            Ok(_) => {
                if !response_line.trim().is_empty() {
                    break;
                }
            }
            Err(e) => panic!("Error reading response: {}", e),
        }
    }
    
    // Parse and verify the response
    println!("Raw response: {:?}", response_line);
    let response: serde_json::Value = serde_json::from_str(&response_line)
        .unwrap_or_else(|e| panic!("Failed to parse response: {:?}\nRaw response: {}", e, response_line));
    
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert!(response.get("result").is_some());
    
    let result = &response["result"];
    assert_eq!(result["protocolVersion"], "2024-11-05");
    assert_eq!(result["serverInfo"]["name"], "sagitta-mcp-enhanced");
    
    // Clean up
    let _ = child.kill();
}

#[test]
fn test_mcp_tools_list() {
    let exe_path = env!("CARGO_BIN_EXE_sagitta-code");
    
    let mut child = Command::new(exe_path)
        .arg("--mcp-internal")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn MCP server");
    
    let stdin = child.stdin.as_mut().unwrap();
    let stdout = child.stdout.as_mut().unwrap();
    let mut reader = BufReader::new(stdout);
    
    // Initialize first
    let init_request = json!({
        "jsonrpc": "2.0",
        "method": "initialize",
        "params": {},
        "id": 1
    });
    writeln!(stdin, "{}", init_request).unwrap();
    stdin.flush().unwrap();
    
    // Read init response
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    
    // Now request tools list
    let tools_request = json!({
        "jsonrpc": "2.0",
        "method": "tools/list",
        "params": {},
        "id": 2
    });
    writeln!(stdin, "{}", tools_request).unwrap();
    stdin.flush().unwrap();
    
    // Read tools response
    line.clear();
    reader.read_line(&mut line).unwrap();
    
    let response: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(response["id"], 2);
    
    let tools = response["result"]["tools"].as_array().unwrap();
    assert!(!tools.is_empty(), "Should have some tools registered");
    
    // Check that we have some expected tools
    let tool_names: Vec<String> = tools.iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    
    // These should exist based on the mcp_app initialization
    assert!(tool_names.iter().any(|n| n.contains("list")), "Should have list tool");
    assert!(tool_names.iter().any(|n| n.contains("shell") || n.contains("execution")), "Should have shell tool");
    
    let _ = child.kill();
}

#[test]
fn test_claude_mcp_config_format() {
    // Test that the MCP config file format is correct for Claude CLI
    let mut temp_file = NamedTempFile::new().unwrap();
    
    let config = json!({
        "mcpServers": {
            "test-server": {
                "command": "/path/to/binary",
                "args": ["--mcp-internal"],
                "env": {},
                "stdin": "pipe",
                "stdout": "pipe", 
                "stderr": "pipe"
            }
        }
    });
    
    temp_file.write_all(serde_json::to_string_pretty(&config).unwrap().as_bytes()).unwrap();
    temp_file.flush().unwrap();
    
    // Verify the file can be read back
    let content = std::fs::read_to_string(temp_file.path()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    
    assert!(parsed["mcpServers"].is_object());
    assert!(parsed["mcpServers"]["test-server"]["command"].is_string());
}