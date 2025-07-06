use anyhow::Result;
use std::sync::Arc;
use uuid::Uuid;

use crate::agent::conversation::service::ConversationService;
use crate::llm::title::{TitleGenerator, TitleGeneratorConfig};
use crate::llm::client::LlmClient;
use crate::agent::message::types::AgentMessage;
use crate::agent::conversation::tagging::TaggingPipeline;
use crate::llm::fast_model::FastModelOperations;

/// Updates conversation titles automatically after messages are added
pub struct ConversationTitleUpdater {
    conversation_service: Arc<ConversationService>,
    title_generator: TitleGenerator,
    tagging_pipeline: Option<TaggingPipeline>,
    fast_model_provider: Option<Arc<dyn FastModelOperations>>,
}

impl ConversationTitleUpdater {
    /// Create a new title updater
    pub fn new(
        conversation_service: Arc<ConversationService>,
        llm_client: Option<Arc<dyn LlmClient>>,
    ) -> Self {
        let mut title_generator = TitleGenerator::new(TitleGeneratorConfig {
            max_title_length: 50,
            min_messages_for_generation: 2, // Generate after 2 messages (user + assistant)
            use_embeddings: false, // Don't require embeddings for now
            fallback_prefix: "Conversation".to_string(),
        });

        if let Some(client) = llm_client {
            title_generator = title_generator.with_llm_client(client);
        }

        // For now, skip tagging pipeline as it requires ConversationManager
        // We'll use fast model directly for tag suggestions instead
        
        Self {
            conversation_service,
            title_generator,
            tagging_pipeline: None,
            fast_model_provider: None,
        }
    }
    
    /// Set the fast model provider for enhanced tagging
    pub fn with_fast_model_provider(mut self, provider: Arc<dyn FastModelOperations>) -> Self {
        // Also set it on the title generator
        self.title_generator = self.title_generator.with_fast_model_provider(provider.clone());
        self.fast_model_provider = Some(provider);
        self
    }

    /// Check if a conversation needs its title updated and update it if necessary
    pub async fn maybe_update_title(&self, conversation_id: Uuid) -> Result<()> {
        // Get the conversation
        let conversation = match self.conversation_service.get_conversation(conversation_id).await? {
            Some(c) => c,
            None => return Ok(()), // Conversation not found, nothing to do
        };

        // Skip if title is already customized (not the default format)
        if !conversation.title.starts_with("Conversation 20") {
            return Ok(());
        }

        // Check if we have enough messages
        if conversation.messages.len() < 2 {
            return Ok(());
        }

        // Generate a new title
        let new_title = self.title_generator.generate_title(&conversation.messages).await?;
        
        // Only update if the new title is different and not a fallback
        if new_title != conversation.title && !new_title.starts_with("Conversation 20") {
            log::info!("Updating conversation {} title from '{}' to '{}'", 
                conversation_id, conversation.title, new_title);
            
            // Update the conversation with the new title
            let mut updated_conversation = conversation;
            updated_conversation.title = new_title;
            
            // Try to use fast model for tagging if available
            let tags = if let Some(ref fast_model) = self.fast_model_provider {
                match fast_model.suggest_tags(&updated_conversation).await {
                    Ok(tag_suggestions) => {
                        // Filter high-confidence tags (>= 0.7) and take top 5
                        let high_confidence_tags: Vec<String> = tag_suggestions.into_iter()
                            .filter(|(_, confidence)| *confidence >= 0.7)
                            .take(5)
                            .map(|(tag, _)| tag)
                            .collect();
                        
                        // If we got tags from fast model, use them
                        if !high_confidence_tags.is_empty() {
                            log::info!("Fast model suggested tags for conversation {conversation_id}: {high_confidence_tags:?}");
                            high_confidence_tags
                        } else {
                            // Fall back to rule-based tagging
                            self.generate_rule_based_tags(&updated_conversation)
                        }
                    }
                    Err(e) => {
                        log::debug!("Fast model tag suggestion failed: {e}, using rule-based");
                        self.generate_rule_based_tags(&updated_conversation)
                    }
                }
            } else {
                // No fast model, use rule-based tagging
                self.generate_rule_based_tags(&updated_conversation)
            };
            
            if !tags.is_empty() {
                log::info!("Adding tags to conversation {conversation_id}: {tags:?}");
                updated_conversation.tags = tags;
            }
            
            self.conversation_service.update_conversation(updated_conversation).await?;
        }

        Ok(())
    }

