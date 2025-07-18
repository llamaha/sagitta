#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::llm::client::{LlmClient, MessagePart, StreamChunk};
    use crate::providers::openai_compatible::{OpenAICompatibleClient, OpenAICompatibleConfig};
    use crate::agent::message::{Message, MessageRole};
    use futures::StreamExt;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path, header};
    
    async fn create_mock_server() -> MockServer {
        MockServer::start().await
    }
    
    fn create_sse_response(events: Vec<&str>) -> String {
        events.iter()
            .map(|event| format!("data: {}\n\n", event))
            .collect::<Vec<_>>()
            .join("")
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
            name: "test".to_string(),
            base_url: mock_server.uri(),
            api_key: "test-key".to_string(),
            model: "gpt-4".to_string(),
            max_tokens: Some(1000),
            timeout_seconds: 30,
        };
        
        let client = OpenAICompatibleClient::new(config);
        
        let messages = vec![
            Message {
                role: MessageRole::User,
                content: vec![MessagePart::Text { text: "Say hello".to_string() }],
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
            name: "test".to_string(),
            base_url: mock_server.uri(),
            api_key: "test-key".to_string(),
            model: "gpt-4".to_string(),
            max_tokens: Some(1000),
            timeout_seconds: 30,
        };
        
        let client = OpenAICompatibleClient::new(config);
        
        let messages = vec![
            Message {
                role: MessageRole::User,
                content: vec![MessagePart::Text { text: "What is the answer?".to_string() }],
            }
        ];
        
        let mut stream = client.generate_stream(&messages, &[]).await.unwrap();
        
        let mut parts = Vec::new();
        
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.unwrap();
            parts.push(chunk.part);
        }
        
        // Verify we got the right sequence
        assert_eq!(parts.len(), 3);
        
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
            name: "test".to_string(),
            base_url: mock_server.uri(),
            api_key: "test-key".to_string(),
            model: "gpt-4".to_string(),
            max_tokens: Some(1000),
            timeout_seconds: 30,
        };
        
        let client = OpenAICompatibleClient::new(config);
        
        let messages = vec![
            Message {
                role: MessageRole::User,
                content: vec![MessagePart::Text { text: "What's the weather?".to_string() }],
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
        
        let params: serde_json::Value = serde_json::from_str(&tool_calls[0].2).unwrap();
        assert_eq!(params["location"], "San Francisco");
        assert_eq!(params["unit"], "celsius");
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
            name: "test".to_string(),
            base_url: mock_server.uri(),
            api_key: "invalid-key".to_string(),
            model: "gpt-4".to_string(),
            max_tokens: Some(1000),
            timeout_seconds: 30,
        };
        
        let client = OpenAICompatibleClient::new(config);
        
        let messages = vec![
            Message {
                role: MessageRole::User,
                content: vec![MessagePart::Text { text: "Hello".to_string() }],
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