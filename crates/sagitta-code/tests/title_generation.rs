use anyhow::Result;
use sagitta_code::agent::conversation::manager::{ConversationManager, ConversationManagerImpl};
use sagitta_code::agent::conversation::persistence::ConversationPersistence;
use sagitta_code::agent::conversation::search::ConversationSearchEngine;
use sagitta_code::agent::conversation::types::{Conversation, ConversationQuery, ConversationSearchResult, ConversationSummary};
use sagitta_code::agent::message::types::AgentMessage;
use sagitta_code::llm::title::TitleGenerator;
use sagitta_code::llm::client::Role;
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;
use chrono::Utc;

// Mock implementations for testing
#[derive(Default)]
struct MockPersistence;

#[async_trait]
impl ConversationPersistence for MockPersistence {
    async fn save_conversation(&self, _conversation: &Conversation) -> Result<()> {
        Ok(())
    }
    
    async fn load_conversation(&self, _id: Uuid) -> Result<Option<Conversation>> {
        Ok(None)
    }
    
    async fn delete_conversation(&self, _id: Uuid) -> Result<()> {
        Ok(())
    }
    
    async fn list_conversation_ids(&self) -> Result<Vec<Uuid>> {
        Ok(Vec::new())
    }
    
    async fn list_conversation_summaries(&self, _workspace_id: Option<Uuid>) -> Result<Vec<ConversationSummary>> {
        Ok(Vec::new())
    }
    
    async fn archive_conversation(&self, _id: Uuid) -> Result<()> {
        Ok(())
    }
    
    async fn list_archived_conversation_ids(&self) -> Result<Vec<Uuid>> {
        Ok(Vec::new())
    }
    
    async fn restore_conversation(&self, _id: Uuid) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct MockSearchEngine;

#[async_trait]
impl ConversationSearchEngine for MockSearchEngine {
    async fn index_conversation(&self, _conversation: &Conversation) -> Result<()> {
        Ok(())
    }
    
    async fn remove_conversation(&self, _id: Uuid) -> Result<()> {
        Ok(())
    }
    
    async fn search(&self, _query: &ConversationQuery) -> Result<Vec<ConversationSearchResult>> {
        Ok(Vec::new())
    }
    
    async fn clear_index(&self) -> Result<()> {
        Ok(())
    }
    
