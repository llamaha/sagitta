use crate::config::RelayConfig;
use crate::llm::message::*;
use crate::utils::error::{RelayError, Result};
use futures::stream::StreamExt;
use reqwest::{Client, StatusCode};
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex};
use tokio_stream::Stream;
use tracing::{debug, error, info, warn};

const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic API client
#[derive(Debug)]
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
    /// Rate limit delay state (Arc<Mutex> to allow modification)
    rate_limit_delay_ms: Arc<Mutex<u64>>,
    /// Last request timestamp (Arc<Mutex> to allow modification)
    last_request: Arc<Mutex<Option<Instant>>>,
    /// Static initial delay value from config
    initial_rate_limit_delay_ms: u64,
}

impl AnthropicClient {
    /// Create a new Anthropic API client
    pub fn new(config: &RelayConfig) -> Result<Self> {
        let api_key = config
            .anthropic_api_key
            .clone()
            .ok_or_else(|| RelayError::ConfigError("Anthropic API key not found in Relay config".to_string()))?;

        let client = Client::builder()
            .timeout(Duration::from_secs(config.request_timeout_secs))
            .build()
            .map_err(|e| RelayError::ConfigError(format!("Failed to build HTTP client: {}", e)))?;

        Ok(Self {
            client,
            api_key,
            api_url: config.anthropic_api_url.clone(),
            model: config.anthropic_model.clone(),
            max_tokens: config.anthropic_max_tokens,
            rate_limit_delay_ms: Arc::new(Mutex::new(config.rate_limit_delay_ms)),
            last_request: Arc::new(Mutex::new(None)),
            initial_rate_limit_delay_ms: config.rate_limit_delay_ms,
        })
    }

    /// Send a streaming chat completion request
    pub async fn chat_completion_stream(
        &self,
        messages: &[AnthropicMessage],
        system_prompt: Option<&str>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        let request_payload = AnthropicRequest {
            model: self.model.clone(),
            messages,
            max_tokens: self.max_tokens,
            temperature: Some(0.7), // TODO: Make configurable?
            stream: true,
            system: system_prompt,
        };

        self.apply_rate_limiting().await;

        let url = format!("{}/v1/messages", self.api_url);
        debug!(%url, ?request_payload, "Sending streaming request to Anthropic");

        let request_body = serde_json::to_string(&request_payload)
            .map_err(|e| RelayError::LlmApiError(format!("Failed to serialize request: {}", e)))?;

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("Content-Type", "application/json")
            .body(request_body)
            .send()
            .await
            .map_err(|e| RelayError::LlmApiError(format!("Failed to send API request: {}", e)))?;

        let status = response.status();
        *self.last_request.lock().await = Some(Instant::now());

        if status == StatusCode::TOO_MANY_REQUESTS {
            warn!("Rate limit exceeded, increasing delay");
            let mut delay = self.rate_limit_delay_ms.lock().await;
            *delay *= 2; // Exponential backoff
            return Err(RelayError::LlmApiError(
                "API rate limit exceeded. Please try again later.".to_string(),
            ));
        } else if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            error!(%status, %error_text, "Anthropic API error");
            return Err(RelayError::LlmApiError(format!(
                "API error: {} - {}",
                status,
                error_text
            )));
        }

        // Reset delay if request was successful
        let mut delay = self.rate_limit_delay_ms.lock().await;
        if *delay > self.initial_rate_limit_delay_ms {
            info!("Resetting rate limit delay to default");
            *delay = self.initial_rate_limit_delay_ms;
        }
        drop(delay);

        // Create a channel for the stream
        let (tx, rx) = mpsc::channel(100);

        // Process the stream in a separate task
        let mut byte_stream = response.bytes_stream();

