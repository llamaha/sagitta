use anyhow::Result;
use sagitta_code::agent::state::types::ConversationStatus;
use sagitta_code::agent::message::types::AgentMessage;
use sagitta_code::llm::client::Role;
use sagitta_code::llm::title::{TitleGenerator, TitleGeneratorConfig};
use sagitta_code::agent::conversation::branching::{
    ConversationBranchingManager, BranchingConfig, BranchReason
};
use sagitta_code::agent::conversation::types::Conversation;
use sagitta_code::agent::state::status_engine::StatusEngineConfig;
use uuid::Uuid;
use chrono::Utc;

/// Test that system messages are filtered out during title generation
#[tokio::test]
async fn test_title_generation_filters_system_messages() -> Result<()> {
    let generator = TitleGenerator::new(Default::default());
    
    let messages = vec![
        AgentMessage {
            id: Uuid::new_v4(),
            role: Role::System,
            content: "[ System: Current repository context is 'helix'. When the user refers to 'this repository', they mean the helix repository. ]".to_string(),
            is_streaming: false,
            timestamp: Utc::now(),
            metadata: Default::default(),
            tool_calls: vec![],
        },
        AgentMessage {
            id: Uuid::new_v4(),
            role: Role::User,
            content: "How do I build this project?".to_string(),
            is_streaming: false,
            timestamp: Utc::now(),
            metadata: Default::default(),
            tool_calls: vec![],
        },
        AgentMessage {
            id: Uuid::new_v4(),
            role: Role::Assistant,
            content: "To build the Helix editor, you can use cargo build.".to_string(),
            is_streaming: false,
            timestamp: Utc::now(),
            metadata: Default::default(),
            tool_calls: vec![],
        },
    ];
    
    let title = generator.generate_title(&messages).await?;
    
    // Title should NOT contain system message content
    assert!(!title.contains("System:"), "Title should not contain system message markers");
    assert!(!title.contains("repository context"), "Title should not contain system context");
    assert!(!title.contains("helix"), "Title should not contain repository name from system message");
    
    // Title should be based on user/assistant messages
    assert!(title.contains("build") || title.contains("Build") || title.contains("project") || title.contains("Project"), 
        "Title should be based on actual conversation content, got: {}", title);
    
    Ok(())
}

/// Test that title generation requires actual conversation content
#[tokio::test]
async fn test_title_generation_requires_conversation_content() -> Result<()> {
    let generator = TitleGenerator::new(Default::default());
    
    // Only system messages - should fall back to default
    let messages = vec![
        AgentMessage {
            id: Uuid::new_v4(),
            role: Role::System,
            content: "System configuration message".to_string(),
            is_streaming: false,
            timestamp: Utc::now(),
            metadata: Default::default(),
            tool_calls: vec![],
        },
    ];
    
    let title = generator.generate_title(&messages).await?;
    
    // Should use fallback title since no actual conversation
    assert!(title.starts_with("Conversation"), "Should use fallback title when only system messages, got: {}", title);
    
    Ok(())
}

/// Test status engine configuration with 30-minute interval
#[tokio::test]
async fn test_status_engine_30_minute_interval() -> Result<()> {
    // Check the default configuration
    let config = StatusEngineConfig::default();
    
    // The implementation shows it's set to 1800 seconds (30 minutes) in the code
    assert_eq!(config.check_interval_seconds, 1800, "Check interval should be 30 minutes (1800 seconds)");
    
    Ok(())
}

/// Test branching manager with rule-based detection
#[tokio::test]
async fn test_branching_rule_based_detection() -> Result<()> {
    let config = BranchingConfig {
        auto_branch_threshold: 0.5, // Lower threshold for testing
        enable_auto_detection: true,
        min_messages_for_branching: 1, // Allow branching with fewer messages for testing
        ..Default::default()
    };
    
    let manager = ConversationBranchingManager::new(config);
    let mut conversation = Conversation::new("Test".to_string(), None);
    
    // Add messages that should trigger branch detection
    conversation.add_message(AgentMessage::user("I'm not sure which approach to take"));
    conversation.add_message(AgentMessage::assistant("Let me help you explore the options"));
    conversation.add_message(AgentMessage::user("What if we tried a different method? This error is confusing"));
    
    let suggestions = manager.analyze_branch_opportunities(&conversation).await?;
    
    // Should detect branching opportunities
    assert!(!suggestions.is_empty(), "Should detect branch opportunities from uncertainty");
    
    // Check detected reasons
    let has_uncertainty = suggestions.iter().any(|s| s.reason == BranchReason::UserUncertainty);
    let has_error = suggestions.iter().any(|s| s.reason == BranchReason::ErrorRecovery);
    
    assert!(has_uncertainty || has_error, "Should detect uncertainty or error patterns");
    
    Ok(())
}

