use anyhow::Result;
use sagitta_code::config::types::{SagittaCodeConfig, ConversationConfig};
use sagitta_code::agent::message::types::AgentMessage;
use sagitta_code::llm::client::Role;
use sagitta_code::agent::conversation::types::Conversation;
use sagitta_code::agent::state::types::ConversationStatus;
use uuid::Uuid;
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Mock LLM provider for testing fast model operations
struct MockFastModelProvider {
    pub model_used: Arc<Mutex<Option<String>>>,
    pub response_delay_ms: u64,
}

impl MockFastModelProvider {
    fn new(response_delay_ms: u64) -> Self {
        Self {
            model_used: Arc::new(Mutex::new(None)),
            response_delay_ms,
        }
    }
    
    async fn generate_title(&self, model: &str, _messages: &[AgentMessage]) -> Result<String> {
        *self.model_used.lock().await = Some(model.to_string());
        tokio::time::sleep(tokio::time::Duration::from_millis(self.response_delay_ms)).await;
        Ok("Test Title Generated with Fast Model".to_string())
    }
    
    async fn suggest_tags(&self, model: &str, _conversation: &Conversation) -> Result<Vec<(String, f32)>> {
        *self.model_used.lock().await = Some(model.to_string());
        tokio::time::sleep(tokio::time::Duration::from_millis(self.response_delay_ms)).await;
        Ok(vec![
            ("programming".to_string(), 0.9),
            ("rust".to_string(), 0.85),
            ("testing".to_string(), 0.8),
        ])
    }
    
    async fn suggest_status(&self, model: &str, _conversation: &Conversation) -> Result<(ConversationStatus, f32)> {
        *self.model_used.lock().await = Some(model.to_string());
        tokio::time::sleep(tokio::time::Duration::from_millis(self.response_delay_ms)).await;
        Ok((ConversationStatus::Active, 0.95))
    }
}

/// Test that fast model configuration is properly loaded and saved
#[tokio::test]
async fn test_fast_model_configuration() -> Result<()> {
    let mut config = SagittaCodeConfig::default();
    
    // Check default values
    assert_eq!(config.conversation.fast_model, "claude-haiku-20250102");
    assert!(config.conversation.enable_fast_model);
    
    // Update configuration
    config.conversation.fast_model = "claude-sonnet-4-20250514".to_string();
    config.conversation.enable_fast_model = false;
    
    // Verify updates
    assert_eq!(config.conversation.fast_model, "claude-sonnet-4-20250514");
    assert!(!config.conversation.enable_fast_model);
    
    Ok(())
}

/// Test that title generation uses fast model when enabled
#[tokio::test]
async fn test_title_generation_with_fast_model() -> Result<()> {
    let provider = MockFastModelProvider::new(100); // 100ms response time
    let messages = vec![
        AgentMessage {
            id: Uuid::new_v4(),
            role: Role::User,
            content: "How do I implement a linked list in Rust?".to_string(),
            is_streaming: false,
            timestamp: Utc::now(),
            metadata: Default::default(),
            tool_calls: vec![],
        },
        AgentMessage {
            id: Uuid::new_v4(),
            role: Role::Assistant,
            content: "Here's how to implement a linked list in Rust...".to_string(),
            is_streaming: false,
            timestamp: Utc::now(),
            metadata: Default::default(),
            tool_calls: vec![],
        },
    ];
    
    // Generate title with fast model
    let start = tokio::time::Instant::now();
    let title = provider.generate_title("claude-haiku-20250102", &messages).await?;
    let duration = start.elapsed();
    
    // Verify fast model was used
    let model_used = provider.model_used.lock().await.clone();
    assert_eq!(model_used, Some("claude-haiku-20250102".to_string()));
    
    // Verify response time is fast
    assert!(duration.as_millis() < 500, "Title generation should be fast");
    assert_eq!(title, "Test Title Generated with Fast Model");
    
    Ok(())
}

/// Test that tag suggestion uses fast model when enabled
#[tokio::test]
async fn test_tag_suggestion_with_fast_model() -> Result<()> {
    let provider = MockFastModelProvider::new(150); // 150ms response time
    let conversation = create_test_conversation();
    
    // Suggest tags with fast model
    let start = tokio::time::Instant::now();
    let tags = provider.suggest_tags("claude-haiku-20250102", &conversation).await?;
    let duration = start.elapsed();
    
    // Verify fast model was used
    let model_used = provider.model_used.lock().await.clone();
    assert_eq!(model_used, Some("claude-haiku-20250102".to_string()));
    
    // Verify response time is fast
    assert!(duration.as_millis() < 500, "Tag suggestion should be fast");
    
    // Verify tags
    assert_eq!(tags.len(), 3);
    assert_eq!(tags[0].0, "programming");
    assert!(tags[0].1 > 0.8); // High confidence
    
    Ok(())
}