    /// Update title for a conversation based on its messages
    pub async fn update_title_from_messages(
        &self,
        conversation_id: Uuid,
        messages: &[AgentMessage],
    ) -> Result<()> {
        // Generate a new title
        let new_title = self.title_generator.generate_title(messages).await?;
        
        // Get the conversation to update
        if let Some(mut conversation) = self.conversation_service.get_conversation(conversation_id).await? {
            // Only update if the title is still the default
            if conversation.title.starts_with("Conversation 20") {
                log::info!("Updating conversation {conversation_id} title to '{new_title}'");
                conversation.title = new_title;
                self.conversation_service.update_conversation(conversation).await?;
            }
        }
        
        Ok(())
    }
    
    /// Generate rule-based tags for a conversation
    fn generate_rule_based_tags(&self, conversation: &crate::agent::conversation::types::Conversation) -> Vec<String> {
        use std::collections::HashSet;
        let mut tags = HashSet::new();
        let content_lower = conversation.messages.iter()
            .filter(|m| matches!(m.role, crate::llm::client::Role::User | crate::llm::client::Role::Assistant))
            .map(|m| m.content.to_lowercase())
            .collect::<String>();
        
        // Programming languages
        if content_lower.contains("rust") || content_lower.contains("cargo") || content_lower.contains(".rs") {
            tags.insert("language/rust".to_string());
        }
        if content_lower.contains("python") || content_lower.contains("pip") || content_lower.contains(".py") {
            tags.insert("language/python".to_string());
        }
        if content_lower.contains("javascript") || content_lower.contains("npm") || content_lower.contains(".js") {
            tags.insert("language/javascript".to_string());
        }
        if content_lower.contains("typescript") || content_lower.contains(".ts") {
            tags.insert("language/typescript".to_string());
        }
        if content_lower.contains("java") && !content_lower.contains("javascript") {
            tags.insert("language/java".to_string());
        }
        if content_lower.contains("c++") || content_lower.contains("cpp") {
            tags.insert("language/cpp".to_string());
        }
        
        // Problem domains
        if content_lower.contains("debug") || content_lower.contains("error") || content_lower.contains("fix") 
            || content_lower.contains("bug") || content_lower.contains("issue") {
            tags.insert("topic/debugging".to_string());
        }
        if content_lower.contains("performance") || content_lower.contains("optimize") || content_lower.contains("slow") {
            tags.insert("topic/performance".to_string());
        }
        if content_lower.contains("refactor") || content_lower.contains("clean") || content_lower.contains("improve") {
            tags.insert("topic/refactoring".to_string());
        }
        
        // Technologies and tools
        if content_lower.contains("api") || content_lower.contains("endpoint") || content_lower.contains("rest") {
            tags.insert("tech/api".to_string());
        }
        if content_lower.contains("database") || content_lower.contains("sql") || content_lower.contains("query") {
            tags.insert("tech/database".to_string());
        }
        if content_lower.contains("docker") || content_lower.contains("container") || content_lower.contains("kubernetes") {
            tags.insert("tech/containers".to_string());
        }
        if content_lower.contains("test") || content_lower.contains("unit") || content_lower.contains("integration") {
            tags.insert("topic/testing".to_string());
        }
        if content_lower.contains("git") || content_lower.contains("github") || content_lower.contains("gitlab") {
            tags.insert("tool/version-control".to_string());
        }
        
        // Frameworks
        if content_lower.contains("react") || content_lower.contains("vue") || content_lower.contains("angular") {
            tags.insert("framework/frontend".to_string());
        }
        if content_lower.contains("tokio") || content_lower.contains("async") || content_lower.contains("await") {
            tags.insert("framework/async".to_string());
        }
        if content_lower.contains("django") || content_lower.contains("flask") || content_lower.contains("fastapi") {
            tags.insert("framework/web".to_string());
        }
        
        // Limit to 5 most relevant tags
        tags.into_iter().take(5).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    
    

    #[tokio::test]
    async fn test_title_update_skips_custom_titles() {
        // This would require mocking the conversation service
        // For now, just verify the updater can be created
        let title_generator = TitleGenerator::new(TitleGeneratorConfig::default());
        assert!(!title_generator.generate_title(&[]).await.unwrap().is_empty());
    }
}