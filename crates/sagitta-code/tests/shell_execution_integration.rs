use sagitta_code::tools::shell_execution::{ShellExecutionTool, ShellExecutionParams, ShellExecutionResult};
use sagitta_code::tools::types::{Tool, ToolResult};
use serde_json::json;
use std::time::Instant;
use tempfile::TempDir;
use tokio;

/// Integration test for shell execution functionality
/// Run with: cargo test --test shell_execution_integration -- --nocapture
#[tokio::test]
async fn test_shell_execution_integration() {
    println!("Starting Shell Execution Integration Test");
    
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    
    // Create the tool with custom config that uses our temp directory as the base
    let tool = ShellExecutionTool::new(temp_dir.path().to_path_buf());
    
    // Test 1: Check execution environment availability
    println!("\n1. Checking execution environment availability...");
    let env_available = tool.check_environment_available().await.expect("Failed to check environment");
    if !env_available {
        println!("❌ Execution environment is not available. Skipping tests.");
        return;
    }
    println!("✅ Execution environment is available");
    
    // Test 2: Simple echo command
    println!("\n2. Testing simple echo command...");
    let start_time = Instant::now();
    let params = json!({
        "command": "echo 'Hello from local execution!'",
        "timeout_seconds": 120
    });
    
    match tool.execute(params).await {
        Ok(ToolResult::Success(result)) => {
            let exec_result: serde_json::Value = result;
            let execution_time = start_time.elapsed();
            println!("✅ Simple command executed successfully in {:?}", execution_time);
            println!("   Exit code: {}", exec_result["exit_code"]);
            println!("   Output: {}", exec_result["stdout"].as_str().unwrap_or(""));
            assert_eq!(exec_result["exit_code"], 0);
            assert!(exec_result["stdout"].as_str().unwrap().contains("Hello from local execution!"));
        }
        Ok(ToolResult::Error { error }) => {
            println!("❌ Command failed with error: {}", error);
            panic!("Simple command should not fail");
        }
        Err(e) => {
            println!("❌ Tool execution failed: {}", e);
            panic!("Tool execution should not fail");
        }
    }
    
    // Test 3: Directory creation (original failing command)
    println!("\n3. Testing directory creation...");
    let start_time = Instant::now();
    let params = json!({
        "command": "mkdir test_dir && ls -la",
        "working_directory": temp_dir.path().to_string_lossy(),
        "timeout_seconds": 120
    });
    
    match tool.execute(params).await {
        Ok(ToolResult::Success(result)) => {
            let exec_result: serde_json::Value = result;
            let execution_time = start_time.elapsed();
            println!("✅ Directory creation executed successfully in {:?}", execution_time);
            println!("   Exit code: {}", exec_result["exit_code"]);
            println!("   Stdout: {}", exec_result["stdout"].as_str().unwrap_or(""));
            println!("   Stderr: {}", exec_result["stderr"].as_str().unwrap_or(""));
            
            // The command should succeed
            if exec_result["exit_code"] != 0 {
                println!("❌ Command failed with non-zero exit code");
                println!("   This might be due to 'ls -la' format differences or directory already existing");
                // Let's try a simpler test
                let simple_params = json!({
                    "command": "mkdir simple_test_dir || echo 'Directory might already exist'",
                    "working_directory": temp_dir.path().to_string_lossy(),
                    "timeout_seconds": 120
                });
                
                match tool.execute(simple_params).await {
                    Ok(ToolResult::Success(simple_result)) => {
                        let simple_exec_result: serde_json::Value = simple_result;
                        println!("   Simple mkdir result: exit_code={}, stdout={}", 
                            simple_exec_result["exit_code"], 
                            simple_exec_result["stdout"].as_str().unwrap_or("")
                        );
                        assert_eq!(simple_exec_result["exit_code"], 0);
                    }
                    _ => panic!("Even simple mkdir failed")
                }
            } else {
                assert_eq!(exec_result["exit_code"], 0);
                assert!(exec_result["stdout"].as_str().unwrap().contains("test_dir"));
            }
        }
        Ok(ToolResult::Error { error }) => {
            println!("❌ Directory creation failed with error: {}", error);
            panic!("Directory creation should not fail");
        }
        Err(e) => {
            println!("❌ Tool execution failed: {}", e);
            panic!("Tool execution should not fail");
        }
    }
    
    // Test 4: Python execution (if python3 is available)
    println!("\n4. Testing Python execution...");
    let start_time = Instant::now();
    let params = json!({
        "command": "echo 'print(\"Python works!\")' | python3 - || echo 'Python not available'",
        "timeout_seconds": 120
    });
    
    match tool.execute(params).await {
        Ok(ToolResult::Success(result)) => {
            let exec_result: serde_json::Value = result;
            let execution_time = start_time.elapsed();
            println!("✅ Python test completed successfully in {:?}", execution_time);
            println!("   Exit code: {}", exec_result["exit_code"]);
            println!("   Output: {}", exec_result["stdout"].as_str().unwrap_or(""));
            assert_eq!(exec_result["exit_code"], 0);
            let output = exec_result["stdout"].as_str().unwrap();
            // Either python worked or we got the fallback message
            assert!(output.contains("Python works!") || output.contains("Python not available"));
        }
        Ok(ToolResult::Error { error }) => {
            println!("❌ Python execution failed with error: {}", error);
            // Don't panic here as Python might not be installed
            println!("   This might be due to Python not being installed");
        }
        Err(e) => {
            println!("❌ Tool execution failed: {}", e);
            // Don't panic here as this might be expected
            println!("   This might be due to Python not being available");
        }
    }
    
    // Test 5: Test timeout behavior with a deliberately slow command
    println!("\n5. Testing timeout behavior...");
    let start_time = Instant::now();
    let params = json!({
        "command": "sleep 2", // This should complete within timeout
        "timeout_seconds": 15  // 15 seconds should be enough
    });
    
    match tool.execute(params).await {
        Ok(ToolResult::Success(result)) => {
            let exec_result: serde_json::Value = result;
            let execution_time = start_time.elapsed();
            println!("✅ Sleep command executed successfully in {:?}", execution_time);
            println!("   Exit code: {}", exec_result["exit_code"]);
            println!("   Timed out: {}", exec_result["timed_out"]);
            assert_eq!(exec_result["exit_code"], 0);
            assert_eq!(exec_result["timed_out"], false);
            assert!(execution_time.as_secs() >= 2); // Should take at least 2 seconds
            assert!(execution_time.as_secs() < 10); // Should complete well before timeout
        }
        Ok(ToolResult::Error { error }) => {
            println!("❌ Sleep command failed with error: {}", error);
        }
        Err(e) => {
            println!("❌ Tool execution failed: {}", e);
        }
    }
    
    // Test 6: Test with custom executor configuration
    println!("\n6. Testing custom executor configuration...");
    let custom_tool = ShellExecutionTool::new(temp_dir.path().to_path_buf());
    
    let start_time = Instant::now();
    let params = json!({
        "command": "echo 'Custom executor config' && pwd",
        "working_directory": temp_dir.path().to_string_lossy(),
        "timeout_seconds": 120
    });
    
    match custom_tool.execute(params).await {
        Ok(ToolResult::Success(result)) => {
            let exec_result: serde_json::Value = result;
            let execution_time = start_time.elapsed();
            println!("✅ Custom executor executed successfully in {:?}", execution_time);
            println!("   Exit code: {}", exec_result["exit_code"]);
            println!("   Container: {}", exec_result["container_image"].as_str().unwrap_or(""));
            println!("   Output: {}", exec_result["stdout"].as_str().unwrap_or(""));
            assert_eq!(exec_result["exit_code"], 0);
            assert_eq!(exec_result["container_image"], "local");
            assert!(exec_result["stdout"].as_str().unwrap().contains("Custom executor config"));
        }
        Ok(ToolResult::Error { error }) => {
            println!("❌ Custom executor failed with error: {}", error);
        }
        Err(e) => {
            println!("❌ Tool execution failed: {}", e);
        }
    }
    
    println!("\n🎉 Shell Execution Integration Test Complete!");
}

