use anyhow::{anyhow, Result};
use futures::stream::StreamExt;
use log::{debug, error, info, warn};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::pin::Pin;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio_stream::Stream;

use crate::config::settings::Settings;
use crate::utils::error::to_app_error;

// Anthropic API model structs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: Vec<AnthropicContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicContent {
    #[serde(rename = "text")]
    Text { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicRequest {
    pub model: String,
    pub messages: Vec<AnthropicMessage>,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    pub stream: bool,
    pub system: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicStreamEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub delta: Option<AnthropicDelta>,
    pub message: Option<AnthropicResponseMessage>,
    pub error: Option<AnthropicError>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicDelta {
    pub text: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicResponseMessage {
    pub id: String,
    pub content: Vec<AnthropicResponseContent>,
    pub role: String,
    pub model: String,
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
    pub usage: Option<AnthropicUsage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicResponseContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicError {
    pub message: String,
    pub type_: String,
}

/// Cache entry for API responses
struct CacheEntry {
    /// Response content
    content: String,
    /// Timestamp when the entry was created
    timestamp: Instant,
}

/// Anthropic API client
pub struct AnthropicClient {
    /// HTTP client
    client: Client,
    /// API key
    api_key: String,
    /// API URL
    api_url: String,
    /// Model to use
    model: String,
    /// Max tokens to generate
    max_tokens: u32,
    /// Response cache
    cache: HashMap<String, CacheEntry>,
    /// Cache TTL in seconds
    cache_ttl: u64,
    /// Last request timestamp
    last_request: Option<Instant>,
    /// Rate limit delay in milliseconds
    rate_limit_delay: u64,
}

impl AnthropicClient {
    /// Create a new Anthropic API client
    pub fn new(settings: &Settings) -> Result<Self> {
        let api_key = settings
            .anthropic_api_key
            .clone()
            .ok_or_else(|| anyhow!("Anthropic API key not found"))?;

        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()?;

        Ok(Self {
            client,
            api_key,
            api_url: settings.anthropic_api_url.clone(),
            model: settings.anthropic_model.clone(),
            max_tokens: settings.anthropic_max_tokens,
            cache: HashMap::new(),
            cache_ttl: 3600, // 1 hour by default
            last_request: None,
            rate_limit_delay: 500, // 500ms between requests
        })
    }

    /// Send a chat completion request (non-streaming)
    pub async fn chat_completion(&mut self, content: &str, system_prompt: Option<&str>, context_files: Vec<(String, String)>) -> Result<String> {
        // Convert content and context files to Anthropic messages format
        let messages = self.create_messages(content, context_files);
        
        // Create the request
        let request = AnthropicRequest {
            model: self.model.clone(),
            messages,
            max_tokens: self.max_tokens,
            temperature: Some(0.7),
            stream: false,
            system: system_prompt.map(|s| s.to_string()),
        };
        
        // Check cache first
        let cache_key = self.generate_cache_key(&request);
        if let Some(entry) = self.cache.get(&cache_key) {
            if entry.timestamp.elapsed().as_secs() < self.cache_ttl {
                debug!("Cache hit for request");
                return Ok(entry.content.clone());
            } else {
                debug!("Cache expired for request");
            }
        }
        
        // Apply rate limiting
        self.apply_rate_limiting().await;
        
        let url = format!("{}/v1/messages", self.api_url);
        
        debug!("Sending request to {}", url);
        
        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| to_app_error(e, "Failed to send API request"))?;

        let status = response.status();
        self.last_request = Some(Instant::now());
        
        if status == StatusCode::TOO_MANY_REQUESTS {
            warn!("Rate limit exceeded, increasing delay");
            self.rate_limit_delay *= 2;
            return Err(anyhow!("API rate limit exceeded. Please try again later."));
        } else if !status.is_success() {
            let error_text = response.text().await?;
            error!("API error: {} - {}", status, error_text);
            return Err(anyhow!("API error: {} - {}", status, error_text));
        }

        let response_text = response.text().await?;
        let response: AnthropicResponseMessage = serde_json::from_str(&response_text)
            .map_err(|e| anyhow!("Failed to parse API response: {}", e))?;
        
        // Extract the text from the response
        let content = response.content.iter()
            .filter_map(|c| c.text.clone())
            .collect::<Vec<String>>()
            .join("");
        
        // Cache the response
        self.cache.insert(
            cache_key,
            CacheEntry {
                content: content.clone(),
                timestamp: Instant::now(),
            },
        );
        
        // If we got here, rate limiting is working fine, so we can reset the delay
        // if it was increased
        if self.rate_limit_delay > 500 {
            self.rate_limit_delay = 500;
        }
        
        Ok(content)
    }

    /// Send a streaming chat completion request
    pub async fn chat_completion_stream(
        &mut self,
        content: &str,
        system_prompt: Option<&str>,
        context_files: Vec<(String, String)>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        // Convert content and context files to Anthropic messages format
        let messages = self.create_messages(content, context_files);
        
        // Create the request
        let request = AnthropicRequest {
            model: self.model.clone(),
            messages,
            max_tokens: self.max_tokens,
            temperature: Some(0.7),
            stream: true,
            system: system_prompt.map(|s| s.to_string()),
        };
        
        // Apply rate limiting
        self.apply_rate_limiting().await;
        
        let url = format!("{}/v1/messages", self.api_url);
        
        debug!("Sending streaming request to {}", url);
        
        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| to_app_error(e, "Failed to send API request"))?;

        let status = response.status();
        self.last_request = Some(Instant::now());
        
        if status == StatusCode::TOO_MANY_REQUESTS {
            warn!("Rate limit exceeded, increasing delay");
            self.rate_limit_delay *= 2;
            return Err(anyhow!("API rate limit exceeded. Please try again later."));
        } else if !status.is_success() {
            let error_text = response.text().await?;
            error!("API error: {} - {}", status, error_text);
            return Err(anyhow!("API error: {} - {}", status, error_text));
        }

        // Create a channel for the stream
        let (tx, rx) = mpsc::channel(100);
        
        // Process the stream in a separate task
        let stream = response.bytes_stream();
        
        tokio::spawn(async move {
            let mut stream = stream;
            
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        let chunk_str = String::from_utf8_lossy(&chunk);
                        
                        // Parse SSE events (each line starting with "data: ")
                        for line in chunk_str.lines() {
                            if line.starts_with("data: ") {
                                let data = &line["data: ".len()..];
                                
                                // Skip [DONE] line
                                if data == "[DONE]" {
                                    continue;
                                }
                                
                                // Parse the event
                                match serde_json::from_str::<AnthropicStreamEvent>(data) {
                                    Ok(event) => {
                                        // Process the event
                                        if let Some(delta) = event.delta {
                                            if let Some(text) = delta.text {
                                                if !text.is_empty() {
                                                    if tx.send(Ok(text)).await.is_err() {
                                                        // Receiver dropped, stop processing
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                        
                                        // Check for errors
                                        if let Some(error) = event.error {
                                            if tx.send(Err(anyhow!("Stream error: {}", error.message))).await.is_err() {
                                                break;
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        // Log parsing error but don't stop processing
                                        error!("Failed to parse stream event: {} - {}", e, data);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        // Send the error to the channel
                        if tx.send(Err(anyhow!("Stream error: {}", e))).await.is_err() {
                            break;
                        }
                    }
                }
            }
        });
        
        // Convert the receiver to a stream
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        
        Ok(Box::pin(stream))
    }

    /// Create messages from content and context files
    fn create_messages(&self, content: &str, context_files: Vec<(String, String)>) -> Vec<AnthropicMessage> {
        let mut user_content = content.to_string();
        
        // Add context files to the message if there are any
        if !context_files.is_empty() {
            user_content = format!(
                "{}\n\nIMPORTANT: I've provided you with file contents that are directly relevant to this question. \
                Please use this context to provide a comprehensive answer. The files are marked with '# FILE:' headers. \
                If the files contain the answer, please reference and explain the specific code or content in your response.\n\n",
                content
            );

            // Add each file to the message
            for (path, content) in &context_files {
                user_content.push_str(&format!("# FILE: {}\n{}\n\n", path, content));
            }
        }
        
        // Create the user message
        let user_message = AnthropicMessage {
            role: "user".to_string(),
            content: vec![AnthropicContent::Text {
                text: user_content,
            }],
        };
        
        vec![user_message]
    }
    
    /// Create messages from conversation history, current content and context files
    fn create_messages_with_history(
        &self, 
        content: &str, 
        history: Vec<(String, String)>, 
        context_files: Vec<(String, String)>
    ) -> Vec<AnthropicMessage> {
        let mut messages = Vec::new();
        
        // Debug the incoming conversation history
        debug!("Received conversation history with {} messages", history.len());
        for (i, (role, message)) in history.iter().enumerate() {
            debug!("History message {}: role={}, content={}", i, role, message.len());
        }
        
        // Add each message from history
        for (i, (role, message_content)) in history.iter().enumerate() {
            // Skip the last user message if it's identical to the current input
            if i == history.len() - 1 && role == "user" && message_content.trim() == content.trim() {
                debug!("Skipping duplicate last user message: {}", content);
                continue;
            }
            
            // Create the message with appropriate role
            let message = AnthropicMessage {
                role: role.clone(),
                content: vec![AnthropicContent::Text {
                    text: message_content.clone(),
                }],
            };
            
            messages.push(message);
        }
        
        // Create the current user message with context files
        let mut current_content = content.to_string();
        
        // Add context files to the message if there are any
        if !context_files.is_empty() {
            current_content = format!(
                "{}\n\nIMPORTANT: I've provided you with file contents that are directly relevant to this question. \
                Please use this context to provide a comprehensive answer. The files are marked with '# FILE:' headers. \
                If the files contain the answer, please reference and explain the specific code or content in your response.\n\n",
                content
            );

            // Add each file to the message
            for (path, content) in &context_files {
                current_content.push_str(&format!("# FILE: {}\n{}\n\n", path, content));
            }
        }
        
        // Add the current user message
        let current_message = AnthropicMessage {
            role: "user".to_string(),
            content: vec![AnthropicContent::Text {
                text: current_content,
            }],
        };
        
        messages.push(current_message);
        
        debug!("Created {} messages for the API request", messages.len());
        
        messages
    }
    
    /// Send a streaming chat completion request with conversation history
    pub async fn chat_completion_stream_with_history(
        &mut self,
        content: &str,
        system_prompt: Option<&str>,
        history: Vec<(String, String)>,
        context_files: Vec<(String, String)>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        // Convert content, history, and context files to Anthropic messages format
        let messages = self.create_messages_with_history(content, history, context_files);
        
        debug!("Using messages: {:?}", messages);
        
        // Create the request
        let request = AnthropicRequest {
            model: self.model.clone(),
            messages,
            max_tokens: self.max_tokens,
            temperature: Some(0.7),
            stream: true,
            system: system_prompt.map(|s| s.to_string()),
        };
        
        // Apply rate limiting
        self.apply_rate_limiting().await;
        
        let url = format!("{}/v1/messages", self.api_url);
        
        debug!("Sending streaming request with history to {}", url);
        debug!("Request body: {}", serde_json::to_string(&request).unwrap_or_default());
        
        let request_body = serde_json::to_string(&request)?;
        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .body(request_body)
            .send()
            .await
            .map_err(|e| to_app_error(e, "Failed to send API request"))?;

        let status = response.status();
        self.last_request = Some(Instant::now());
        
        if status == StatusCode::TOO_MANY_REQUESTS {
            warn!("Rate limit exceeded, increasing delay");
            self.rate_limit_delay *= 2;
            return Err(anyhow!("API rate limit exceeded. Please try again later."));
        } else if !status.is_success() {
            let error_text = response.text().await?;
            error!("API error: {} - {}", status, error_text);
            return Err(anyhow!("API error: {} - {}", status, error_text));
        }
        
        let stream = response.bytes_stream();

        // Create a channel for sending chunks of text
        let (tx, rx) = mpsc::channel(100);
        
        // Spawn a task to process the stream
        tokio::spawn(async move {
            let mut buffer = String::new();
            let mut line_buffer = String::new();
            
            let mut stream = stream;
            
            while let Some(result) = stream.next().await {
                match result {
                    Ok(chunk) => {
                        let chunk_str = String::from_utf8_lossy(&chunk);
                        buffer.push_str(&chunk_str);
                        
                        // Process the buffer line by line
                        while let Some(pos) = buffer.find('\n') {
                            let line = buffer.drain(..=pos).collect::<String>();
                            
                            // Check if it's a data line
                            if line.starts_with("data:") {
                                let data = line.trim_start_matches("data:").trim();
                                
                                // Skip empty data lines
                                if data == "[DONE]" {
                                    continue;
                                }
                                
                                if !data.is_empty() {
                                    // Try to parse it as a stream event
                                    if let Ok(event) = serde_json::from_str::<AnthropicStreamEvent>(data) {
                                        match event.event_type.as_str() {
                                            "content_block_delta" => {
                                                if let Some(delta) = event.delta {
                                                    if let Some(text) = delta.text {
                                                        // Add to the line buffer
                                                        line_buffer.push_str(&text);
                                                        
                                                        // Send the update
                                                        if tx.send(Ok(text)).await.is_err() {
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                            "message_start" => {
                                                // Check for usage information
                                                if let Some(message) = &event.message {
                                                    if let Some(usage) = &message.usage {
                                                        // Send a special event with usage statistics
                                                        // Use a special prefix that can be easily detected
                                                        let usage_event = json!({
                                                            "type": "usage_info",
                                                            "input_tokens": usage.input_tokens,
                                                            "output_tokens": usage.output_tokens
                                                        });
                                                        
                                                        // Make this a special message type with "$$INTERNAL$$:" prefix
                                                        // This will be removed before displaying to the user
                                                        let internal_message = format!("$$INTERNAL$$:{}", usage_event.to_string());
                                                        
                                                        if tx.send(Ok(internal_message)).await.is_err() {
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                            "message_stop" => {
                                                // Message complete
                                                debug!("Message complete");
                                            }
                                            "error" => {
                                                if let Some(error) = event.error {
                                                    error!("Stream error: {}", error.message);
                                                    // Send the error to the channel
                                                    if tx.send(Err(anyhow!("Stream error: {}", error.message))).await.is_err() {
                                                        break;
                                                    }
                                                }
                                            }
                                            _ => {
                                                debug!("Unhandled event type: {}", event.event_type);
                                            }
                                        }
                                    } else {
                                        debug!("Failed to parse stream event: {}", data);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Stream error: {}", e);
                        
                        // Send the error to the channel
                        if tx.send(Err(anyhow!("Stream error: {}", e))).await.is_err() {
                            break;
                        }
                    }
                }
            }
        });
        
        // Convert the receiver to a stream
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        
        Ok(Box::pin(stream))
    }

    /// Generate a cache key for a request
    fn generate_cache_key(&self, request: &AnthropicRequest) -> String {
        // Simple cache key based on the request JSON
        let request_json = serde_json::to_string(request).unwrap_or_default();
        format!("request:{}", request_json)
    }
    
    /// Apply rate limiting
    async fn apply_rate_limiting(&self) {
        if let Some(last_request) = self.last_request {
            let elapsed = last_request.elapsed();
            let delay = Duration::from_millis(self.rate_limit_delay);
            
            if elapsed < delay {
                let wait_time = delay - elapsed;
                debug!("Rate limiting: waiting for {}ms", wait_time.as_millis());
                tokio::time::sleep(wait_time).await;
            }
        }
    }
    
    /// Clear the cache
    pub fn clear_cache(&mut self) {
        self.cache.clear();
        info!("API response cache cleared");
    }
    
    /// Set the cache TTL in seconds
    pub fn set_cache_ttl(&mut self, ttl: u64) {
        self.cache_ttl = ttl;
        info!("API response cache TTL set to {}s", ttl);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::Settings;
    use std::time::Instant;

    // Setup test settings
    pub fn create_test_settings() -> Settings {
        Settings {
            anthropic_api_key: Some("test_api_key".to_string()),
            anthropic_api_url: "https://api.anthropic.com".to_string(),
            anthropic_model: "claude-3-opus-20240229".to_string(),
            anthropic_max_tokens: 4000,
            max_context_size: 10000,
            yolo_mode: false,
            pending_shell_command: None,
            disable_command_execution: false,
            test_mode: true,
            show_auto_prompts: false,
        }
    }

    // Get a test client for unit tests
    fn get_test_client() -> Result<AnthropicClient> {
        let settings = create_test_settings();
        AnthropicClient::new(&settings)
    }

    #[tokio::test]
    async fn test_new_client() {
        // Test creation with valid settings
        let settings = create_test_settings();
        let client = AnthropicClient::new(&settings);
        assert!(client.is_ok());
        
        // Test API key validation
        let mut invalid_settings = settings.clone();
        invalid_settings.anthropic_api_key = None;
        let client = AnthropicClient::new(&invalid_settings);
        assert!(client.is_err());
    }
    
    #[tokio::test]
    async fn test_generate_cache_key() {
        let client = get_test_client().unwrap();
        
        // Create a test request
        let request = AnthropicRequest {
            model: "claude-3-opus-20240229".to_string(),
            messages: vec![
                AnthropicMessage {
                    role: "user".to_string(),
                    content: vec![
                        AnthropicContent::Text {
                            text: "Hello".to_string()
                        }
                    ]
                }
            ],
            max_tokens: 4000,
            temperature: Some(0.7),
            stream: false,
            system: Some("You are a helpful assistant".to_string()),
        };
        
        // Generate cache key
        let key = client.generate_cache_key(&request);
        
        // Key should be a non-empty string
        assert!(!key.is_empty());
        
        // Different requests should have different keys
        let mut request2 = request.clone();
        request2.messages[0].content[0] = AnthropicContent::Text {
            text: "Different message".to_string()
        };
        
        let key2 = client.generate_cache_key(&request2);
        assert_ne!(key, key2);
    }
    
    #[tokio::test]
    async fn test_create_messages() {
        let client = get_test_client().unwrap();
        
        // Test with empty context
        let messages = client.create_messages("Hello", vec![]);
        
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
        
        // Test with context files
        let context_files = vec![
            ("file1.rs".to_string(), "fn hello() {}".to_string()),
            ("file2.rs".to_string(), "fn world() {}".to_string()),
        ];
        
        let messages = client.create_messages("Show me these files", context_files);
        
        // Should have one user message
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
        
        // Message should include context files
        // Using let instead of if let since the pattern is irrefutable
        let AnthropicContent::Text { text } = &messages[0].content[0];
        assert!(text.contains("file1.rs"));
        assert!(text.contains("fn hello() {}"));
        assert!(text.contains("file2.rs"));
        assert!(text.contains("fn world() {}"));
    }

    #[tokio::test]
    async fn test_create_messages_with_history() {
        let client = get_test_client().unwrap();
        
        // Create history
        let history = vec![
            ("user".to_string(), "Hello".to_string()),
            ("assistant".to_string(), "Hi there!".to_string()),
        ];
        
        // Create messages with history and empty context
        let messages = client.create_messages_with_history(
            "How are you?", 
            history, 
            vec![]
        );
        
        // Should have 3 messages: 2 from history + 1 new
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[2].role, "user");
        
        // Check content of messages
        // Using let instead of if let since the patterns are irrefutable
        let AnthropicContent::Text { text } = &messages[0].content[0];
        assert_eq!(text, "Hello");
        
        let AnthropicContent::Text { text } = &messages[1].content[0];
        assert_eq!(text, "Hi there!");
        
        let AnthropicContent::Text { text } = &messages[2].content[0];
        assert_eq!(text, "How are you?");
    }
    
    #[tokio::test]
    async fn test_set_cache_ttl() {
        let mut client = get_test_client().unwrap();
        
        // Default TTL
        let initial_ttl = client.cache_ttl;
        
        // Change TTL
        client.set_cache_ttl(60);
        
        // Check new TTL
        assert_eq!(client.cache_ttl, 60);
        assert_ne!(client.cache_ttl, initial_ttl);
    }
    
    #[tokio::test]
    async fn test_clear_cache() {
        let mut client = get_test_client().unwrap();
        
        // Add something to cache
        let key = "test_key".to_string();
        let content = "test_content".to_string();
        client.cache.insert(key.clone(), CacheEntry {
            content: content.clone(),
            timestamp: Instant::now(),
        });
        
        // Verify it's in cache
        assert_eq!(client.cache.len(), 1);
        
        // Clear cache
        client.clear_cache();
        
        // Cache should be empty
        assert_eq!(client.cache.len(), 0);
    }
} 