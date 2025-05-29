// Gemini API request/response types will go here

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use log::{trace};

use crate::llm::client::{Role, MessagePart};

/// The content of a Gemini request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Content {
    /// The role of the message sender
    pub role: String,
    
    /// The parts that make up this content (optional for completion markers)
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<Part>,
}

/// A part in a Gemini message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Part {
    /// The text of the part, if it's a text part
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    
    /// Function call information if it's a function_call part
    #[serde(rename = "functionCall")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<FunctionCall>,
    
    /// Function response information if it's a function_response part
    #[serde(rename = "functionResponse")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_response: Option<FunctionResponse>,
    
    /// Whether this part contains thinking content (thought summary)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought: Option<bool>,
}

/// A function call from Gemini
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    /// The name of the function to call
    pub name: String,
    
    /// The parameters to pass to the function
    pub args: Value,
}

/// A function response to Gemini
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionResponse {
    /// The name of the function that was called
    pub name: String,
    
    /// The response from the function
    pub response: Value,
}

/// A function/tool definition for Gemini
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDeclaration {
    /// The name of the function
    pub name: String,
    
    /// The description of the function
    pub description: String,
    
    /// The parameters of the function
    pub parameters: Value,
}

/// Safety settings for Gemini
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetySetting {
    /// The category of safety setting
    pub category: String,
    
    /// The threshold for this safety setting
    pub threshold: String,
}

/// Generation configuration for Gemini
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConfig {
    /// Temperature for sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    
    /// Top-p for sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    
    /// Top-k for sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,
    
    /// Maximum number of output tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i32>,
    
    /// List of stop sequences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    
    /// Thinking configuration for models that support it
    #[serde(rename = "thinkingConfig")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_config: Option<ThinkingConfig>,
}

/// Configuration for thinking mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingConfig {
    /// Whether to include thought summaries in the response
    #[serde(rename = "includeThoughts")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_thoughts: Option<bool>,
    
    /// Budget for thinking tokens (0-24576, only supported by Flash 2.5)
    #[serde(rename = "thinkingBudget")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_budget: Option<i32>,
}

/// A request to the Gemini API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiRequest {
    /// The contents of the request
    pub contents: Vec<Content>,
    
    /// The safety settings for the request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_settings: Option<Vec<SafetySetting>>,
    
    /// The generation configuration for the request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GenerationConfig>,
    
    /// The tools that the model can use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
}

/// A tool for Gemini
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    /// The function declarations for this tool
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_declarations: Option<Vec<FunctionDeclaration>>,
    
    /// Google Search grounding tool  
    #[serde(rename = "googleSearch")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub google_search: Option<GoogleSearch>,
}

/// Google Search grounding configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleSearch {
    /// Dynamic retrieval configuration
    #[serde(rename = "dynamicRetrieval")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_retrieval: Option<DynamicRetrieval>,
}

/// Dynamic retrieval configuration for Google Search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicRetrieval {
    /// Prediction threshold for determining when to use grounding
    /// Range: [0,1], default: 0.3
    /// 0 = always ground, 1 = never ground
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f32>,
}

/// Grounding metadata from the response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundingMetadata {
    /// Search entry point with suggestions
    #[serde(rename = "searchEntryPoint")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_entry_point: Option<SearchEntryPoint>,
    
    /// Grounding chunks used in the response
    #[serde(rename = "groundingChunks")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grounding_chunks: Option<Vec<GroundingChunk>>,
    
    /// Grounding supports for segments of the response
    #[serde(rename = "groundingSupports")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grounding_supports: Option<Vec<GroundingSupport>>,
    
    /// Web search queries that were performed
    #[serde(rename = "webSearchQueries")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_search_queries: Option<Vec<String>>,
}

/// Search entry point with Google Search suggestions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchEntryPoint {
    /// Rendered content for search suggestions (HTML)
    #[serde(rename = "renderedContent")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rendered_content: Option<String>,
}

/// A grounding chunk from a web source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundingChunk {
    /// Web source information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web: Option<WebSource>,
}