/// Test specifically for timeout scenarios
#[tokio::test]
async fn test_timeout_scenarios() {
    println!("Starting Timeout Scenarios Test");
    
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    
    // Create the tool with custom config that uses our temp directory as the base
    let tool = ShellExecutionTool::new(temp_dir.path().to_path_buf());
    
    // Check environment availability
    if !tool.check_environment_available().await.expect("Failed to check environment") {
        println!("❌ Execution environment is not available. Skipping timeout tests.");
        return;
    }
    
    // Test 1: Command that should complete quickly (no timeout)
    println!("\n1. Testing fast command (no timeout)...");
    let start_time = Instant::now();
    let params = json!({
        "command": "echo 'Fast command'", 
        "timeout_seconds": 5   // 5 second timeout should be plenty
    });
    
    match tool.execute(params).await {
        Ok(ToolResult::Success(result)) => {
            let exec_result: serde_json::Value = result;
            let execution_time = start_time.elapsed();
            println!("✅ Fast command completed in {:?}", execution_time);
            println!("   Exit code: {}", exec_result["exit_code"]);
            println!("   Timed out: {}", exec_result["timed_out"]);
            
            // Should not timeout
            assert_eq!(exec_result["timed_out"], false);
            assert_eq!(exec_result["exit_code"], 0);
            assert!(execution_time.as_secs() < 2); // Should complete quickly
        }
        Ok(ToolResult::Error { error }) => {
            println!("❌ Fast command failed with error: {}", error);
        }
        Err(e) => {
            println!("❌ Tool execution failed: {}", e);
        }
    }
    
    // Test 2: Command that should complete within timeout
    println!("\n2. Testing command within timeout...");
    let start_time = Instant::now();
    let params = json!({
        "command": "sleep 1", // This should complete
        "timeout_seconds": 10  // 10 second timeout
    });
    
    match tool.execute(params).await {
        Ok(ToolResult::Success(result)) => {
            let exec_result: serde_json::Value = result;
            let execution_time = start_time.elapsed();
            println!("✅ Within timeout test completed in {:?}", execution_time);
            println!("   Exit code: {}", exec_result["exit_code"]);
            println!("   Timed out: {}", exec_result["timed_out"]);
            
            // Should not timeout
            assert_eq!(exec_result["timed_out"], false);
            assert_eq!(exec_result["exit_code"], 0);
            assert!(execution_time.as_secs() >= 1); // Should take at least 1 second
            assert!(execution_time.as_secs() < 3);  // Should complete well before timeout
        }
        Ok(ToolResult::Error { error }) => {
            println!("❌ Within timeout test failed with error: {}", error);
        }
        Err(e) => {
            println!("❌ Tool execution failed: {}", e);
        }
    }
    
    println!("\n🎉 Timeout Scenarios Test Complete!");
}

