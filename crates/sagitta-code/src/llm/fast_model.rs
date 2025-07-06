use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use crate::config::types::SagittaCodeConfig;
use crate::llm::claude_code::client::ClaudeCodeClient;
use crate::llm::client::{LlmClient, Message, LlmResponse};
use crate::agent::message::types::AgentMessage;
use crate::agent::conversation::types::Conversation;
use crate::agent::state::types::ConversationStatus;
use crate::utils::errors::SagittaCodeError;

/// Trait for fast model operations
#[async_trait]
pub trait FastModelOperations: Send + Sync {
    /// Generate a title for the conversation
    async fn generate_title(&self, messages: &[AgentMessage]) -> Result<String>;
    
    /// Suggest tags for the conversation
    async fn suggest_tags(&self, conversation: &Conversation) -> Result<Vec<(String, f32)>>;
    
    /// Evaluate and suggest status for the conversation
    async fn evaluate_status(&self, conversation: &Conversation) -> Result<(ConversationStatus, f32)>;
    
    /// Suggest branch points for the conversation
    async fn suggest_branch_points(&self, conversation: &Conversation) -> Result<Vec<(usize, String, f32)>>;
}

/// Fast model provider using Claude Code
pub struct FastModelProvider {
    config: SagittaCodeConfig,
    client: Option<Arc<dyn LlmClient>>,
}

impl FastModelProvider {
    /// Create a new fast model provider
    pub fn new(config: SagittaCodeConfig) -> Self {
        Self {
            config,
            client: None,
        }
    }
    
    /// Initialize the provider with the appropriate client
    pub async fn initialize(&mut self) -> Result<(), SagittaCodeError> {
        if self.config.conversation.enable_fast_model {
            // Create a new config with the fast model
            let mut fast_config = self.config.clone();
            fast_config.claude_code.model = self.config.conversation.fast_model.clone();
            
            // Create the client
            let client = ClaudeCodeClient::new(&fast_config)?;
            self.client = Some(Arc::new(client) as Arc<dyn LlmClient>);
            
            log::info!("FastModelProvider: Initialized with model {}", self.config.conversation.fast_model);
        } else {
            log::info!("FastModelProvider: Fast model disabled, will use rule-based fallbacks");
        }
        
        Ok(())
    }
    
    /// Get the client, creating it if necessary
    async fn get_client(&self) -> Option<&Arc<dyn LlmClient>> {
        self.client.as_ref()
    }

    /// Generate simple text response from a prompt (for commit messages, etc.)
    pub async fn generate_simple_text(&self, prompt: &str) -> Result<String> {
        if let Some(client) = self.get_client().await {
            let prompt_messages = vec![Message {
                id: uuid::Uuid::new_v4(),
                role: crate::llm::client::Role::User,
                parts: vec![crate::llm::client::MessagePart::Text { text: prompt.to_string() }],
                metadata: Default::default(),
            }];
            
            // Generate with fast model
            let response = client.generate(&prompt_messages, &[]).await
                .map_err(|e| anyhow::anyhow!("Failed to generate text: {}", e))?;
            
            // Extract text from response
            let text = extract_text_from_response(&response);
            Ok(text.trim().to_string())
        } else {
            Err(anyhow::anyhow!("Fast model not available"))
        }
    }
}

#[async_trait]
impl FastModelOperations for FastModelProvider {
    async fn generate_title(&self, messages: &[AgentMessage]) -> Result<String> {
        if let Some(client) = self.get_client().await {
            // Convert legacy messages to new format
            let converted_messages = convert_legacy_messages(messages);
            
            // Create a prompt for title generation
            let prompt = create_title_prompt(&converted_messages);
            let prompt_messages = vec![Message {
                id: uuid::Uuid::new_v4(),
                role: crate::llm::client::Role::User,
                parts: vec![crate::llm::client::MessagePart::Text { text: prompt }],
                metadata: Default::default(),
            }];
            
            // Generate with fast model
            let response = client.generate(&prompt_messages, &[]).await
                .map_err(|e| anyhow::anyhow!("Failed to generate title: {}", e))?;
            
            // Extract title from response
            let title = extract_text_from_response(&response)
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            
            Ok(title)
        } else {
            // Fallback to rule-based
            Err(anyhow::anyhow!("Fast model not available"))
        }
    }
    
