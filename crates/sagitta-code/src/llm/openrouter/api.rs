use serde::{Deserialize, Serialize};
use serde_json::Value;

/// OpenRouter API chat completion request
#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderPreferences>,
}

/// Chat message in OpenRouter format (OpenAI-compatible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Tool definition for OpenAI-compatible tool calling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String, // "function"
    pub function: FunctionDefinition,
}

/// Function definition for tool calling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Value>, // JSON Schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>, // Enable strict mode for function calls
}

/// Tool choice specification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    String(String), // "none", "auto", "required"
    Specific {
        #[serde(rename = "type")]
        tool_type: String,
        function: FunctionChoice,
    },
}

/// Specific function choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionChoice {
    pub name: String,
}

/// Tool call in messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String, // "function"
    pub function: FunctionCall,
}

/// Function call details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String, // JSON string
}

/// Provider preferences for routing
#[derive(Debug, Clone, Serialize)]
pub struct ProviderPreferences {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_fallbacks: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_collection: Option<String>,
}

/// OpenRouter API response
#[derive(Debug, Clone, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<Choice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

/// Choice in the response
#[derive(Debug, Clone, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: Option<String>,
}

/// Token usage information
#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Streaming response chunk
#[derive(Debug, Clone, Deserialize)]
pub struct StreamChunk {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<StreamChoice>,
}

/// Choice in streaming response
#[derive(Debug, Clone, Deserialize)]
pub struct StreamChoice {
    pub index: u32,
    pub delta: Delta,
    pub finish_reason: Option<String>,
}

/// Delta content in streaming
#[derive(Debug, Clone, Deserialize)]
pub struct Delta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallDelta>>,
}

/// Tool call delta for streaming
#[derive(Debug, Clone, Deserialize)]
pub struct ToolCallDelta {
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub tool_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<FunctionCallDelta>,
}

/// Function call delta for streaming
#[derive(Debug, Clone, Deserialize)]
pub struct FunctionCallDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

/// Model information from /models endpoint
#[derive(Debug, Clone, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub created: u64,
    pub description: String,
    pub context_length: u64,
    pub architecture: Architecture,
    pub pricing: Pricing,
    pub top_provider: TopProvider,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hugging_face_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_request_limits: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supported_parameters: Option<Vec<String>>,
}

/// Model architecture details
#[derive(Debug, Clone, Deserialize)]
pub struct Architecture {
    pub input_modalities: Vec<String>,
    pub output_modalities: Vec<String>,
    pub tokenizer: String,
}

/// Pricing information
#[derive(Debug, Clone, Deserialize)]
pub struct Pricing {
    pub prompt: String,
    pub completion: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_cache_read: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_cache_write: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_search: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal_reasoning: Option<String>,
}

/// Top provider information
#[derive(Debug, Clone, Deserialize)]
pub struct TopProvider {
    pub is_moderated: bool,
}

/// Models list response
#[derive(Debug, Clone, Deserialize)]
pub struct ModelsResponse {
    pub data: Vec<ModelInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_info_deserialization() {
        // Sample response based on OpenRouter API documentation
        let json_response = r#"
        {
          "data": [
            {
              "id": "anthropic/claude-3.5-sonnet",
              "name": "Claude 3.5 Sonnet",
              "created": 1741818122,
              "description": "Anthropic's most intelligent model",
              "architecture": {
                "input_modalities": [
                  "text",
                  "image"
                ],
                "output_modalities": [
                  "text"
                ],
                "tokenizer": "GPT"
              },
              "top_provider": {
                "is_moderated": true
              },
              "pricing": {
                "prompt": "0.0000007",
                "completion": "0.0000007",
                "image": "0",
                "request": "0",
                "input_cache_read": "0",
                "input_cache_write": "0",
                "web_search": "0",
                "internal_reasoning": "0"
              },
              "context_length": 128000,
              "hugging_face_id": "anthropic/claude-3.5-sonnet",
              "per_request_limits": {
                "key": "value"
              },
              "supported_parameters": [
                "temperature",
                "max_tokens"
              ]
            }
          ]
        }
        "#;

        let result: Result<ModelsResponse, _> = serde_json::from_str(json_response);
        assert!(result.is_ok(), "Failed to deserialize: {:?}", result.err());
        
        let response = result.unwrap();
        assert_eq!(response.data.len(), 1);
        
        let model = &response.data[0];
        assert_eq!(model.id, "anthropic/claude-3.5-sonnet");
        assert_eq!(model.name, "Claude 3.5 Sonnet");
        assert_eq!(model.context_length, 128000);
        assert_eq!(model.pricing.prompt, "0.0000007");
        assert_eq!(model.pricing.completion, "0.0000007");
        assert!(model.pricing.input_cache_read.is_some());
        assert!(model.hugging_face_id.is_some());
        assert!(model.supported_parameters.is_some());
    }

    #[test]
    fn test_model_info_minimal_deserialization() {
        // Test with minimal required fields only
        let json_response = r#"
        {
          "data": [
            {
              "id": "test/model",
              "name": "Test Model",
              "created": 1234567890,
              "description": "A test model",
              "architecture": {
                "input_modalities": ["text"],
                "output_modalities": ["text"],
                "tokenizer": "GPT"
              },
              "top_provider": {
                "is_moderated": false
              },
              "pricing": {
                "prompt": "0.001",
                "completion": "0.002",
                "image": "0",
                "request": "0"
              },
              "context_length": 4096
            }
          ]
        }
        "#;

        let result: Result<ModelsResponse, _> = serde_json::from_str(json_response);
        assert!(result.is_ok(), "Failed to deserialize minimal model: {:?}", result.err());
        
        let response = result.unwrap();
        assert_eq!(response.data.len(), 1);
        
        let model = &response.data[0];
        assert_eq!(model.id, "test/model");
        assert!(model.hugging_face_id.is_none());
        assert!(model.per_request_limits.is_none());
        assert!(model.supported_parameters.is_none());
        assert!(model.pricing.input_cache_read.is_none());
    }
} 