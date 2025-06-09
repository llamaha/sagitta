use anyhow::Result;
use chrono::Utc;
use std::sync::Arc;

use crate::agent::message::types::AgentMessage;
use crate::llm::client::{LlmClient, Role};
use sagitta_embed::EmbeddingPool;

/// Configuration for title generation
#[derive(Debug, Clone)]
pub struct TitleGeneratorConfig {
    /// Maximum length for generated titles
    pub max_title_length: usize,
    
    /// Minimum number of messages before generating a title
    pub min_messages_for_generation: usize,
    
    /// Whether to use embeddings for context analysis
    pub use_embeddings: bool,
    
    /// Fallback title prefix when generation fails
    pub fallback_prefix: String,
}

impl Default for TitleGeneratorConfig {
    fn default() -> Self {
        Self {
            max_title_length: 50,
            min_messages_for_generation: 1,
            use_embeddings: true,
            fallback_prefix: "Conversation".to_string(),
        }
    }
}

/// Generator for conversation titles using LLM
pub struct TitleGenerator {
    config: TitleGeneratorConfig,
    llm_client: Option<Arc<dyn LlmClient>>,
    embedding_pool: Option<Arc<EmbeddingPool>>,
    should_fail: bool, // For testing failure scenarios
}

impl TitleGenerator {
    /// Create a new title generator
    pub fn new(config: TitleGeneratorConfig) -> Self {
        Self {
            config,
            llm_client: None,
            embedding_pool: None,
            should_fail: false,
        }
    }
    
    /// Set the LLM client for title generation
    pub fn with_llm_client(mut self, client: Arc<dyn LlmClient>) -> Self {
        self.llm_client = Some(client);
        self
    }
    
    /// Set the embedding pool for context analysis
    pub fn with_embedding_pool(mut self, pool: Arc<EmbeddingPool>) -> Self {
        self.embedding_pool = Some(pool);
        self
    }
    
    /// Create a failing title generator for testing
    pub fn failing() -> Self {
        Self {
            config: TitleGeneratorConfig::default(),
            llm_client: None,
            embedding_pool: None,
            should_fail: true,
        }
    }
    
    /// Generate a title for a conversation based on its messages
    pub async fn generate_title(&self, messages: &[AgentMessage]) -> Result<String> {
        // Handle test failure scenario
        if self.should_fail {
            return Ok(self.generate_fallback_title());
        }
        
        // Check if we have enough messages
        if messages.len() < self.config.min_messages_for_generation {
            return Ok(self.generate_fallback_title());
        }
        
        // Try LLM generation first
        if let Some(ref llm_client) = self.llm_client {
            match self.generate_with_llm(messages, llm_client).await {
                Ok(title) => {
                    let truncated = self.truncate_title(title);
                    if !truncated.is_empty() {
                        return Ok(truncated);
                    }
                }
                Err(e) => {
                    eprintln!("LLM title generation failed: {}", e);
                }
            }
        }
        
        // Fall back to rule-based generation
        self.generate_rule_based_title(messages)
    }
    
    /// Generate title using LLM
    async fn generate_with_llm(&self, messages: &[AgentMessage], llm_client: &Arc<dyn LlmClient>) -> Result<String> {
        // Take first few messages for context (limit to avoid token limits)
        let context_messages: Vec<_> = messages.iter()
            .take(5)
            .map(|msg| format!("{}: {}", 
                match msg.role {
                    Role::User => "User",
                    Role::Assistant => "Assistant",
                    Role::System => "System",
                    Role::Function => "Function",
                },
                msg.content.chars().take(200).collect::<String>()
            ))
            .collect();
        
        let context = context_messages.join("\n");
        
        let prompt = format!(
            "Generate a concise, descriptive title for this conversation. The title should be under {} characters and capture the main topic or purpose. Do not include quotes or extra formatting.\n\nConversation:\n{}\n\nTitle:",
            self.config.max_title_length,
            context
        );
        
        // Convert to LlmClient message format
        use crate::llm::client::{Message, MessagePart};
        let llm_messages = vec![
            Message {
                id: uuid::Uuid::new_v4(),
                role: Role::User,
                parts: vec![MessagePart::Text { text: prompt }],
                metadata: std::collections::HashMap::new(),
            }
        ];
        
        let response = llm_client.generate(&llm_messages, &[]).await
            .map_err(|e| anyhow::anyhow!("LLM generation failed: {}", e))?;
        
        // Extract text from response
        let title = response.message.parts.iter()
            .find_map(|part| match part {
                MessagePart::Text { text } => Some(text.clone()),
                _ => None,
            })
            .unwrap_or_default()
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();
        
        Ok(title)
    }
    