    async fn rebuild_index(&self, _conversations: &[Conversation]) -> Result<()> {
        Ok(())
    }
}

/// Test that conversations with empty titles get auto-generated titles after 3 messages
#[tokio::test]
async fn test_auto_title_generation_after_messages() -> Result<()> {
    let manager = create_test_manager().await?;
    let title_generator = create_test_title_generator().await?;
    
    // Create conversation with empty title
    let conversation_id = manager.create_conversation("".to_string(), None).await?;
    
    // Add 3 messages
    let mut conversation = manager.get_conversation(conversation_id).await?
        .expect("Conversation should exist");
    
    conversation.add_message(AgentMessage {
        id: Uuid::new_v4(),
        role: Role::User,
        content: "I need help with Rust error handling".to_string(),
        is_streaming: false,
        timestamp: Utc::now(),
        metadata: Default::default(),
        tool_calls: vec![],
    });
    
    conversation.add_message(AgentMessage {
        id: Uuid::new_v4(),
        role: Role::Assistant,
        content: "I can help you with Rust error handling. What specific issue are you facing?".to_string(),
        is_streaming: false,
        timestamp: Utc::now(),
        metadata: Default::default(),
        tool_calls: vec![],
    });
    
    conversation.add_message(AgentMessage {
        id: Uuid::new_v4(),
        role: Role::User,
        content: "I'm getting panic errors when unwrapping Option values".to_string(),
        is_streaming: false,
        timestamp: Utc::now(),
        metadata: Default::default(),
        tool_calls: vec![],
    });
    
    manager.update_conversation(conversation).await?;
    
    // TODO: This should trigger title generation
    // For now, this test will fail until we implement the title generator
    
    // Generate title
    let updated_conversation = manager.get_conversation(conversation_id).await?
        .expect("Conversation should exist");
    
    let generated_title = title_generator.generate_title(&updated_conversation.messages).await?;
    
    // Title should be descriptive and under 50 characters
    assert!(!generated_title.is_empty(), "Generated title should not be empty");
    assert!(generated_title.len() <= 50, "Generated title should be under 50 characters");
    assert!(generated_title.to_lowercase().contains("rust") || 
            generated_title.to_lowercase().contains("error"), 
            "Title should be relevant to conversation content");
    
    Ok(())
}

/// Test that title generation falls back gracefully when LLM fails
#[tokio::test]
async fn test_title_generation_fallback() -> Result<()> {
    let manager = create_test_manager().await?;
    let title_generator = create_failing_title_generator().await?;
    
    // Create conversation with empty title
    let conversation_id = manager.create_conversation("".to_string(), None).await?;
    
    // Add messages
    let mut conversation = manager.get_conversation(conversation_id).await?
        .expect("Conversation should exist");
    
    conversation.add_message(AgentMessage {
        id: Uuid::new_v4(),
        role: Role::User,
        content: "Test message".to_string(),
        is_streaming: false,
        timestamp: Utc::now(),
        metadata: Default::default(),
        tool_calls: vec![],
    });
    
    manager.update_conversation(conversation).await?;
    
    // Generate title with failing generator
    let updated_conversation = manager.get_conversation(conversation_id).await?
        .expect("Conversation should exist");
    
    let fallback_title = title_generator.generate_title(&updated_conversation.messages).await?;
    
    // Should fall back to timestamp-based title
    assert!(fallback_title.starts_with("Conversation"), "Should use fallback title format");
    
    Ok(())
}

/// Test that title generation respects character limits
#[tokio::test]
async fn test_title_generation_character_limit() -> Result<()> {
    let title_generator = create_test_title_generator().await?;
    
    // Create very long messages
    let messages = vec![
        AgentMessage {
            id: Uuid::new_v4(),
            role: Role::User,
            content: "This is a very long message that contains a lot of information about a complex topic that should result in a title that needs to be truncated because it would otherwise be way too long for a conversation title and would not fit in the UI properly".to_string(),
            is_streaming: false,
            timestamp: Utc::now(),
            metadata: Default::default(),
            tool_calls: vec![],
        }
    ];
    
    let generated_title = title_generator.generate_title(&messages).await?;
    
    // Title should be under 50 characters
    assert!(generated_title.len() <= 50, "Generated title should be under 50 characters, got: {generated_title}");
    
    Ok(())
}

/// Test that title generation works with different conversation types
#[tokio::test]
async fn test_title_generation_different_types() -> Result<()> {
    let title_generator = create_test_title_generator().await?;
    
    // Test coding conversation
    let coding_messages = vec![
        AgentMessage {
            id: Uuid::new_v4(),
            role: Role::User,
            content: "How do I implement a binary search tree in Python?".to_string(),
            is_streaming: false,
            timestamp: Utc::now(),
            metadata: Default::default(),
            tool_calls: vec![],
        }
    ];
    
    let coding_title = title_generator.generate_title(&coding_messages).await?;
    assert!(!coding_title.is_empty());
    
    // Test general conversation
    let general_messages = vec![
        AgentMessage {
            id: Uuid::new_v4(),
            role: Role::User,
            content: "What's the weather like today?".to_string(),
            is_streaming: false,
            timestamp: Utc::now(),
            metadata: Default::default(),
            tool_calls: vec![],
        }
    ];
    
    let general_title = title_generator.generate_title(&general_messages).await?;
    assert!(!general_title.is_empty());
    
    // Titles should be different for different conversation types
    assert_ne!(coding_title, general_title, "Different conversation types should generate different titles");
    
    Ok(())
}

/// Test that empty or very short conversations get appropriate titles
#[tokio::test]
async fn test_title_generation_edge_cases() -> Result<()> {
    let title_generator = create_test_title_generator().await?;
    
    // Test empty messages
    let empty_messages = vec![];
    let empty_title = title_generator.generate_title(&empty_messages).await?;
    assert!(empty_title.starts_with("Conversation"), "Empty conversation should get fallback title");
    
    // Test very short message
    let short_messages = vec![
        AgentMessage {
            id: Uuid::new_v4(),
            role: Role::User,
            content: "Hi".to_string(),
            is_streaming: false,
            timestamp: Utc::now(),
            metadata: Default::default(),
            tool_calls: vec![],
        }
    ];
    
    let short_title = title_generator.generate_title(&short_messages).await?;
    assert!(!short_title.is_empty());
    assert!(short_title.len() <= 50);
    
    Ok(())
}

/// Helper function to create a test conversation manager
async fn create_test_manager() -> Result<Arc<dyn ConversationManager>> {
    let manager = ConversationManagerImpl::new(
        Box::new(MockPersistence),
        Box::new(MockSearchEngine),
    ).await?;
    Ok(Arc::new(manager) as Arc<dyn ConversationManager>)
}

/// Helper function to create a test title generator
async fn create_test_title_generator() -> Result<TitleGenerator> {
    use sagitta_code::llm::title::TitleGeneratorConfig;
    Ok(TitleGenerator::new(TitleGeneratorConfig::default()))
}

/// Helper function to create a failing title generator for testing fallbacks
async fn create_failing_title_generator() -> Result<TitleGenerator> {
    Ok(TitleGenerator::failing())
} 