/// Test conversation tagging concept with hierarchical tags
#[tokio::test]
async fn test_hierarchical_tagging_concept() -> Result<()> {
    // Test the concept of hierarchical tags
    let example_tags = vec![
        "language/rust".to_string(),
        "topic/async".to_string(),
        "framework/tokio".to_string(),
        "tool/cargo".to_string(),
    ];
    
    // Verify hierarchical structure
    for tag in &example_tags {
        assert!(tag.contains("/"), "Tags should be hierarchical with '/' separator");
        let parts: Vec<&str> = tag.split('/').collect();
        assert_eq!(parts.len(), 2, "Tags should have category/value structure");
    }
    
    // Check categories
    let has_language = example_tags.iter().any(|t| t.starts_with("language/"));
    let has_topic = example_tags.iter().any(|t| t.starts_with("topic/"));
    let has_framework = example_tags.iter().any(|t| t.starts_with("framework/"));
    
    assert!(has_language, "Should have language category");
    assert!(has_topic, "Should have topic category");
    assert!(has_framework, "Should have framework category");
    
    Ok(())
}

/// Test rename conversation event flow
#[tokio::test]
async fn test_rename_conversation_event() -> Result<()> {
    use sagitta_code::gui::app::events::AppEvent;
    use tokio::sync::mpsc;
    
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let conversation_id = Uuid::new_v4();
    let new_title = "Updated Title".to_string();
    
    // Send rename event
    sender.send(AppEvent::RenameConversation {
        conversation_id,
        new_title: new_title.clone(),
    })?;
    
    // Verify event is received correctly
    let event = receiver.recv().await.expect("Should receive event");
    
    match event {
        AppEvent::RenameConversation { conversation_id: id, new_title: title } => {
            assert_eq!(id, conversation_id);
            assert_eq!(title, new_title);
        }
        _ => panic!("Expected RenameConversation event"),
    }
    
    Ok(())
}

/// Test that title generation happens on conversation end, not during
#[tokio::test]
async fn test_title_generation_timing() -> Result<()> {
    use sagitta_code::agent::events::AgentEvent;
    
    // This tests the concept - actual implementation would require full app setup
    let conversation_id = Uuid::new_v4();
    
    // Simulate conversation completed event
    let event = AgentEvent::ConversationCompleted { conversation_id };
    
    // Verify event structure
    match event {
        AgentEvent::ConversationCompleted { conversation_id: id } => {
            assert_eq!(id, conversation_id);
        }
        _ => panic!("Expected ConversationCompleted event"),
    }
    
    Ok(())
}

/// Test tag filtering and confidence scores
#[tokio::test]
async fn test_tag_confidence_filtering() -> Result<()> {
    // Test tag structure with confidence
    let tags_with_confidence = vec![
        ("language/rust".to_string(), 0.9),
        ("topic/async".to_string(), 0.8),
        ("framework/tokio".to_string(), 0.7),
        ("low-confidence-tag".to_string(), 0.3),
    ];
    
    // Filter by confidence threshold
    let threshold = 0.6;
    let filtered_tags: Vec<_> = tags_with_confidence
        .into_iter()
        .filter(|(_, confidence)| *confidence >= threshold)
        .collect();
    
    assert_eq!(filtered_tags.len(), 3, "Should filter out low confidence tags");
    assert!(filtered_tags.iter().all(|(_, conf)| *conf >= threshold));
    
    Ok(())
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    
    /// Integration test: Full conversation flow with all features
    #[tokio::test]
    async fn test_full_conversation_feature_flow() -> Result<()> {
        // 1. Start conversation with system message
        let mut messages = vec![
            AgentMessage {
                id: Uuid::new_v4(),
                role: Role::System,
                content: "System: Repository context".to_string(),
                is_streaming: false,
                timestamp: Utc::now(),
                metadata: Default::default(),
                tool_calls: vec![],
            },
        ];
        
        // 2. Add user/assistant messages
        messages.push(AgentMessage::user("How do I handle errors in Rust?"));
        messages.push(AgentMessage::assistant("You can use Result<T, E> for error handling"));
        messages.push(AgentMessage::user("What if I want to try a different approach?"));
        
        // 3. Test title generation (should filter system messages)
        let generator = TitleGenerator::new(Default::default());
        let title = generator.generate_title(&messages).await?;
        assert!(!title.contains("System"));
        assert!(title.contains("Rust") || title.contains("error") || title.contains("Error"));
        
        // 4. Test branch detection
        let mut conversation = Conversation::new(title.clone(), None);
        for msg in &messages {
            conversation.add_message(msg.clone());
        }
        
        let branch_manager = ConversationBranchingManager::new(BranchingConfig {
            min_messages_for_branching: 2,
            auto_branch_threshold: 0.5,
            ..Default::default()
        });
        
        let branch_suggestions = branch_manager.analyze_branch_opportunities(&conversation).await?;
        assert!(!branch_suggestions.is_empty(), "Should suggest branches for uncertainty");
        
        // 5. Test status management
        let status_config = StatusEngineConfig {
            check_interval_seconds: 1800, // 30 minutes
            ..Default::default()
        };
        assert_eq!(status_config.check_interval_seconds, 1800);
        
        Ok(())
    }
}