    async fn suggest_tags(&self, conversation: &Conversation) -> Result<Vec<(String, f32)>> {
        if let Some(client) = self.get_client().await {
            // Create a prompt for tag suggestion
            let prompt = create_tag_prompt(conversation);
            let prompt_messages = vec![Message {
                id: uuid::Uuid::new_v4(),
                role: crate::llm::client::Role::User,
                parts: vec![crate::llm::client::MessagePart::Text { text: prompt }],
                metadata: Default::default(),
            }];
            
            // Generate with fast model
            let response = client.generate(&prompt_messages, &[]).await
                .map_err(|e| anyhow::anyhow!("Failed to suggest tags: {}", e))?;
            
            // Parse tags from response
            let tags = parse_tags_from_response(&response)?;
            Ok(tags)
        } else {
            // Fallback to rule-based
            Err(anyhow::anyhow!("Fast model not available"))
        }
    }
    
    async fn evaluate_status(&self, conversation: &Conversation) -> Result<(ConversationStatus, f32)> {
        if let Some(client) = self.get_client().await {
            // Create a prompt for status evaluation
            let prompt = create_status_prompt(conversation);
            let prompt_messages = vec![Message {
                id: uuid::Uuid::new_v4(),
                role: crate::llm::client::Role::User,
                parts: vec![crate::llm::client::MessagePart::Text { text: prompt }],
                metadata: Default::default(),
            }];
            
            // Generate with fast model
            let response = client.generate(&prompt_messages, &[]).await
                .map_err(|e| anyhow::anyhow!("Failed to evaluate status: {}", e))?;
            
            // Parse status from response
            let status = parse_status_from_response(&response)?;
            Ok(status)
        } else {
            // Fallback to rule-based
            Err(anyhow::anyhow!("Fast model not available"))
        }
    }
    
    async fn suggest_branch_points(&self, conversation: &Conversation) -> Result<Vec<(usize, String, f32)>> {
        if let Some(client) = self.get_client().await {
            // Create a prompt for branch point suggestion
            let prompt = create_branch_prompt(conversation);
            let prompt_messages = vec![Message {
                id: uuid::Uuid::new_v4(),
                role: crate::llm::client::Role::User,
                parts: vec![crate::llm::client::MessagePart::Text { text: prompt }],
                metadata: Default::default(),
            }];
            
            // Generate with fast model
            let response = client.generate(&prompt_messages, &[]).await
                .map_err(|e| anyhow::anyhow!("Failed to suggest branch points: {}", e))?;
            
            // Parse branch suggestions from response
            let suggestions = parse_branch_suggestions_from_response(&response)?;
            Ok(suggestions)
        } else {
            // Fallback to empty suggestions
            Err(anyhow::anyhow!("Fast model not available"))
        }
    }
}

// Helper functions

fn convert_legacy_messages(messages: &[AgentMessage]) -> Vec<Message> {
    messages.iter().map(|msg| Message {
        id: msg.id,
        role: msg.role.clone(),
        parts: vec![crate::llm::client::MessagePart::Text { text: msg.content.clone() }],
        metadata: Default::default(),
    }).collect()
}