/// Web source information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSource {
    /// URI of the web source
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    
    /// Title of the web source
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Grounding support for a segment of text
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundingSupport {
    /// The text segment that is supported
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segment: Option<GroundingSegment>,
    
    /// Indices of grounding chunks that support this segment
    #[serde(rename = "groundingChunkIndices")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grounding_chunk_indices: Option<Vec<i32>>,
    
    /// Confidence scores for the grounding
    #[serde(rename = "confidenceScores")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence_scores: Option<Vec<f32>>,
}

/// A segment of text that has grounding support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundingSegment {
    /// Start index in the response text
    #[serde(rename = "startIndex")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_index: Option<i32>,
    
    /// End index in the response text  
    #[serde(rename = "endIndex")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_index: Option<i32>,
    
    /// The text content of the segment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// A candidate from the Gemini API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    /// The content of the candidate
    pub content: Content,
    
    /// The safety ratings for this candidate
    #[serde(default)]
    pub safety_ratings: Vec<SafetyRating>,
    
    /// The finish reason for this candidate
    #[serde(rename = "finishReason")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    
    /// Grounding metadata for this candidate
    #[serde(rename = "groundingMetadata")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grounding_metadata: Option<GroundingMetadata>,
}

/// A safety rating from the Gemini API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyRating {
    /// The category of the safety rating
    pub category: String,
    
    /// The probability of the safety rating
    pub probability: String,
}

/// A response from the Gemini API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiResponse {
    /// The candidates from the response
    pub candidates: Vec<Candidate>,
    
    /// The prompt feedback from the API
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_feedback: Option<PromptFeedback>,
    
    /// Usage information from the API
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_metadata: Option<UsageMetadata>,
}

/// Prompt feedback from the Gemini API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptFeedback {
    /// The safety ratings for the prompt
    pub safety_ratings: Vec<SafetyRating>,
    
    /// The block reason if the prompt was blocked
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_reason: Option<String>,
}

/// Usage metadata from the Gemini API
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_token_count: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidates_token_count: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_token_count: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thoughts_token_count: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_content_token_count: Option<i32>,
}

/// Error response from the Gemini API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiError {
    /// The error information
    pub error: ErrorInfo,
}

/// Error information from the Gemini API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    /// The error code
    pub code: i32,
    
    /// The error message
    pub message: String,
    
    /// The error status
    pub status: String,
    
    /// Additional error details
    #[serde(default)]
    pub details: Vec<ErrorDetail>,
}

/// Additional error details from the Gemini API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetail {
    /// The type of error detail
    #[serde(rename = "@type")]
    pub type_url: String,
    
    /// Additional fields in the error detail
    #[serde(flatten)]
    pub additional_fields: HashMap<String, Value>,
}

/// Helper functions to convert between Gemini and client types
impl From<&crate::llm::client::Message> for Content {
    fn from(message: &crate::llm::client::Message) -> Self {
        let role = match message.role {
            Role::User => "user",
            Role::Assistant => "model",
            Role::System => "user",
            Role::Function => "function",
        }.to_string();
        
        trace!("Gemini API: Converting llm::client::Message (ID: {}, Role: {:?}) to Gemini Content (Role: {}).", message.id, message.role, role);
        
        let parts = message.parts.iter()
            .map(|part| match part {
                MessagePart::Text { text } => {
                    trace!("Gemini API: Adding TextPart: {}", text);
                    Part {
                        text: Some(text.clone()),
                        function_call: None,
                        function_response: None,
                        thought: None,
                    }
                },
                MessagePart::Thought { text } => {
                    trace!("Gemini API: Adding ThoughtPart: {}", text);
                    Part {
                        text: Some(text.clone()),
                        function_call: None,
                        function_response: None,
                        thought: Some(true),
                    }
                },
                MessagePart::ToolCall { name, parameters, tool_call_id } => {
                    trace!("Gemini API: Adding ToolCall (FunctionCall) part for LLM. ID: {}, Name: {}, Params: {:?}", tool_call_id, name, parameters);
                    Part {
                        text: None,
                        function_call: Some(FunctionCall {
                            name: name.clone(),
                            args: parameters.clone(),
                        }),
                        function_response: None,
                        thought: None,
                    }
                },
                MessagePart::ToolResult { tool_call_id, name, result } => {
                    trace!("Gemini API: Converting ToolResult to FunctionResponse. ToolCallID: {}, Name: {}, Response: {:?}", tool_call_id, name, result);
                    Part {
                        text: None,
                        function_call: None,
                        function_response: Some(FunctionResponse {
                            name: name.clone(),
                            response: result.clone(),
                        }),
                        thought: None,
                    }
                },
            })
            .collect();
        
        Content { role, parts }
    }
}

