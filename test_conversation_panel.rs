use sagitta_code::gui::conversation::{SimpleConversationManager, panel::ConversationPanel};
use sagitta_code::gui::chat::StreamingChatManager;
use std::sync::Arc;
use tempfile::TempDir;

fn main() {
    // Initialize logger
    env_logger::init();

    // Create a temporary directory for testing
    let temp_dir = TempDir::new().unwrap();
    let storage_path = temp_dir.path().to_path_buf();
    
    println!("Using storage path: {:?}", storage_path);
    
    // Create chat manager
    let chat_manager = Arc::new(StreamingChatManager::new());
    
    // Create simple conversation manager
    let mut manager = SimpleConversationManager::new(storage_path, chat_manager).unwrap();
    
    // Create a few conversations
    let id1 = manager.create_conversation("Test Conversation 1".to_string()).unwrap();
    println!("Created conversation 1: {}", id1);
    
    let id2 = manager.create_conversation("Test Conversation 2".to_string()).unwrap();
    println!("Created conversation 2: {}", id2);
    
    // List conversations
    let conversations = manager.list_conversations().unwrap();
    println!("Found {} conversations:", conversations.len());
    for (id, title, last_active) in conversations {
        println!("  {} - {} ({})", id, title, last_active);
    }
}