fn create_title_prompt(messages: &[Message]) -> String {
    let context: Vec<String> = messages.iter()
        .take(5)
        .map(|msg| {
            let role = match msg.role {
                crate::llm::client::Role::User => "User",
                crate::llm::client::Role::Assistant => "Assistant",
                _ => "System",
            };
            let content = msg.parts.iter()
                .find_map(|p| match p {
                    crate::llm::client::MessagePart::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            format!("{}: {}", role, content.chars().take(200).collect::<String>())
        })
        .collect();
    
    format!(
        "Generate a concise, descriptive title for this conversation. \
        The title should be under 50 characters and capture the main topic or purpose. \
        Do not include quotes or extra formatting.\n\n\
        Conversation:\n{}\n\n\
        Title:",
        context.join("\n")
    )
}

fn create_tag_prompt(conversation: &Conversation) -> String {
    let messages_summary = conversation.messages.iter()
        .filter(|msg| matches!(msg.role, crate::llm::client::Role::User | crate::llm::client::Role::Assistant))
        .take(10)
        .map(|msg| format!("{}: {}", 
            match msg.role {
                crate::llm::client::Role::User => "User",
                crate::llm::client::Role::Assistant => "Assistant",
                _ => "System",
            },
            msg.content.chars().take(300).collect::<String>()
        ))
        .collect::<Vec<_>>()
        .join("\n");
    
    format!(
        "Analyze this conversation and suggest relevant tags for categorization and search.\n\n\
        Guidelines:\n\
        - Focus on programming languages, frameworks, tools, and concepts mentioned\n\
        - Include problem domains (e.g., debugging, api-design, testing)\n\
        - Use lowercase, hyphenated tags (e.g., 'version-control', not 'Version Control')\n\
        - Prefer specific over generic tags when possible\n\
        - Consider hierarchical tags (e.g., 'language/rust', 'topic/debugging')\n\n\
        Return up to 5 tags with confidence scores (0.0-1.0) in the format:\n\
        tag_name:score\n\n\
        Current title: {}\n\
        Messages:\n{}\n\n\
        Tags:",
        conversation.title,
        messages_summary
    )
}

fn create_status_prompt(conversation: &Conversation) -> String {
    let last_messages = conversation.messages.iter()
        .rev()
        .take(3)
        .map(|msg| format!("{}: {}", 
            match msg.role {
                crate::llm::client::Role::User => "User",
                crate::llm::client::Role::Assistant => "Assistant",
                _ => "System",
            },
            msg.content.chars().take(200).collect::<String>()
        ))
        .collect::<Vec<_>>();
    
    format!(
        "Evaluate the status of this conversation. \
        Choose one of: Active, Paused, Completed, Archived, Summarizing. \
        Return the status with a confidence score (0.0-1.0) in the format: \
        status:score\n\n\
        Current status: {:?}\n\
        Message count: {}\n\
        Last messages:\n{}\n\n\
        Status:",
        conversation.status,
        conversation.messages.len(),
        last_messages.join("\n")
    )
}

fn extract_text_from_response(response: &LlmResponse) -> String {
    response.message.parts.iter()
        .find_map(|part| match part {
            crate::llm::client::MessagePart::Text { text } => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

fn parse_tags_from_response(response: &LlmResponse) -> Result<Vec<(String, f32)>> {
    let text = extract_text_from_response(response);
    let mut tags = Vec::new();
    
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        
        // Handle both simple tags and hierarchical tags
        if let Some((tag, score_str)) = line.split_once(':') {
            let tag = tag.trim();
            
            // Validate tag format (lowercase, alphanumeric with hyphens/slashes)
            if tag.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '/') {
                if let Ok(score) = score_str.trim().parse::<f32>() {
                    tags.push((tag.to_string(), score.clamp(0.0, 1.0)));
                }
            } else {
                // Normalize tag to expected format
                let normalized = tag.to_lowercase().replace(' ', "-");
                if let Ok(score) = score_str.trim().parse::<f32>() {
                    tags.push((normalized, score.clamp(0.0, 1.0)));
                }
            }
        }
    }
    
    // Sort by confidence score (highest first)
    tags.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    
    Ok(tags)
}

fn parse_status_from_response(response: &LlmResponse) -> Result<(ConversationStatus, f32)> {
    let text = extract_text_from_response(response);
    
    if let Some((status_str, score_str)) = text.trim().split_once(':') {
        let status = match status_str.trim().to_lowercase().as_str() {
            "active" => ConversationStatus::Active,
            "paused" => ConversationStatus::Paused,
            "completed" => ConversationStatus::Completed,
            "archived" => ConversationStatus::Archived,
            "summarizing" => ConversationStatus::Summarizing,
            _ => ConversationStatus::Active,
        };
        
        let score = score_str.trim().parse::<f32>()
            .unwrap_or(0.5)
            .clamp(0.0, 1.0);
        
        Ok((status, score))
    } else {
        Ok((ConversationStatus::Active, 0.5))
    }
}

fn create_branch_prompt(conversation: &Conversation) -> String {
    let messages_text = conversation.messages.iter()
        .filter(|msg| matches!(msg.role, crate::llm::client::Role::User | crate::llm::client::Role::Assistant))
        .enumerate()
        .map(|(i, msg)| format!("[{}] {}: {}", 
            i,
            match msg.role {
                crate::llm::client::Role::User => "User",
                crate::llm::client::Role::Assistant => "Assistant",
                _ => "System",
            },
            msg.content.chars().take(500).collect::<String>()
        ))
        .collect::<Vec<_>>()
        .join("\n");
    
    format!(
        "Analyze this conversation and identify good branch points where alternative approaches could be explored.\n\n\
        A good branch point is where:\n\
        - Multiple valid solutions exist\n\
        - An error or problem was encountered\n\
        - The user expressed uncertainty\n\
        - A complex problem could benefit from parallel exploration\n\n\
        Return up to 3 branch points in the format:\n\
        message_index:reason:confidence\n\n\
        Example:\n\
        5:error_recovery:0.8\n\
        12:alternative_approach:0.6\n\n\
        Conversation:\n{messages_text}\n\n\
        Branch points:"
    )
}

fn parse_branch_suggestions_from_response(response: &LlmResponse) -> Result<Vec<(usize, String, f32)>> {
    let text = extract_text_from_response(response);
    let mut suggestions = Vec::new();
    
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 3 {
            if let Ok(index) = parts[0].trim().parse::<usize>() {
                let reason = parts[1].trim().to_string();
                if let Ok(confidence) = parts[2].trim().parse::<f32>() {
                    suggestions.push((index, reason, confidence.clamp(0.0, 1.0)));
                }
            }
        }
    }
    
    Ok(suggestions)
}