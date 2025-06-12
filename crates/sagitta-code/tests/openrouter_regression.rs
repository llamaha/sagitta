/// OpenRouter Integration Regression Tests
/// 
/// These tests are designed to fail initially and serve as a safety net
/// for fixing the critical OpenRouter integration issues in Sagitta Code.
/// 
/// Issues being tested:
/// 1. 400 Bad Request from OpenRouter API 
/// 2. Complete Tool Execution Failure
/// 3. Severe Text Duplication
/// 4. Stream Handler Architecture Issues

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use wiremock::{MockServer, Mock, ResponseTemplate, Request, matchers::{method, path, header, body_partial_json}};
use serde_json::{json, Value};
use futures_util::{Stream, StreamExt};
use uuid::Uuid;

use sagitta_code::config::types::{SagittaCodeConfig, OpenRouterConfig};
use sagitta_code::llm::openrouter::client::OpenRouterClient;
use sagitta_code::llm::client::{LlmClient, Message, Role, MessagePart, ToolDefinition, StreamChunk};
use sagitta_code::tools::registry::ToolRegistry;
use sagitta_code::tools::types::{Tool, ToolResult, ToolDefinition as ToolTypesDefinition, ToolCategory, BoxedTool};
use sagitta_code::utils::errors::SagittaCodeError;

/// Mock OpenRouter server helper for capturing exact requests
struct MockOpenRouterServer {
    server: MockServer,
    captured_requests: Arc<Mutex<Vec<Value>>>,
}

impl MockOpenRouterServer {
    async fn new() -> Self {
        let server = MockServer::start().await;
        let captured_requests = Arc::new(Mutex::new(Vec::new()));
        
        Self {
            server,
            captured_requests,
        }
    }
    
    /// Setup mock for chat completions that captures the exact request JSON
    async fn setup_chat_completions_capture(&self) {
        let captured = self.captured_requests.clone();
        
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(move |req: &Request| {
                // Capture the request body for analysis
                if let Some(body_str) = std::str::from_utf8(&req.body).ok() {
                    if let Ok(json_body) = serde_json::from_str::<Value>(body_str) {
                        captured.lock().unwrap().push(json_body);
                    }
                }
                
                // Return a valid response to avoid hanging the test
                ResponseTemplate::new(200)
                    .set_body_json(json!({
                        "id": "test-completion",
                        "object": "chat.completion",
                        "model": "openai/gpt-4",
                        "choices": [{
                            "index": 0,
                            "message": {
                                "role": "assistant",
                                "content": "Test response"
                            },
                            "finish_reason": "stop"
                        }],
                        "usage": {
                            "prompt_tokens": 10,
                            "completion_tokens": 5,
                            "total_tokens": 15
                        }
                    }))
            })
            .mount(&self.server)
            .await;
    }
    
    /// Setup mock for streaming that returns tool calls
    async fn setup_streaming_with_tool_calls(&self) {
        let captured = self.captured_requests.clone();
        
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .and(body_partial_json(json!({"stream": true})))
            .respond_with(move |req: &Request| {
                // Capture the request body
                if let Some(body_str) = std::str::from_utf8(&req.body).ok() {
                    if let Ok(json_body) = serde_json::from_str::<Value>(body_str) {
                        captured.lock().unwrap().push(json_body);
                    }
                }
                
                // Return SSE stream with tool call
                let sse_data = vec![
                    "data: {\"id\":\"test-stream\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"I'll create a new branch for you.\"}}]}",
                    "data: {\"id\":\"test-stream\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_123\",\"type\":\"function\",\"function\":{\"name\":\"create_branch\",\"arguments\":\"{\\\"name\\\":\\\"test-branch\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}]}",
                    "data: [DONE]"
                ].join("\n\n");
                
                ResponseTemplate::new(200)
                    .set_body_string(sse_data)
                    .insert_header("content-type", "text/event-stream")
            })
            .mount(&self.server)
            .await;
    }
    
    /// Setup mock that always returns 400 Bad Request for diagnosis
    async fn setup_bad_request_capture(&self) {
        let captured = self.captured_requests.clone();
        
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(move |req: &Request| {
                // Capture the malformed request
                if let Some(body_str) = std::str::from_utf8(&req.body).ok() {
                    if let Ok(json_body) = serde_json::from_str::<Value>(body_str) {
                        captured.lock().unwrap().push(json_body);
                    }
                }
                
                ResponseTemplate::new(400)
                    .set_body_json(json!({
                        "error": {
                            "message": "Invalid request: missing required field",
                            "type": "invalid_request_error"
                        }
                    }))
            })
            .mount(&self.server)
            .await;
    }
    
