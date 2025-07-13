// Tests for agent cancellation functionality

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::agent::events::AgentEvent;
    use tokio::time::{sleep, Duration};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use async_trait::async_trait;
    
    // Simple mock embedding provider for tests
    #[derive(Debug)]
    struct MockEmbeddingProvider;
    
    impl sagitta_embed::provider::EmbeddingProvider for MockEmbeddingProvider {
        fn dimension(&self) -> usize {
            3
        }
        
        fn model_type(&self) -> sagitta_embed::model::EmbeddingModelType {
            sagitta_embed::model::EmbeddingModelType::Default
        }
        
        fn embed_batch(&self, texts: &[&str]) -> sagitta_embed::error::Result<Vec<Vec<f32>>> {
            Ok(texts.iter().map(|_| vec![0.1, 0.2, 0.3]).collect())
        }
    }
    
    #[tokio::test]
    async fn test_agent_cancel_stops_processing() {
        // Create a mock agent that simulates a long-running operation
        let agent = create_test_agent().await;
        let processing_started = Arc::new(AtomicBool::new(false));
        let processing_completed = Arc::new(AtomicBool::new(false));
        
        let started = processing_started.clone();
        let completed = processing_completed.clone();
        
        // Start a long-running task
        let agent_clone = agent.clone();
        let handle = tokio::spawn(async move {
            started.store(true, Ordering::SeqCst);
            
            // Simulate processing with cancellation checks
            for _ in 0..10 {
                if agent_clone.is_cancelled().await {
                    return;
                }
                sleep(Duration::from_millis(100)).await;
            }
            
            completed.store(true, Ordering::SeqCst);
        });
        
        // Wait for processing to start
        sleep(Duration::from_millis(50)).await;
        assert!(processing_started.load(Ordering::SeqCst));
        
        // Cancel the operation
        agent.cancel().await;
        
        // Wait for the task to finish
        let _ = handle.await;
        
        // Verify processing was cancelled
        assert!(!processing_completed.load(Ordering::SeqCst));
        assert!(agent.is_cancelled().await);
    }
    
    #[tokio::test]
    async fn test_cancel_clears_on_new_request() {
        let agent = create_test_agent().await;
        
        // Cancel the agent
        agent.cancel().await;
        assert!(agent.is_cancelled().await);
        
        // Start a new request (should clear cancellation)
        agent.process_message_stream("test message").await.unwrap();
        
        // Cancellation should be cleared
        assert!(!agent.is_cancelled().await);
    }
    
    #[tokio::test]
    async fn test_cancel_interrupts_llm_stream() {
        let agent = create_test_agent().await;
        
        // Subscribe to events
        let mut event_receiver = agent.subscribe();
        
        // Start streaming in background
        let agent_clone = agent.clone();
        let handle = tokio::spawn(async move {
            // This should be interrupted by cancellation
            let _ = agent_clone.process_message_stream("Generate a long response").await;
        });
        
        // Wait briefly then cancel
        sleep(Duration::from_millis(100)).await;
        agent.cancel().await;
        
        // Check for cancellation event
        let mut received_cancel_event = false;
        while let Ok(event) = event_receiver.try_recv() {
            if matches!(event, AgentEvent::Cancelled) {
                received_cancel_event = true;
                break;
            }
        }
        
        // Wait for task to complete
        let _ = handle.await;
        
        assert!(received_cancel_event);
    }
    
    #[tokio::test]
    async fn test_cancel_is_idempotent() {
        let agent = create_test_agent().await;
        
        // Cancel multiple times
        agent.cancel().await;
        agent.cancel().await;
        agent.cancel().await;
        
        // Should still be cancelled only once
        assert!(agent.is_cancelled().await);
    }
    
    #[tokio::test]
    async fn test_cancel_during_tool_execution() {
        let agent = create_test_agent().await;
        
        // Simulate tool execution
        let tool_started = Arc::new(AtomicBool::new(false));
        let tool_completed = Arc::new(AtomicBool::new(false));
        
        let started = tool_started.clone();
        let completed = tool_completed.clone();
        
        let agent_clone = agent.clone();
        let handle = tokio::spawn(async move {
            started.store(true, Ordering::SeqCst);
            
            // Simulate tool execution with cancellation check
            for _ in 0..5 {
                if agent_clone.is_cancelled().await {
                    return;
                }
                sleep(Duration::from_millis(100)).await;
            }
            
            completed.store(true, Ordering::SeqCst);
        });
        
        // Wait for tool to start
        sleep(Duration::from_millis(50)).await;
        assert!(tool_started.load(Ordering::SeqCst));
        
        // Cancel during tool execution
        agent.cancel().await;
        
        // Wait for completion
        let _ = handle.await;
        
        // Tool should not have completed
        assert!(!tool_completed.load(Ordering::SeqCst));
    }
    
    // Helper function to create a test agent
    async fn create_test_agent() -> Agent {
        use crate::config::types::SagittaCodeConfig;
        use crate::llm::test_client::TestLlmClient;
        use crate::agent::conversation::persistence::MockConversationPersistence;
        use crate::agent::conversation::search::MockConversationSearchEngine;
        
        let config = SagittaCodeConfig::default();
        let llm_client = Arc::new(TestLlmClient::new());
        let embedding_provider = Arc::new(MockEmbeddingProvider);
        
        // Set up mock persistence with expected calls
        let mut persistence = MockConversationPersistence::new();
        persistence.expect_list_conversation_ids()
            .returning(|| Ok(vec![]));
        persistence.expect_list_conversation_summaries()
            .returning(|_| Ok(vec![]));
        persistence.expect_load_conversation()
            .returning(|_| Ok(None));
        persistence.expect_save_conversation()
            .returning(|_| Ok(()));
        
        let mut search_engine = MockConversationSearchEngine::new();
        search_engine.expect_index_conversation()
            .returning(|_| Ok(()));
        search_engine.expect_search()
            .returning(|_| Ok(vec![]));
        
        Agent::new(
            config,
            None,
            embedding_provider,
            Box::new(persistence),
            Box::new(search_engine),
            llm_client,
        ).await.unwrap()
    }
}