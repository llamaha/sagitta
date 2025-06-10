use std::time::Instant;
use tempfile::TempDir;
use sagitta_code::agent::conversation::{
    manager::ConversationManagerImpl,
    persistence::disk::DiskConversationPersistence,
    search::text::TextConversationSearchEngine,
    types::Conversation,
};

#[tokio::test]
async fn test_phase_1_no_duplicate_managers() {
    // Test Phase 1: No duplicate conversation manager creation
    let temp_dir = TempDir::new().unwrap();
    let storage_path = temp_dir.path().to_path_buf();
    
    let start_time = Instant::now();
    
    // Create only one conversation manager (Phase 1 optimization)
    let persistence = Box::new(DiskConversationPersistence::new(storage_path).await.unwrap());
    let search_engine = Box::new(TextConversationSearchEngine::new());
    let _manager = ConversationManagerImpl::new(persistence, search_engine).await.unwrap();
    
    let elapsed = start_time.elapsed();
    println!("Phase 1: Single manager creation took: {:?}", elapsed);
    
    // Verify it's fast (should be under 1 second for empty storage)
    assert!(elapsed.as_secs() < 1, "Phase 1 should complete quickly with no conversations");
}

#[tokio::test]
async fn test_phase_2_lazy_loading() {
    // Test Phase 2: Lazy conversation loading
    let temp_dir = TempDir::new().unwrap();
    let storage_path = temp_dir.path().to_path_buf();
    
    // First, create some test conversations
    {
        let persistence = Box::new(DiskConversationPersistence::new(storage_path.clone()).await.unwrap());
        let search_engine = Box::new(TextConversationSearchEngine::new());
        let manager = ConversationManagerImpl::new(persistence, search_engine).await.unwrap();
        
        // Create several conversations to test with
        for i in 0..10 {
            let title = format!("Test Conversation {}", i);
            manager.create_conversation(title, None).await.unwrap();
        }
    }
    
    // Now test lazy loading startup time
    let start_time = Instant::now();
    
    // Phase 2: Should only load index, not full conversations
    let persistence = Box::new(DiskConversationPersistence::new(storage_path).await.unwrap());
    let search_engine = Box::new(TextConversationSearchEngine::new());
    let mut manager = ConversationManagerImpl::new(persistence, search_engine).await.unwrap();
    
    // Force loading (this should be fast in Phase 2 - only loads IDs)
    manager.load_all_conversations().await.unwrap();
    
    let lazy_loading_time = start_time.elapsed();
    println!("Phase 2: Lazy loading startup took: {:?}", lazy_loading_time);
    
    // Test that we can list conversations quickly (from index)
    let list_start = Instant::now();
    let summaries = manager.list_conversations(None).await.unwrap();
    let list_time = list_start.elapsed();
    
    println!("Phase 2: Listing {} conversations took: {:?}", summaries.len(), list_time);
    assert_eq!(summaries.len(), 10);
    
    // Test that individual conversation loading works (lazy loading)
    let get_start = Instant::now();
    let conversation = manager.get_conversation(summaries[0].id).await.unwrap();
    let get_time = get_start.elapsed();
    
    println!("Phase 2: Loading individual conversation took: {:?}", get_time);
    assert!(conversation.is_some());
    
    // Verify lazy loading is faster than loading all conversations upfront
    // The lazy loading startup should be much faster since it doesn't load full conversations
    assert!(lazy_loading_time.as_millis() < 500, "Phase 2 lazy loading should be very fast");
    assert!(list_time.as_millis() < 100, "Phase 2 conversation listing should be very fast");
} 