    /// Get all captured requests for analysis
    fn get_captured_requests(&self) -> Vec<Value> {
        self.captured_requests.lock().unwrap().clone()
    }
    
    /// Get the server URI for client configuration
    fn uri(&self) -> String {
        self.server.uri()
    }
}

/// Mock tool for testing tool execution
#[derive(Debug, Clone)]
struct MockCreateBranchTool {
    execution_count: Arc<Mutex<u32>>,
}

impl MockCreateBranchTool {
    fn new() -> Self {
        Self {
            execution_count: Arc::new(Mutex::new(0)),
        }
    }
    
    fn get_execution_count(&self) -> u32 {
        *self.execution_count.lock().unwrap()
    }
}

#[async_trait::async_trait]
impl Tool for MockCreateBranchTool {
    fn definition(&self) -> ToolTypesDefinition {
        ToolTypesDefinition {
            name: "create_branch".to_string(),
            description: "Create a new git branch".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Name of the branch to create"
                    }
                },
                "required": ["name"]
            }),
            is_required: false,
            category: ToolCategory::Repository,
            metadata: HashMap::new(),
        }
    }
    
    async fn execute(&self, parameters: serde_json::Value) -> Result<ToolResult, SagittaCodeError> {
        // Increment execution count
        *self.execution_count.lock().unwrap() += 1;
        
        Ok(ToolResult::success(json!({
            "message": "Branch created successfully",
            "branch_name": parameters.get("name").unwrap_or(&json!("unknown"))
        })))
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Create test configuration with mock server
fn create_test_config_with_mock_server(mock_server_uri: &str) -> SagittaCodeConfig {
    SagittaCodeConfig {
        openrouter: OpenRouterConfig {
            api_key: Some("test-api-key".to_string()),
            model: "openai/gpt-4".to_string(),
            provider_preferences: None,
            max_history_size: 10,
            max_reasoning_steps: 5,
            request_timeout: 30,
        },
        // Other fields would be initialized with defaults
        ..Default::default()
    }
}

/// TEST 1: 400 Bad Request Diagnosis
/// This test captures the exact JSON we send to OpenRouter to identify malformation
#[tokio::test]
async fn test_bad_request_diagnosis() {
    let mock_server = MockOpenRouterServer::new().await;
    mock_server.setup_bad_request_capture().await;
    
    let config = create_test_config_with_mock_server(&mock_server.uri());
    
    // Create client with mock server URL 
    let client = OpenRouterClient::new_with_base_url(&config, &mock_server.uri()).expect("Failed to create client");
    
    let messages = vec![
        Message {
            id: Uuid::new_v4(),
            role: Role::User,
            parts: vec![MessagePart::Text { text: "Hello, create a branch called test".to_string() }],
            metadata: HashMap::new(),
        }
    ];
    
    let tools = vec![
        ToolDefinition {
            name: "create_branch".to_string(),
            description: "Create a new git branch".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Name of the branch to create"
                    }
                },
                "required": ["name"]
            }),
            is_required: false,
        }
    ];
    
    // This should fail with 400 Bad Request - we want to capture WHY
    let result = client.generate(&messages, &tools).await;
    
    // Verify we got a 400 error
    assert!(result.is_err(), "Expected 400 error but request succeeded");
    
    // Capture and analyze the request JSON
    let captured_requests = mock_server.get_captured_requests();
    assert!(!captured_requests.is_empty(), "No requests were captured");
    
    let request_json = &captured_requests[0];
    
    // Save the malformed request to a golden file for manual inspection
    // TODO: Add golden file comparison once we know what SHOULD be sent
    println!("CAPTURED MALFORMED REQUEST: {}", serde_json::to_string_pretty(request_json).unwrap());
    
    // TODO: Once we fix the issue, this test should compare against a golden file
    // and pass without 400 errors
    panic!("This test is designed to fail initially - captured request for analysis");
}