impl From<&crate::llm::client::ToolDefinition> for FunctionDeclaration {
    fn from(tool: &crate::llm::client::ToolDefinition) -> Self {
        trace!("Gemini API: Converting ToolDefinition '{}' to FunctionDeclaration.", tool.name);
        FunctionDeclaration {
            name: tool.name.clone(),
            description: tool.description.clone(),
            parameters: tool.parameters.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::client::{Message, MessagePart, Role};
    use serde_json::json;

    #[test]
    fn test_part_serialization() {
        // Test text part
        let text_part = Part {
            text: Some("Hello, world!".to_string()),
            function_call: None,
            function_response: None,
            thought: None,
        };
        
        let serialized = serde_json::to_string(&text_part).unwrap();
        let deserialized: Part = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(text_part.text, deserialized.text);
        assert!(deserialized.function_call.is_none());
        assert!(deserialized.function_response.is_none());
    }

    #[test]
    fn test_function_call_serialization() {
        let function_call = FunctionCall {
            name: "test_function".to_string(),
            args: json!({"param1": "value1", "param2": 42}),
        };
        
        let serialized = serde_json::to_string(&function_call).unwrap();
        let deserialized: FunctionCall = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(function_call.name, deserialized.name);
        assert_eq!(function_call.args, deserialized.args);
    }

    #[test]
    fn test_function_response_serialization() {
        let function_response = FunctionResponse {
            name: "test_function".to_string(),
            response: json!({"result": "success", "data": [1, 2, 3]}),
        };
        
        let serialized = serde_json::to_string(&function_response).unwrap();
        let deserialized: FunctionResponse = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(function_response.name, deserialized.name);
        assert_eq!(function_response.response, deserialized.response);
    }

    #[test]
    fn test_content_serialization() {
        let content = Content {
            role: "user".to_string(),
            parts: vec![
                Part {
                    text: Some("Hello".to_string()),
                    function_call: None,
                    function_response: None,
                    thought: None,
                },
                Part {
                    text: None,
                    function_call: Some(FunctionCall {
                        name: "get_weather".to_string(),
                        args: json!({"location": "San Francisco"}),
                    }),
                    function_response: None,
                    thought: None,
                },
            ],
        };
        
        let serialized = serde_json::to_string(&content).unwrap();
        let deserialized: Content = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(content.role, deserialized.role);
        assert_eq!(content.parts.len(), deserialized.parts.len());
        assert_eq!(content.parts[0].text, deserialized.parts[0].text);
        assert_eq!(content.parts[1].function_call.as_ref().unwrap().name, 
                   deserialized.parts[1].function_call.as_ref().unwrap().name);
    }

    #[test]
    fn test_generation_config_serialization() {
        let config = GenerationConfig {
            temperature: Some(0.7),
            top_p: Some(0.9),
            top_k: Some(40),
            max_output_tokens: Some(1024),
            stop_sequences: Some(vec!["STOP".to_string(), "END".to_string()]),
            thinking_config: None,
        };
        
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: GenerationConfig = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(config.temperature, deserialized.temperature);
        assert_eq!(config.top_p, deserialized.top_p);
        assert_eq!(config.top_k, deserialized.top_k);
        assert_eq!(config.max_output_tokens, deserialized.max_output_tokens);
        assert_eq!(config.stop_sequences, deserialized.stop_sequences);
    }

    #[test]
    fn test_generation_config_with_none_values() {
        let config = GenerationConfig {
            temperature: None,
            top_p: None,
            top_k: None,
            max_output_tokens: None,
            stop_sequences: None,
            thinking_config: None,
        };
        
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: GenerationConfig = serde_json::from_str(&serialized).unwrap();
        
        assert!(deserialized.temperature.is_none());
        assert!(deserialized.top_p.is_none());
        assert!(deserialized.top_k.is_none());
        assert!(deserialized.max_output_tokens.is_none());
        assert!(deserialized.stop_sequences.is_none());
    }

    #[test]
    fn test_function_declaration_serialization() {
        let declaration = FunctionDeclaration {
            name: "calculate_sum".to_string(),
            description: "Calculates the sum of two numbers".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "a": {"type": "number", "description": "First number"},
                    "b": {"type": "number", "description": "Second number"}
                },
                "required": ["a", "b"]
            }),
        };
        