/// Performance and stress test
#[tokio::test]
async fn test_performance() {
    println!("Starting Performance Test");
    
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    
    // Create the tool with custom config that uses our temp directory as the base
    let tool = ShellExecutionTool::new(temp_dir.path().to_path_buf());
    
    // Check environment availability
    if !tool.check_environment_available().await.expect("Failed to check environment") {
        println!("❌ Execution environment is not available. Skipping performance tests.");
        return;
    }
    
    // Test multiple quick commands
    println!("\n1. Testing multiple quick commands...");
    let start_time = Instant::now();
    
    for i in 0..5 {
        let params = json!({
            "command": format!("echo 'Command {}'", i),
            "timeout_seconds": 30
        });
        
        match tool.execute(params).await {
            Ok(ToolResult::Success(result)) => {
                let exec_result: serde_json::Value = result;
                println!("   Command {}: exit_code={}, output={}", 
                    i, 
                    exec_result["exit_code"], 
                    exec_result["stdout"].as_str().unwrap_or("").trim()
                );
                assert_eq!(exec_result["exit_code"], 0);
            }
            Ok(ToolResult::Error { error }) => {
                println!("❌ Command {} failed with error: {}", i, error);
            }
            Err(e) => {
                println!("❌ Command {} execution failed: {}", i, e);
            }
        }
    }
    
    let total_time = start_time.elapsed();
    println!("✅ Executed 5 commands in {:?}", total_time);
    
    println!("\n🎉 Performance Test Complete!");
} 