/// TEST 2: Tool Execution Failure 
/// This test verifies that tool calls from the LLM actually execute our tools
#[tokio::test] 
async fn test_tool_call_not_executed() {
    let mock_server = MockOpenRouterServer::new().await;
    mock_server.setup_streaming_with_tool_calls().await;
    
    let config = create_test_config_with_mock_server(&mock_server.uri());
    let client = Arc::new(OpenRouterClient::new_with_base_url(&config, &mock_server.uri()).expect("Failed to create client"));
    
    // Create mock tool and registry
    let mock_tool: BoxedTool = Arc::new(MockCreateBranchTool::new());
    let tool_registry = Arc::new(ToolRegistry::new());
    tool_registry.register(mock_tool.clone()).await
        .expect("Failed to register tool");
    
    let messages = vec![
        Message {
            id: Uuid::new_v4(),
            role: Role::User, 
            parts: vec![MessagePart::Text { text: "Create a new branch called test-branch".to_string() }],
            metadata: HashMap::new(),
        }
    ];
    
    let tools = vec![
        ToolDefinition {
            name: "create_branch".to_string(),
            description: "Create a new git branch".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Name of the branch to create"
                    }
                },
                "required": ["name"]
            }),
            is_required: false,
        }
    ];
    
    // Start streaming (which should include tool calls from our mock)
    let mut stream = client.generate_stream(&messages, &tools).await
        .expect("Failed to start stream");
    
    // Collect all chunks
    let mut chunks = Vec::new();
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.expect("Stream chunk error");
        chunks.push(chunk);
        
        // TODO: This is where we need to detect tool calls and actually execute them
        // Currently, the agent receives tool call chunks but doesn't execute
    }
    
    // Verify we received tool call chunks
    let has_tool_call = chunks.iter().any(|chunk| {
        matches!(chunk.part, MessagePart::ToolCall { .. })
    });
    
    assert!(has_tool_call, "Expected to receive tool call chunks but didn't");
    
    // Verify the tool was actually executed - cast back to our mock tool
    let mock_tool_ref = mock_tool.as_any().downcast_ref::<MockCreateBranchTool>().unwrap();
    let execution_count = mock_tool_ref.get_execution_count();
    assert_eq!(execution_count, 1, "Expected tool to be executed once but was executed {} times", execution_count);
    
    // TODO: This test is designed to fail until we fix tool execution
    panic!("This test is designed to fail initially - tool execution is not working");
}

