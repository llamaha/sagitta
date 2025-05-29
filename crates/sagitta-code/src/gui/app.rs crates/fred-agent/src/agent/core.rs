use crate::agent::message::types::ToolCategory;
use crate::agent::tool::ToolRegistry;
use crate::agent::types::AgentMessage;
use crate::config::FredAgentConfig;
use crate::gemini::client::GeminiClient;
use crate::gui::app::FredApp;
use crate::gui::chat::ChatMessage;
use crate::gui::chat::MessageAuthor;
use crate::gui::tools::ValidateTool;
use crate::repo::RepoManager;
use anyhow::anyhow;
use chrono::Utc;
use eframe::egui;
use egui::Context;
use log::info;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

impl FredApp {
    async fn initialize_agent(&mut self, ctx: &Context) -> anyhow::Result<()> {
        let repo_manager = Arc::new(RepoManager::new(self.config.as_ref().clone()));
        let tool_registry = Arc::new(ToolRegistry::new());

        tool_registry.register(Arc::new(ValidateTool::new(repo_manager))).await?;

        // Create Gemini Client for the Agent
        let gemini_client = match GeminiClient::new(&self.config) {
            Ok(client) => Arc::new(client),
            Err(e) => {
                log::error!("Failed to create GeminiClient: {}. Agent will not be initialized.", e);
                // Optionally, push an immediate error message to chat_messages here
                // self.chat_messages.push(ChatMessage::new(
                //     MessageAuthor::System,
                //     format!("Critical Error: Failed to create LLM client: {}. Agent disabled.", e),
                // ));
                return Err(anyhow!("Failed to create GeminiClient for Agent: {}", e)); // Return error to stop initialization
            }
        };

        let agent_result = Agent::new(self.config.as_ref().clone(), tool_registry.clone(), gemini_client).await; // Added gemini_client
        match agent_result {
            Ok(agent) => {
                self.agent = Some(agent);
                info!("Agent initialized successfully");
                Ok(())
            }
            Err(e) => {
                log::error!("Failed to initialize Agent: {}. Agent will not be initialized.", e);
                // Optionally, push an immediate error message to chat_messages here
                // self.chat_messages.push(ChatMessage::new(
                //     MessageAuthor::System,
                //     format!("Critical Error: Failed to initialize Agent: {}. Agent disabled.", e),
                // ));
                Err(anyhow!("Failed to initialize Agent: {}", e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*; // Keep this as the first line in the module
    use mockall::predicate; // Added for predicate::always, etc.
    use crate::agent::message::types::ToolCategory; // For ToolCategory::default()
    use std::collections::HashMap; // For HashMap::new()

    // Mock LLM client for testing
    mock! {
        pub LlmClient {}

        impl LlmClient for LlmClient {
            fn generate(&self, messages: &[Message], functions: &[Function]) -> Result<LlmResponse, LlmError>;
 