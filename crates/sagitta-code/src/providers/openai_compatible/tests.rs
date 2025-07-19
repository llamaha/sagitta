#[cfg(test)]
mod integration_tests {
    use crate::llm::client::{LlmClient, MessagePart, Message, Role};
    use crate::providers::openai_compatible::OpenAICompatibleClient;
    use crate::providers::types::OpenAICompatibleConfig;
    use crate::providers::claude_code::mcp_integration::McpIntegration;
    use futures::StreamExt;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path, header};
    use std::sync::Arc;
    use uuid::Uuid;
    use std::collections::HashMap;
    
    async fn create_mock_server() -> MockServer {
        MockServer::start().await
    }
    
    fn create_sse_response(events: Vec<&str>) -> String {
        events.iter()
            .map(|event| format!("data: {}\n\n", event))
            .collect::<Vec<_>>()
            .join("")
    }
    
    fn create_mock_mcp_integration() -> Arc<McpIntegration> {
        Arc::new(McpIntegration::new())
    }
    
    #[tokio::test]
    async fn test_streaming_text_response() {
        let mock_server = create_mock_server().await;
        
        // Create SSE response with text chunks
        let response_body = create_sse_response(vec![
            r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#,
            r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"content":" world"},"finish_reason":null}]}"#,
            r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#,
            "[DONE]",
        ]);
        
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .and(header("content-type", "application/json"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(response_body)
                    .insert_header("content-type", "text/event-stream")
            )
            .mount(&mock_server)
            .await;
        
        let config = OpenAICompatibleConfig {
            base_url: mock_server.uri(),
            api_key: Some("test-key".to_string()),
            model: Some("gpt-4".to_string()),
            timeout_seconds: 30,
            max_retries: 3,
        };
        
        let mcp_integration = create_mock_mcp_integration();
        let client = OpenAICompatibleClient::new(
            config.base_url.clone(),
            config.api_key.clone(),
            config.model.clone(),
            mcp_integration,
            config.timeout_seconds,
            3, // max_retries
        );
        
        let messages = vec![
            Message {
                id: Uuid::new_v4(),
                role: Role::User,
                parts: vec![MessagePart::Text { text: "Say hello".to_string() }],
                metadata: HashMap::new(),
            }
        ];
        
        let mut stream = client.generate_stream(&messages, &[]).await.unwrap();
        
        let mut collected_text = String::new();
        let mut final_seen = false;
        
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.unwrap();
            
            match &chunk.part {
                MessagePart::Text { text } => {
                    collected_text.push_str(text);
                }
                _ => panic!("Unexpected message part"),
            }
            
            if chunk.is_final {
                final_seen = true;
                assert_eq!(chunk.finish_reason, Some("stop".to_string()));
            }
        }
        
        assert_eq!(collected_text, "Hello world");
        assert!(final_seen);
    }
    
    #[tokio::test]
    async fn test_streaming_with_thinking() {
        let mock_server = create_mock_server().await;
        
        // Create SSE response with thinking tags
        let response_body = create_sse_response(vec![
            r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"content":"Let me think <thinking>"},"finish_reason":null}]}"#,
            r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"content":"This is complex"},"finish_reason":null}]}"#,
            r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"content":"</thinking> The answer is 42"},"finish_reason":null}]}"#,
            r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#,
            "[DONE]",
        ]);
        
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(response_body)
                    .insert_header("content-type", "text/event-stream")
            )
            .mount(&mock_server)
            .await;
        
        let config = OpenAICompatibleConfig {
            base_url: mock_server.uri(),
            api_key: Some("test-key".to_string()),
            model: Some("gpt-4".to_string()),
            timeout_seconds: 30,
            max_retries: 3,
        };
        
        let mcp_integration = create_mock_mcp_integration();
        let client = OpenAICompatibleClient::new(
            config.base_url.clone(),
            config.api_key.clone(),
            config.model.clone(),
            mcp_integration,
            config.timeout_seconds,
            3, // max_retries
        );
        
        let messages = vec![
            Message {
                id: Uuid::new_v4(),
                role: Role::User,
                parts: vec![MessagePart::Text { text: "What is the answer?".to_string() }],
                metadata: HashMap::new(),
            }
        ];
        
        let mut stream = client.generate_stream(&messages, &[]).await.unwrap();
        
        let mut parts = Vec::new();
        
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.unwrap();
            parts.push(chunk.part);
        }
        
        // Verify we got the right sequence (including final empty chunk)
        assert!(parts.len() >= 3);
        
        match &parts[0] {
            MessagePart::Text { text } => assert_eq!(text, "Let me think "),
            _ => panic!("Expected text"),
        }
        
        match &parts[1] {
            MessagePart::Thought { text } => assert_eq!(text, "This is complex"),
            _ => panic!("Expected thought"),
        }
        
        match &parts[2] {
            MessagePart::Text { text } => assert_eq!(text, " The answer is 42"),
            _ => panic!("Expected text"),
        }
        
        // Last part should be empty final chunk
        if parts.len() > 3 {
            match &parts[3] {
                MessagePart::Text { text } => assert!(text.is_empty()),
                _ => panic!("Expected empty text for final chunk"),
            }
        }
    }
    
    #[tokio::test]
    async fn test_streaming_tool_calls() {
        let mock_server = create_mock_server().await;
        
        // Create SSE response with tool calls
        let response_body = create_sse_response(vec![
            r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_abc123","type":"function","function":{"name":"get_weather","arguments":""}}]},"finish_reason":null}]}"#,
            r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"location\":"}}]},"finish_reason":null}]}"#,
            r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"San Francisco\",\"unit\":\"celsius\"}"}}]},"finish_reason":null}]}"#,
            r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#,
            "[DONE]",
        ]);
        
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(response_body)
                    .insert_header("content-type", "text/event-stream")
            )
            .mount(&mock_server)
            .await;
        
        let config = OpenAICompatibleConfig {
            base_url: mock_server.uri(),
            api_key: Some("test-key".to_string()),
            model: Some("gpt-4".to_string()),
            timeout_seconds: 30,
            max_retries: 3,
        };
        
        let mcp_integration = create_mock_mcp_integration();
        let client = OpenAICompatibleClient::new(
            config.base_url.clone(),
            config.api_key.clone(),
            config.model.clone(),
            mcp_integration,
            config.timeout_seconds,
            3, // max_retries
        );
        
        let messages = vec![
            Message {
                id: Uuid::new_v4(),
                role: Role::User,
                parts: vec![MessagePart::Text { text: "What's the weather?".to_string() }],
                metadata: HashMap::new(),
            }
        ];
        
        let tools = vec![
            crate::llm::client::ToolDefinition {
                name: "get_weather".to_string(),
                description: "Get weather information".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "location": {"type": "string"},
                        "unit": {"type": "string", "enum": ["celsius", "fahrenheit"]}
                    },
                    "required": ["location"]
                }),
                is_required: false,
            }
        ];
        
        let mut stream = client.generate_stream(&messages, &tools).await.unwrap();
        
        let mut tool_calls = Vec::new();
        
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.unwrap();
            
            match chunk.part {
                MessagePart::ToolCall { tool_call_id, name, parameters } => {
                    tool_calls.push((tool_call_id, name, parameters));
                }
                MessagePart::Text { text } if text.is_empty() => {
                    // Empty text chunk at the end is ok
                }
                _ => panic!("Unexpected message part: {:?}", chunk.part),
            }
        }
        
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].0, "call_abc123");
        assert_eq!(tool_calls[0].1, "get_weather");
        
        let params = &tool_calls[0].2;
        assert_eq!(params["location"], "San Francisco");
        assert_eq!(params["unit"], "celsius");
    }
    
    #[tokio::test]
    async fn test_streaming_multiple_tool_calls() {
        let mock_server = create_mock_server().await;
        
        // Create SSE response with multiple tool calls
        let response_body = create_sse_response(vec![
            r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"content":"I'll check the weather for both cities."},"finish_reason":null}]}"#,
            // First tool call
            r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_sf","type":"function","function":{"name":"get_weather","arguments":""}}]},"finish_reason":null}]}"#,
            r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"location\":\"San Francisco\"}"}}]},"finish_reason":null}]}"#,
            // Second tool call
            r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"tool_calls":[{"index":1,"id":"call_ny","type":"function","function":{"name":"get_weather","arguments":""}}]},"finish_reason":null}]}"#,
            r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"tool_calls":[{"index":1,"function":{"arguments":"{\"location\":\"New York\"}"}}]},"finish_reason":null}]}"#,
            r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#,
            "[DONE]",
        ]);
        
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(response_body)
                    .insert_header("content-type", "text/event-stream")
            )
            .mount(&mock_server)
            .await;
        
        let config = OpenAICompatibleConfig {
            base_url: mock_server.uri(),
            api_key: Some("test-key".to_string()),
            model: Some("gpt-4".to_string()),
            timeout_seconds: 30,
            max_retries: 3,
        };
        
        let mcp_integration = create_mock_mcp_integration();
        let client = OpenAICompatibleClient::new(
            config.base_url.clone(),
            config.api_key.clone(),
            config.model.clone(),
            mcp_integration,
            config.timeout_seconds,
            3, // max_retries
        );
        
        let messages = vec![
            Message {
                id: Uuid::new_v4(),
                role: Role::User,
                parts: vec![MessagePart::Text { text: "What's the weather in San Francisco and New York?".to_string() }],
                metadata: HashMap::new(),
            }
        ];
        
        let tools = vec![
            crate::llm::client::ToolDefinition {
                name: "get_weather".to_string(),
                description: "Get weather information".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "location": {"type": "string"}
                    },
                    "required": ["location"]
                }),
                is_required: false,
            }
        ];
        
        let mut stream = client.generate_stream(&messages, &tools).await.unwrap();
        
        let mut parts = Vec::new();
        let mut tool_calls = Vec::new();
        
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.unwrap();
            
            match chunk.part {
                MessagePart::ToolCall { tool_call_id, name, parameters } => {
                    tool_calls.push((tool_call_id, name, parameters));
                }
                MessagePart::Text { text } => {
                    parts.push(text);
                }
                _ => panic!("Unexpected message part: {:?}", chunk.part),
            }
        }
        
        // Verify we got both tool calls
        assert_eq!(tool_calls.len(), 2, "Expected 2 tool calls, got {}", tool_calls.len());
        
        // First tool call
        assert_eq!(tool_calls[0].0, "call_sf");
        assert_eq!(tool_calls[0].1, "get_weather");
        assert_eq!(tool_calls[0].2["location"], "San Francisco");
        
        // Second tool call
        assert_eq!(tool_calls[1].0, "call_ny");
        assert_eq!(tool_calls[1].1, "get_weather");
        assert_eq!(tool_calls[1].2["location"], "New York");
        
        // Verify we got the initial text
        assert!(parts.iter().any(|text| text.contains("I'll check the weather for both cities.")));
    }
    
    #[tokio::test]
    async fn test_streaming_error_handling() {
        let mock_server = create_mock_server().await;
        
        // Create SSE response with an error
        let response_body = create_sse_response(vec![
            r#"{"error":{"message":"Invalid API key","type":"invalid_request_error","code":"invalid_api_key"}}"#,
        ]);
        
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(response_body)
                    .insert_header("content-type", "text/event-stream")
            )
            .mount(&mock_server)
            .await;
        
        let config = OpenAICompatibleConfig {
            base_url: mock_server.uri(),
            api_key: Some("invalid-key".to_string()),
            model: Some("gpt-4".to_string()),
            timeout_seconds: 30,
            max_retries: 3,
        };
        
        let mcp_integration = create_mock_mcp_integration();
        let client = OpenAICompatibleClient::new(
            config.base_url.clone(),
            config.api_key.clone(),
            config.model.clone(),
            mcp_integration,
            config.timeout_seconds,
            3, // max_retries
        );
        
        let messages = vec![
            Message {
                id: Uuid::new_v4(),
                role: Role::User,
                parts: vec![MessagePart::Text { text: "Hello".to_string() }],
                metadata: HashMap::new(),
            }
        ];
        
        let mut stream = client.generate_stream(&messages, &[]).await.unwrap();
        
        let error = stream.next().await.unwrap().unwrap_err();
        match error {
            crate::providers::SagittaCodeError::LlmError(msg) => {
                assert!(msg.contains("Invalid API key"));
            }
            _ => panic!("Expected LlmError"),
        }
    }
}