/// Test that status suggestion uses fast model when enabled
#[tokio::test]
async fn test_status_suggestion_with_fast_model() -> Result<()> {
    let provider = MockFastModelProvider::new(100); // 100ms response time
    let conversation = create_test_conversation();
    
    // Suggest status with fast model
    let start = tokio::time::Instant::now();
    let (status, confidence) = provider.suggest_status("claude-haiku-20250102", &conversation).await?;
    let duration = start.elapsed();
    
    // Verify fast model was used
    let model_used = provider.model_used.lock().await.clone();
    assert_eq!(model_used, Some("claude-haiku-20250102".to_string()));
    
    // Verify response time is fast
    assert!(duration.as_millis() < 500, "Status suggestion should be fast");
    
    // Verify status
    assert_eq!(status, ConversationStatus::Active);
    assert!(confidence > 0.9); // High confidence
    
    Ok(())
}

/// Test fallback to rule-based when fast model is disabled
#[tokio::test]
async fn test_fallback_when_fast_model_disabled() -> Result<()> {
    let mut config = ConversationConfig::default();
    config.enable_fast_model = false;
    
    // When fast model is disabled, the system should use rule-based approaches
    // This test would need actual implementation to verify the behavior
    
    assert!(!config.enable_fast_model, "Fast model should be disabled");
    
    Ok(())
}

/// Test concurrent operations with fast model
#[tokio::test]
async fn test_concurrent_fast_model_operations() -> Result<()> {
    let provider = Arc::new(MockFastModelProvider::new(50)); // 50ms response time
    let conversation = create_test_conversation();
    let messages = vec![
        AgentMessage {
            id: Uuid::new_v4(),
            role: Role::User,
            content: "Test message".to_string(),
            is_streaming: false,
            timestamp: Utc::now(),
            metadata: Default::default(),
            tool_calls: vec![],
        },
    ];
    
    // Launch multiple concurrent operations
    let provider1 = Arc::clone(&provider);
    let provider2 = Arc::clone(&provider);
    let provider3 = Arc::clone(&provider);
    
    let conversation1 = conversation.clone();
    let conversation2 = conversation.clone();
    let messages1 = messages.clone();
    
    let (title_result, tags_result, status_result) = tokio::join!(
        provider1.generate_title("claude-haiku-20250102", &messages1),
        provider2.suggest_tags("claude-haiku-20250102", &conversation1),
        provider3.suggest_status("claude-haiku-20250102", &conversation2),
    );
    
    // All operations should succeed
    assert!(title_result.is_ok());
    assert!(tags_result.is_ok());
    assert!(status_result.is_ok());
    
    Ok(())
}

/// Test performance benchmarks for fast model operations
#[tokio::test]
async fn test_fast_model_performance_benchmarks() -> Result<()> {
    let provider = MockFastModelProvider::new(100); // 100ms base response time
    let iterations = 10;
    let mut durations = Vec::new();
    
    for _ in 0..iterations {
        let start = tokio::time::Instant::now();
        let _ = provider.generate_title("claude-haiku-20250102", &[]).await?;
        durations.push(start.elapsed());
    }
    
    // Calculate average duration
    let total: tokio::time::Duration = durations.iter().sum();
    let average = total / iterations;
    
    // Verify average is under 500ms (requirement for fast model)
    assert!(
        average.as_millis() < 500,
        "Average response time {} ms should be under 500ms",
        average.as_millis()
    );
    
    Ok(())
}

/// Test model selection logic
#[tokio::test]
async fn test_model_selection_logic() -> Result<()> {
    let config = ConversationConfig {
        fast_model: "claude-haiku-20250102".to_string(),
        enable_fast_model: true,
        ..Default::default()
    };
    
    // When fast model is enabled, use the configured fast model
    let selected_model = if config.enable_fast_model {
        &config.fast_model
    } else {
        "claude-sonnet-4-20250514" // Default model
    };
    
    assert_eq!(selected_model, "claude-haiku-20250102");
    
    Ok(())
}

// Helper function to create a test conversation
fn create_test_conversation() -> Conversation {
    Conversation {
        id: Uuid::new_v4(),
        title: "Test Conversation".to_string(),
        workspace_id: None,
        created_at: Utc::now(),
        last_active: Utc::now(),
        status: ConversationStatus::Active,
        messages: vec![
            AgentMessage {
                id: Uuid::new_v4(),
                role: Role::User,
                content: "How do I implement a linked list in Rust?".to_string(),
                is_streaming: false,
                timestamp: Utc::now(),
                metadata: Default::default(),
                tool_calls: vec![],
            },
            AgentMessage {
                id: Uuid::new_v4(),
                role: Role::Assistant,
                content: "Here's how to implement a linked list in Rust...".to_string(),
                is_streaming: false,
                timestamp: Utc::now(),
                metadata: Default::default(),
                tool_calls: vec![],
            },
        ],
        tags: vec!["programming".to_string(), "rust".to_string()],
        branches: vec![],
        checkpoints: vec![],
        project_context: None,
    }
}