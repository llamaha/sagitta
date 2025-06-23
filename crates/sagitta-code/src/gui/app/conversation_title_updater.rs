use anyhow::Result;
use std::sync::Arc;
use uuid::Uuid;

use crate::agent::conversation::service::ConversationService;
use crate::llm::title::{TitleGenerator, TitleGeneratorConfig};
use crate::llm::client::LlmClient;
use crate::agent::message::types::AgentMessage;
use crate::agent::conversation::tagging::{TaggingPipeline, TaggingPipelineConfig};

/// Updates conversation titles automatically after messages are added
pub struct ConversationTitleUpdater {
    conversation_service: Arc<ConversationService>,
    title_generator: TitleGenerator,
    tagging_pipeline: Option<TaggingPipeline>,
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
        // TODO: Integrate tagging once we have access to ConversationManager
        
        Self {
            conversation_service,
            title_generator,
            tagging_pipeline: None,
        }
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
            
            // For now, apply basic rule-based tags based on content
            let mut tags = Vec::new();
            let content_lower = updated_conversation.messages.iter()
                .map(|m| m.content.to_lowercase())
                .collect::<String>();
            
            // Simple rule-based tagging
            if content_lower.contains("rust") || content_lower.contains("cargo") {
                tags.push("rust".to_string());
            }
            if content_lower.contains("python") || content_lower.contains("pip") {
                tags.push("python".to_string());
            }
            if content_lower.contains("javascript") || content_lower.contains("npm") {
                tags.push("javascript".to_string());
            }
            if content_lower.contains("debug") || content_lower.contains("error") || content_lower.contains("fix") {
                tags.push("debugging".to_string());
            }
            if content_lower.contains("api") || content_lower.contains("endpoint") {
                tags.push("api".to_string());
            }
            if content_lower.contains("database") || content_lower.contains("sql") {
                tags.push("database".to_string());
            }
            
            if !tags.is_empty() {
                log::info!("Adding tags to conversation {}: {:?}", conversation_id, tags);
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
                log::info!("Updating conversation {} title to '{}'", conversation_id, new_title);
                conversation.title = new_title;
                self.conversation_service.update_conversation(conversation).await?;
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::conversation::types::Conversation;
    use crate::llm::client::Role;
    use chrono::Utc;

    #[tokio::test]
    async fn test_title_update_skips_custom_titles() {
        // This would require mocking the conversation service
        // For now, just verify the updater can be created
        let title_generator = TitleGenerator::new(TitleGeneratorConfig::default());
        assert_eq!(title_generator.generate_title(&[]).await.unwrap().len() > 0, true);
    }
}