        tokio::spawn(async move {
            let mut buffer = String::new();
            while let Some(chunk_result) = byte_stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        // Append chunk to buffer
                        buffer.push_str(&String::from_utf8_lossy(&chunk));
                        
                        // Process buffer line by line for SSE events
                        while let Some(newline_pos) = buffer.find('\n') {
                            let line = buffer.drain(..=newline_pos).collect::<String>();
                            // Trim whitespace, especially the trailing newline
                            let trimmed_line = line.trim();

                            if trimmed_line.starts_with("data: ") {
                                let data = &trimmed_line["data: ".len()..];

                                if data == "[DONE]" {
                                    debug!("Received [DONE] marker");
                                    continue;
                                }

                                match serde_json::from_str::<AnthropicStreamEvent>(data) {
                                    Ok(event) => {
                                        // Handle different event types
                                        match event.event_type.as_str() {
                                            "message_start" => {
                                                debug!(?event.message, "Stream started");
                                            }
                                            "content_block_delta" => {
                                                if let Some(delta) = event.delta {
                                                    if let Some(text) = delta.text {
                                                        if tx.send(Ok(text)).await.is_err() {
                                                            warn!("Receiver dropped, stopping stream processing");
                                                            return; // Stop task
                                                        }
                                                    }
                                                }
                                            }
                                            "message_delta" => {
                                                 // Could contain usage updates
                                                if let Some(usage) = event.usage {
                                                    debug!(?usage, "Received usage update in delta");
                                                }
                                            }
                                            "message_stop" => {
                                                debug!("Stream finished");
                                                // Implicitly handled by closing the stream
                                                // Receiver will get None when channel closes
                                                return; // Stop task
                                            }
                                             "ping" => {
                                                // Ignore ping events
                                                debug!("Received ping event");
                                            }
                                            "error" => {
                                                if let Some(error_data) = event.error {
                                                    error!(?error_data, "Received error event in stream");
                                                    let err_msg = format!(
                                                        "Stream error [{}]: {}",
                                                        error_data.error_type,
                                                        error_data.message
                                                    );
                                                    if tx.send(Err(RelayError::LlmApiError(err_msg))).await.is_err() {
                                                        warn!("Receiver dropped after error, stopping stream processing");
                                                        return; // Stop task
                                                    }
                                                } else {
                                                    error!("Received error event without details");
                                                     if tx.send(Err(RelayError::LlmApiError("Unknown stream error".to_string()))).await.is_err() {
                                                        warn!("Receiver dropped after unknown error, stopping stream processing");
                                                        return; // Stop task
                                                    }
                                                }
                                            }
                                            _ => {
                                                warn!(event_type = %event.event_type, "Received unknown stream event type");
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error!(error = %e, data = %data, "Failed to parse stream event");
                                        // Decide if we should send an error or just log
                                        // Sending error might be better to signal issue to consumer
                                         let err_msg = format!("Failed to parse stream event: {}", e);
                                        if tx.send(Err(RelayError::LlmApiError(err_msg))).await.is_err() {
                                             warn!("Receiver dropped after parsing error, stopping stream processing");
                                            return; // Stop task
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "Stream chunk error");
                        let err_msg = format!("Stream chunk error: {}", e);
                        if tx.send(Err(RelayError::LlmApiError(err_msg))).await.is_err() {
                            warn!("Receiver dropped after chunk error, stopping stream processing");
                            return; // Stop task
                        }
                    }
                }
            }
            // If loop finishes without message_stop, stream ended unexpectedly
            warn!("Byte stream ended without a 'message_stop' event.");
        });

        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Box::pin(stream))
    }

    /// Apply rate limiting logic before sending a request.
    async fn apply_rate_limiting(&self) {
        let last_req_opt = self.last_request.lock().await;
        if let Some(last_req) = *last_req_opt {
            let elapsed = last_req.elapsed();
            let delay_ms = *self.rate_limit_delay_ms.lock().await;
            let required_delay = Duration::from_millis(delay_ms);

            if elapsed < required_delay {
                let sleep_duration = required_delay - elapsed;
                debug!(?sleep_duration, "Applying rate limit delay");
                tokio::time::sleep(sleep_duration).await;
            }
        }
        // Update last request time *after* sending the request
        // *last_req_opt = Some(Instant::now()); // Moved to after send()
    }
} 