    /// Generate title using rule-based approach
    fn generate_rule_based_title(&self, messages: &[AgentMessage]) -> Result<String> {
        if messages.is_empty() {
            return Ok(self.generate_fallback_title());
        }
        
        // Get the first user message for context
        let first_user_message = messages.iter()
            .find(|msg| msg.role == Role::User)
            .map(|msg| &msg.content)
            .unwrap_or(&messages[0].content);
        
        // Extract key topics using simple keyword matching
        let content_lower = first_user_message.to_lowercase();
        
        let title = if content_lower.contains("rust") || content_lower.contains("cargo") {
            if content_lower.contains("error") || content_lower.contains("panic") {
                "Rust Error Handling Help".to_string()
            } else if content_lower.contains("async") || content_lower.contains("tokio") {
                "Rust Async Programming".to_string()
            } else {
                "Rust Programming Help".to_string()
            }
        } else if content_lower.contains("python") {
            if content_lower.contains("data") || content_lower.contains("pandas") {
                "Python Data Analysis".to_string()
            } else {
                "Python Programming Help".to_string()
            }
        } else if content_lower.contains("javascript") || content_lower.contains("js") {
            "JavaScript Development".to_string()
        } else if content_lower.contains("web") || content_lower.contains("html") || content_lower.contains("css") {
            "Web Development Help".to_string()
        } else if content_lower.contains("database") || content_lower.contains("sql") {
            "Database Query Help".to_string()
        } else if content_lower.contains("api") || content_lower.contains("rest") {
            "API Development".to_string()
        } else if content_lower.contains("debug") || content_lower.contains("fix") {
            "Debugging Assistance".to_string()
        } else if content_lower.contains("dog") || content_lower.contains("pet") || content_lower.contains("cat") {
            if content_lower.contains("name") {
                "Pet Naming Advice".to_string()
            } else {
                "Pet Care Discussion".to_string()
            }
        } else if content_lower.contains("recipe") || content_lower.contains("cooking") || content_lower.contains("food") {
            "Cooking & Recipe Help".to_string()
        } else if content_lower.contains("travel") || content_lower.contains("vacation") {
            "Travel Planning".to_string()
        } else if content_lower.contains("book") || content_lower.contains("reading") {
            "Book Recommendations".to_string()
        } else if content_lower.contains("movie") || content_lower.contains("film") {
            "Movie Discussion".to_string()
        } else if content_lower.contains("music") || content_lower.contains("song") {
            "Music Discussion".to_string()
        } else if content_lower.contains("health") || content_lower.contains("exercise") || content_lower.contains("fitness") {
            "Health & Fitness".to_string()
        } else if content_lower.contains("how") && content_lower.contains("?") {
            // Extract the "how to" part
            if let Some(start) = content_lower.find("how") {
                let question_part = &first_user_message[start..];
                let words: Vec<&str> = question_part.split_whitespace().take(4).collect();
                format!("{}", words.join(" "))
            } else {
                "How-To Question".to_string()
            }
        } else if content_lower.contains("what") && content_lower.contains("?") {
            "General Question".to_string()
        } else if content_lower.contains("why") && content_lower.contains("?") {
            "Explanation Request".to_string()
        } else if content_lower.contains("where") && content_lower.contains("?") {
            "Location Question".to_string()
        } else if content_lower.contains("when") && content_lower.contains("?") {
            "Timing Question".to_string()
        } else {
            // Use first few words of the message, but make it more title-like
            let words: Vec<&str> = first_user_message.split_whitespace().take(4).collect();
            if words.is_empty() {
                return Ok(self.generate_fallback_title());
            }
            let raw_title = words.join(" ");
            
            // If it ends with a question mark, make it more title-like
            if raw_title.ends_with("?") {
                if raw_title.to_lowercase().starts_with("what") {
                    "General Question".to_string()
                } else if raw_title.to_lowercase().starts_with("how") {
                    "How-To Question".to_string()
                } else {
                    "Question & Answer".to_string()
                }
            } else {
                raw_title
            }
        };
        
        Ok(self.truncate_title(title))
    }
    
    /// Generate a fallback title with timestamp
    fn generate_fallback_title(&self) -> String {
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M");
        format!("{} {}", self.config.fallback_prefix, timestamp)
    }
    
    /// Truncate title to maximum length
    fn truncate_title(&self, title: String) -> String {
        if title.len() <= self.config.max_title_length {
            title
        } else {
            let truncated = title.chars().take(self.config.max_title_length - 3).collect::<String>();
            format!("{}...", truncated)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::message::types::AgentMessage;
    use crate::llm::client::Role;
    use uuid::Uuid;
    
    #[tokio::test]
    async fn test_rule_based_title_generation() {
        let generator = TitleGenerator::new(TitleGeneratorConfig::default());
        
        let messages = vec![
            AgentMessage {
                id: Uuid::new_v4(),
                role: Role::User,
                content: "How do I handle errors in Rust?".to_string(),
                is_streaming: false,
                timestamp: Utc::now(),
                metadata: Default::default(),
                tool_calls: vec![],
            }
        ];
        
        let title = generator.generate_title(&messages).await.unwrap();
        assert_eq!(title, "Rust Error Handling Help");
    }
    
    #[tokio::test]
    async fn test_fallback_title_generation() {
        let generator = TitleGenerator::new(TitleGeneratorConfig::default());
        
        let empty_messages = vec![];
        let title = generator.generate_title(&empty_messages).await.unwrap();
        assert!(title.starts_with("Conversation"));
    }
    
    #[tokio::test]
    async fn test_title_truncation() {
        let generator = TitleGenerator::new(TitleGeneratorConfig {
            max_title_length: 20,
            ..Default::default()
        });
        
        let long_title = "This is a very long title that should be truncated".to_string();
        let truncated = generator.truncate_title(long_title);
        assert!(truncated.len() <= 20);
        assert!(truncated.ends_with("..."));
    }
    
    #[tokio::test]
    async fn test_failing_generator() {
        let generator = TitleGenerator::failing();
        
        let messages = vec![
            AgentMessage {
                id: Uuid::new_v4(),
                role: Role::User,
                content: "Test message".to_string(),
                is_streaming: false,
                timestamp: Utc::now(),
                metadata: Default::default(),
                tool_calls: vec![],
            }
        ];
        
        let title = generator.generate_title(&messages).await.unwrap();
        assert!(title.starts_with("Conversation"));
    }
} 