        let serialized = serde_json::to_string(&declaration).unwrap();
        let deserialized: FunctionDeclaration = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(declaration.name, deserialized.name);
        assert_eq!(declaration.description, deserialized.description);
        assert_eq!(declaration.parameters, deserialized.parameters);
    }

    #[test]
    fn test_tool_serialization() {
        let tool = Tool {
            function_declarations: Some(vec![
                FunctionDeclaration {
                    name: "function1".to_string(),
                    description: "First function".to_string(),
                    parameters: json!({"type": "object"}),
                },
                FunctionDeclaration {
                    name: "function2".to_string(),
                    description: "Second function".to_string(),
                    parameters: json!({"type": "object"}),
                },
            ]),
            google_search: None,
        };
        
        let serialized = serde_json::to_string(&tool).unwrap();
        let deserialized: Tool = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(tool.function_declarations.as_ref().unwrap().len(), deserialized.function_declarations.as_ref().unwrap().len());
        assert_eq!(tool.function_declarations.as_ref().unwrap()[0].name, deserialized.function_declarations.as_ref().unwrap()[0].name);
        assert_eq!(tool.function_declarations.as_ref().unwrap()[1].name, deserialized.function_declarations.as_ref().unwrap()[1].name);
    }

    #[test]
    fn test_gemini_request_serialization() {
        let request = GeminiRequest {
            contents: vec![
                Content {
                    role: "user".to_string(),
                    parts: vec![Part {
                        text: Some("Hello".to_string()),
                        function_call: None,
                        function_response: None,
                        thought: None,
                    }],
                },
            ],
            safety_settings: None,
            generation_config: Some(GenerationConfig {
                temperature: Some(0.7),
                top_p: None,
                top_k: None,
                max_output_tokens: Some(1024),
                stop_sequences: None,
                thinking_config: None,
            }),
            tools: Some(vec![Tool {
                function_declarations: Some(vec![FunctionDeclaration {
                    name: "test_tool".to_string(),
                    description: "A test tool".to_string(),
                    parameters: json!({"type": "object"}),
                }]),
                google_search: None,
            }]),
        };
        
        let serialized = serde_json::to_string(&request).unwrap();
        let deserialized: GeminiRequest = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(request.contents.len(), deserialized.contents.len());
        assert!(deserialized.generation_config.is_some());
        assert!(deserialized.tools.is_some());
        assert_eq!(request.tools.as_ref().unwrap().len(), deserialized.tools.as_ref().unwrap().len());
    }

    #[test]
    fn test_candidate_serialization() {
        let candidate = Candidate {
            content: Content {
                role: "model".to_string(),
                parts: vec![Part {
                    text: Some("Hello!".to_string()),
                    function_call: None,
                    function_response: None,
                    thought: None,
                }],
            },
            safety_ratings: vec![SafetyRating {
                category: "HARM_CATEGORY_HARASSMENT".to_string(),
                probability: "NEGLIGIBLE".to_string(),
            }],
            finish_reason: Some("STOP".to_string()),
            grounding_metadata: None,
        };
        
        let serialized = serde_json::to_string(&candidate).unwrap();
        let deserialized: Candidate = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(candidate.content.role, deserialized.content.role);
        assert_eq!(candidate.safety_ratings.len(), deserialized.safety_ratings.len());
        assert_eq!(candidate.finish_reason, deserialized.finish_reason);
    }

    #[test]
    fn test_gemini_response_serialization() {
        let response = GeminiResponse {
            candidates: vec![
                Candidate {
                    content: Content {
                        role: "model".to_string(),
                        parts: vec![Part {
                            text: Some("Response 1".to_string()),
                            function_call: None,
                            function_response: None,
                            thought: None,
                        }],
                    },
                    safety_ratings: vec![],
                    finish_reason: Some("STOP".to_string()),
                    grounding_metadata: None,
                }
            ],
            prompt_feedback: Some(PromptFeedback {
                safety_ratings: vec![],
                block_reason: None,
            }),
            usage_metadata: Some(UsageMetadata {
                prompt_token_count: Some(10),
                candidates_token_count: Some(20),
                total_token_count: Some(35),
                thoughts_token_count: None,
                cached_content_token_count: None,
            }),
        };
        
        let serialized = serde_json::to_string(&response).unwrap();
        let deserialized: GeminiResponse = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(response.candidates.len(), deserialized.candidates.len());
        assert!(deserialized.prompt_feedback.is_some());
        assert!(deserialized.usage_metadata.is_some());
    }

    #[test]
    fn test_gemini_error_serialization() {
        let error = GeminiError {
            error: ErrorInfo {
                code: 400,
                message: "Invalid request".to_string(),
                status: "INVALID_ARGUMENT".to_string(),
                details: vec![],
            },
        };
        
        let serialized = serde_json::to_string(&error).unwrap();
        let deserialized: GeminiError = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(error.error.code, deserialized.error.code);
        assert_eq!(error.error.message, deserialized.error.message);
        assert_eq!(error.error.status, deserialized.error.status);
    }

    #[test]
    fn test_message_to_content_conversion_user() {
        use crate::llm::client::{Message, Role, MessagePart};
        use uuid::Uuid;
        use std::collections::HashMap;
        
        let message = Message {
            id: Uuid::new_v4(),
            role: Role::User,
            parts: vec![MessagePart::Text {
                text: "Hello, world!".to_string(),
            }],
            metadata: HashMap::new(),
        };
        
        let content: Content = (&message).into();
        
        assert_eq!(content.role, "user");
        assert_eq!(content.parts.len(), 1);
        assert_eq!(content.parts[0].text, Some("Hello, world!".to_string()));
    }

    #[test]
    fn test_message_to_content_conversion_assistant() {
        use crate::llm::client::{Message, Role, MessagePart};
        use uuid::Uuid;
        use std::collections::HashMap;
        
        let message = Message {
            id: Uuid::new_v4(),
            role: Role::Assistant,
            parts: vec![MessagePart::Text {
                text: "Hello, human!".to_string(),
            }],
            metadata: HashMap::new(),
        };
        
        let content: Content = (&message).into();
        
        assert_eq!(content.role, "model");
        assert_eq!(content.parts.len(), 1);
        assert_eq!(content.parts[0].text, Some("Hello, human!".to_string()));
    }

    #[test]
    fn test_message_to_content_conversion_system() {
        use crate::llm::client::{Message, Role, MessagePart};
        use uuid::Uuid;
        use std::collections::HashMap;
        
        let message = Message {
            id: Uuid::new_v4(),
            role: Role::System,
            parts: vec![MessagePart::Text {
                text: "System message".to_string(),
            }],
            metadata: HashMap::new(),
        };
        
        let content: Content = (&message).into();
        
        assert_eq!(content.role, "user"); // System messages are converted to user role
        assert_eq!(content.parts.len(), 1);
        assert_eq!(content.parts[0].text, Some("System message".to_string()));
    }

    #[test]
    fn test_message_to_content_conversion_function() {
        use crate::llm::client::{Message, Role, MessagePart};
        use uuid::Uuid;
        use std::collections::HashMap;
        
        let message = Message {
            id: Uuid::new_v4(),
            role: Role::Function,
            parts: vec![MessagePart::ToolResult {
                tool_call_id: "call-123".to_string(),
                name: "get_weather".to_string(),
                result: json!({"temperature": 72, "condition": "sunny"}),
            }],
            metadata: HashMap::new(),
        };
        
        let content: Content = (&message).into();
        
        assert_eq!(content.role, "function");
        assert_eq!(content.parts.len(), 1);
        assert!(content.parts[0].text.is_none());
        assert!(content.parts[0].function_call.is_none());
        assert!(content.parts[0].function_response.is_some());
        
        let function_response = content.parts[0].function_response.as_ref().unwrap();
        assert_eq!(function_response.name, "get_weather");
        assert_eq!(function_response.response, json!({"temperature": 72, "condition": "sunny"}));
    }

    #[test]
    fn test_message_to_content_conversion_tool_call() {
        use crate::llm::client::{Message, Role, MessagePart};
        use uuid::Uuid;
        use std::collections::HashMap;
        
        let message = Message {
            id: Uuid::new_v4(),
            role: Role::Assistant,
            parts: vec![MessagePart::ToolCall {
                tool_call_id: "call-456".to_string(),
                name: "calculate_sum".to_string(),
                parameters: json!({"a": 5, "b": 3}),
            }],
            metadata: HashMap::new(),
        };
        
        let content: Content = (&message).into();
        
        assert_eq!(content.role, "model");
        assert_eq!(content.parts.len(), 1);
        assert!(content.parts[0].text.is_none());
        assert!(content.parts[0].function_response.is_none());
        assert!(content.parts[0].function_call.is_some());
        
        let function_call = content.parts[0].function_call.as_ref().unwrap();
        assert_eq!(function_call.name, "calculate_sum");
        assert_eq!(function_call.args, json!({"a": 5, "b": 3}));
    }

    #[test]
    fn test_message_to_content_conversion_multiple_parts() {
        use crate::llm::client::{Message, Role, MessagePart};
        use uuid::Uuid;
        use std::collections::HashMap;
        
        let message = Message {
            id: Uuid::new_v4(),
            role: Role::User,
            parts: vec![
                MessagePart::Text {
                    text: "Please calculate this:".to_string(),
                },
                MessagePart::Text {
                    text: "5 + 3".to_string(),
                },
            ],
            metadata: HashMap::new(),
        };
        
        let content: Content = (&message).into();
        
        assert_eq!(content.role, "user");
        assert_eq!(content.parts.len(), 2);
        assert_eq!(content.parts[0].text, Some("Please calculate this:".to_string()));
        assert_eq!(content.parts[1].text, Some("5 + 3".to_string()));
    }

    #[test]
    fn test_tool_definition_to_function_declaration_conversion() {
        use crate::llm::client::ToolDefinition;
        
        let tool_def = ToolDefinition {
            name: "search_web".to_string(),
            description: "Search the web for information".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query"
                    }
                },
                "required": ["query"]
            }),
            is_required: false,
        };
        
        let function_declaration: FunctionDeclaration = (&tool_def).into();
        
        assert_eq!(function_declaration.name, "search_web");
        assert_eq!(function_declaration.description, "Search the web for information");
        assert_eq!(function_declaration.parameters, tool_def.parameters);
    }

    #[test]
    fn test_empty_parts_handling() {
        let content = Content {
            role: "user".to_string(),
            parts: vec![],
        };
        
        let serialized = serde_json::to_string(&content).unwrap();
        let deserialized: Content = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(content.role, deserialized.role);
        assert!(deserialized.parts.is_empty());
    }

    #[test]
    fn test_part_with_all_none_fields() {
        let part = Part {
            text: None,
            function_call: None,
            function_response: None,
            thought: None,
        };
        
        let serialized = serde_json::to_string(&part).unwrap();
        let deserialized: Part = serde_json::from_str(&serialized).unwrap();
        
        assert!(deserialized.text.is_none());
        assert!(deserialized.function_call.is_none());
        assert!(deserialized.function_response.is_none());
    }

    #[test]
    fn test_complex_function_parameters() {
        let declaration = FunctionDeclaration {
            name: "complex_function".to_string(),
            description: "A function with complex parameters".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "nested_object": {
                        "type": "object",
                        "properties": {
                            "inner_array": {
                                "type": "array",
                                "items": {"type": "string"}
                            }
                        }
                    },
                    "optional_param": {"type": "string"},
                    "enum_param": {
                        "type": "string",
                        "enum": ["option1", "option2", "option3"]
                    }
                },
                "required": ["nested_object"]
            }),
        };
        
        let serialized = serde_json::to_string(&declaration).unwrap();
        let deserialized: FunctionDeclaration = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(declaration.name, deserialized.name);
        assert_eq!(declaration.description, deserialized.description);
        assert_eq!(declaration.parameters, deserialized.parameters);
    }

    #[test]
    fn test_thinking_config_serialization() {
        let thinking_config = ThinkingConfig {
            include_thoughts: Some(true),
            thinking_budget: Some(1024),
        };
        
        let serialized = serde_json::to_string(&thinking_config).unwrap();
        let deserialized: ThinkingConfig = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(thinking_config.include_thoughts, deserialized.include_thoughts);
        assert_eq!(thinking_config.thinking_budget, deserialized.thinking_budget);
    }

    #[test]
    fn test_thinking_config_with_none_values() {
        let thinking_config = ThinkingConfig {
            include_thoughts: None,
            thinking_budget: None,
        };
        
        let serialized = serde_json::to_string(&thinking_config).unwrap();
        let deserialized: ThinkingConfig = serde_json::from_str(&serialized).unwrap();
        
        assert!(deserialized.include_thoughts.is_none());
        assert!(deserialized.thinking_budget.is_none());
    }

    #[test]
    fn test_generation_config_with_thinking() {
        let config = GenerationConfig {
            temperature: Some(0.7),
            top_p: Some(0.9),
            top_k: Some(40),
            max_output_tokens: Some(1024),
            stop_sequences: None,
            thinking_config: Some(ThinkingConfig {
                include_thoughts: Some(true),
                thinking_budget: Some(2048),
            }),
        };
        
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: GenerationConfig = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(config.temperature, deserialized.temperature);
        assert!(deserialized.thinking_config.is_some());
        let thinking = deserialized.thinking_config.unwrap();
        assert_eq!(Some(true), thinking.include_thoughts);
        assert_eq!(Some(2048), thinking.thinking_budget);
    }

    #[test]
    fn test_part_with_thought() {
        let part = Part {
            text: Some("This is a thought summary".to_string()),
            function_call: None,
            function_response: None,
            thought: Some(true),
        };
        
        let serialized = serde_json::to_string(&part).unwrap();
        let deserialized: Part = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(part.text, deserialized.text);
        assert_eq!(part.thought, deserialized.thought);
        assert!(deserialized.function_call.is_none());
        assert!(deserialized.function_response.is_none());
    }

    #[test]
    fn test_usage_metadata_with_thinking_tokens() {
        let usage = UsageMetadata {
            prompt_token_count: Some(10),
            candidates_token_count: Some(20),
            total_token_count: Some(35),
            thoughts_token_count: Some(5),
            cached_content_token_count: None,
        };
        
        let serialized = serde_json::to_string(&usage).unwrap();
        let deserialized: UsageMetadata = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(usage.prompt_token_count, deserialized.prompt_token_count);
        assert_eq!(usage.candidates_token_count, deserialized.candidates_token_count);
        assert_eq!(usage.total_token_count, deserialized.total_token_count);
        assert_eq!(usage.thoughts_token_count, deserialized.thoughts_token_count);
    }
}