/// TEST 3: Text Duplication in Streaming
/// This test verifies that each chunk is only processed once, not multiple times
#[tokio::test]
async fn test_duplicate_stream_chunks() {
    let mock_server = MockOpenRouterServer::new().await;
    
    // Setup a mock that returns exactly 3 distinct chunks
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(body_partial_json(json!({"stream": true})))
        .respond_with(ResponseTemplate::new(200)
            .set_body_string(
                "data: {\"id\":\"test-1\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"}}]}\n\n\
                 data: {\"id\":\"test-2\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\" World\"}}]}\n\n\
                 data: {\"id\":\"test-3\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"!\"}}]}\n\n\
                 data: [DONE]"
            )
            .insert_header("content-type", "text/event-stream"))
        .mount(&mock_server.server)
        .await;
    
    let config = create_test_config_with_mock_server(&mock_server.uri());
    let client = Arc::new(OpenRouterClient::new_with_base_url(&config, &mock_server.uri()).expect("Failed to create client"));
    
    let messages = vec![
        Message {
            id: Uuid::new_v4(),
            role: Role::User,
            parts: vec![MessagePart::Text { text: "Say hello".to_string() }],
            metadata: HashMap::new(),
        }
    ];
    
    // Create multiple consumers to simulate Agent + GUI
    let (sender1, mut _receiver1): (mpsc::UnboundedSender<usize>, mpsc::UnboundedReceiver<usize>) = mpsc::unbounded_channel();
    let (sender2, mut _receiver2): (mpsc::UnboundedSender<usize>, mpsc::UnboundedReceiver<usize>) = mpsc::unbounded_channel();
    
    // Start streaming
    let mut stream = client.generate_stream(&messages, &[]).await
        .expect("Failed to start stream");
    
    // Simulate multiple consumers reading from the same stream
    let consumer1_handle = tokio::spawn(async move {
        let mut chunks = Vec::new();
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.expect("Stream error");
            chunks.push(chunk);
            let _ = sender1.send(chunks.len());
        }
        chunks
    });
    
    // TODO: This simulates the current broken architecture where GUI also tries to read
    // In reality, there should be only ONE stream reader with broadcast to consumers
    
    let consumer1_chunks = consumer1_handle.await.expect("Consumer 1 failed");
    
    // Verify we got exactly 3 text chunks (not 6-12 due to duplication)
    let text_chunks: Vec<_> = consumer1_chunks.iter()
        .filter(|chunk| matches!(chunk.part, MessagePart::Text { .. }))
        .collect();
    
    assert_eq!(text_chunks.len(), 3, "Expected exactly 3 text chunks but got {}", text_chunks.len());
    
    // Verify the content is not duplicated
    let full_text: String = text_chunks.iter()
        .filter_map(|chunk| match &chunk.part {
            MessagePart::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    
    assert_eq!(full_text, "Hello World!", "Expected 'Hello World!' but got '{}'", full_text);
    
    // TODO: This test may fail due to current duplication issues
    // Once fixed, it should pass
}

/// TEST 4: Single Stream Consumer Architecture
/// This test verifies that only one consumer reads from the network stream
/// while others subscribe to an internal broadcast
#[tokio::test]
async fn test_single_stream_consumer() {
    let mock_server = MockOpenRouterServer::new().await;
    
    // Setup mock that tracks how many times it's called
    let call_count = Arc::new(Mutex::new(0));
    let call_count_clone = call_count.clone();
    
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(move |_req: &Request| {
            *call_count_clone.lock().unwrap() += 1;
            ResponseTemplate::new(200)
                .set_body_string(
                    "data: {\"id\":\"test\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Test\"}}]}\n\n\
                     data: [DONE]"
                )
                .insert_header("content-type", "text/event-stream")
        })
        .mount(&mock_server.server)
        .await;
    
    let config = create_test_config_with_mock_server(&mock_server.uri());
    let client = Arc::new(OpenRouterClient::new_with_base_url(&config, &mock_server.uri()).expect("Failed to create client"));
    
    let messages = vec![
        Message {
            id: Uuid::new_v4(),
            role: Role::User,
            parts: vec![MessagePart::Text { text: "Test".to_string() }],
            metadata: HashMap::new(),
        }
    ];
    
    // Simulate multiple subscribers (Agent + GUI)
    let (broadcast_sender, _): (broadcast::Sender<StreamChunk>, broadcast::Receiver<StreamChunk>) = broadcast::channel(100);
    let mut _subscriber1 = broadcast_sender.subscribe();
    let mut _subscriber2 = broadcast_sender.subscribe();
    
    // TODO: Implement the correct architecture where:
    // 1. Only ONE task reads from the HTTP stream
    // 2. That task broadcasts chunks to all subscribers
    // 3. No duplicate HTTP calls are made
    
    // For now, this simulates the broken behavior
    let client1 = client.clone();
    let client2 = client.clone();
    let messages1 = messages.clone();
    let messages2 = messages.clone();
    
    // Two "consumers" both trying to create streams (BAD ARCHITECTURE)
    let task1 = tokio::spawn(async move {
        let mut stream = client1.generate_stream(&messages1, &[]).await.unwrap();
        let mut count = 0;
        while let Some(_) = stream.next().await {
            count += 1;
        }
        count
    });
    
    let task2 = tokio::spawn(async move {
        let mut stream = client2.generate_stream(&messages2, &[]).await.unwrap();
        let mut count = 0;
        while let Some(_) = stream.next().await {
            count += 1;
        }
        count
    });
    
    let (count1, count2) = tokio::join!(task1, task2);
    let count1 = count1.expect("Task 1 failed");
    let count2 = count2.expect("Task 2 failed");
    
    // Check how many HTTP calls were made
    let http_calls = *call_count.lock().unwrap();
    
    // Currently this will likely be 2 (broken), should be 1 (fixed)
    println!("HTTP calls made: {}, Consumer 1 chunks: {}, Consumer 2 chunks: {}", http_calls, count1, count2);
    
    assert_eq!(http_calls, 1, "Expected exactly 1 HTTP call but got {}", http_calls);
    
    // TODO: This test is designed to fail initially due to multiple stream consumers
    panic!("This test is designed to fail initially - multiple HTTP streams being created");
}

/// Integration test that combines all the issues
#[tokio::test]
async fn test_end_to_end_openrouter_integration() {
    // This test will be implemented once the individual issues are fixed
    // It should test the complete flow: request -> stream -> tool execution -> response
    todo!("Implement after fixing individual issues");
}

#[cfg(test)]
mod golden_file_tests {
    use super::*;
    
    /// Compare captured requests against known good requests
    #[tokio::test]
    async fn test_request_format_golden_file() {
        // TODO: Once we know what a correct OpenRouter request looks like,
        // save it as a golden file and compare against it in tests
        todo!("Implement golden file comparison");
    }
} 