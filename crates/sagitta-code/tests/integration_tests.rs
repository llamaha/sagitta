use sagitta_code::config::{FredAgentConfig, load_merged_config, load_config_from_path};
use sagitta_code::gui::app::FredAgentApp;
use sagitta_code::gui::repository::manager::RepositoryManager;
use sagitta_code::gui::chat::StreamingChatManager;
use std::sync::Arc;
use tokio::sync::Mutex;
use tempfile::TempDir;
use tokio::fs;
use std::path::PathBuf;
use sagitta_code::llm::gemini::streaming::GeminiStream;
use sagitta_code::llm::gemini::api::GeminiResponse;
use futures_util::StreamExt;
use egui;
use sagitta_code::agent::Agent;
use sagitta_code::tools::registry::ToolRegistry;
use sagitta_code::agent::conversation::persistence::disk::DiskConversationPersistence;
use sagitta_code::agent::conversation::persistence::ConversationPersistence;
use sagitta_code::agent::conversation::search::text::TextConversationSearchEngine;
use sagitta_search::embedding::provider::onnx::{ThreadSafeOnnxProvider, OnnxEmbeddingModel};
use sagitta_code::llm::gemini::client::GeminiClient;
use std::path::Path;
use uuid::Uuid;

/// Test that demonstrates Fred overwriting messages instead of creating new ones
#[tokio::test]
async fn test_fred_message_overwriting_issue() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("sagitta_code_config.json");
    
    // Create a test config
    let test_config = FredAgentConfig::default();
    let config_json = serde_json::to_string_pretty(&test_config).unwrap();
    fs::write(&config_path, config_json).await.unwrap();
    
    // Create app
    let app_core_config = sagitta_search::config::AppConfig::default();
    let repo_manager = Arc::new(Mutex::new(
        RepositoryManager::new(Arc::new(Mutex::new(app_core_config.clone())))
    ));
    let mut app = FredAgentApp::new(repo_manager, test_config.clone(), app_core_config);
    
    // Simulate the exact behavior that causes message overwriting
    let chat_manager = Arc::new(StreamingChatManager::new());
    
    // User sends first message
    let user_msg_1 = "What is Rust?";
    chat_manager.add_user_message(user_msg_1.to_string());
    
    // Fred starts responding to first message
    let fred_response_1_id = chat_manager.start_agent_response();
    chat_manager.append_content(&fred_response_1_id, "Rust is a systems programming language".to_string());
    chat_manager.finish_streaming(&fred_response_1_id);
    
    // User sends second message
    let user_msg_2 = "Can you give me an example?";
    chat_manager.add_user_message(user_msg_2.to_string());
    
    // Fred starts responding to second message - THIS SHOULD CREATE A NEW MESSAGE
    let fred_response_2_id = chat_manager.start_agent_response();
    chat_manager.append_content(&fred_response_2_id, "Here's a simple example:".to_string());
    chat_manager.finish_streaming(&fred_response_2_id);
    
    // Get all messages
    let messages = chat_manager.get_all_messages();
    
    // CRITICAL TEST: Fred should have created 2 separate messages with different IDs and timestamps
    let fred_messages: Vec<_> = messages.iter()
        .filter(|m| m.author == sagitta_code::gui::chat::view::MessageAuthor::Agent)
        .collect();
    
    assert_eq!(fred_messages.len(), 2, "Fred should create 2 separate messages, not overwrite the first one");
    assert_ne!(fred_messages[0].id, fred_messages[1].id, "Fred's messages should have different IDs");
    assert!(fred_messages[1].timestamp > fred_messages[0].timestamp, "Second message should have later timestamp");
    
    // Content should be different
    assert_eq!(fred_messages[0].content, "Rust is a systems programming language");
    assert_eq!(fred_messages[1].content, "Here's a simple example:");
    
    // Total messages should be 4: user1, fred1, user2, fred2
    assert_eq!(messages.len(), 4, "Should have 4 total messages: user1, fred1, user2, fred2");
}

/// Test that demonstrates settings not loading from config files
#[tokio::test]
async fn test_settings_not_loading_from_config() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create sagitta_code_config.json with specific values
    let app_config_path = temp_dir.path().join("sagitta_code_config.json");
    let app_config_content = r#"{
  "gemini": {
    "api_key": "test-api-key-123",
    "model": "test-model-name",
    "max_history_size": 50
  },
  "ui": {
    "theme": "light",
    "dark_mode": false,
    "window_width": 1200,
    "window_height": 900
  }
}"#;
    fs::write(&app_config_path, app_config_content).await.unwrap();
    
    // Load the config directly from the file
    let loaded_config = load_config_from_path(&app_config_path).expect("Should load config from test file");
    
    // CRITICAL TEST: Settings should be loaded from the files
    assert_eq!(loaded_config.gemini.api_key, Some("test-api-key-123".to_string()));
    assert_eq!(loaded_config.gemini.model, "test-model-name");
    assert_eq!(loaded_config.ui.theme, "light");
    assert_eq!(loaded_config.ui.window_width, 1200);
    
    // Create app with loaded config
    let app_core_config_for_loaded = sagitta_search::config::AppConfig::default();
    let repo_manager = Arc::new(Mutex::new(
        RepositoryManager::new(Arc::new(Mutex::new(app_core_config_for_loaded.clone())))
    ));
    let mut app = FredAgentApp::new(repo_manager, loaded_config.clone(), app_core_config_for_loaded);
    
    // CRITICAL TEST: App should be created with the loaded config values
    assert_eq!(app.settings_panel.gemini_api_key, "test-api-key-123");
    assert_eq!(app.settings_panel.gemini_model, "test-model-name");
    
    // The theme should be set correctly in the app constructor
    let expected_theme_name = "Light";
    assert_eq!(app.current_theme.name(), expected_theme_name);
}

/// Test that demonstrates the streaming message issue in the actual app context
#[tokio::test]
async fn test_app_streaming_message_behavior() {
    let temp_dir = TempDir::new().unwrap();
    let test_config = FredAgentConfig::default();
    
    let app_core_config_stream_test = sagitta_search::config::AppConfig::default();
    let repo_manager = Arc::new(Mutex::new(
        RepositoryManager::new(Arc::new(Mutex::new(app_core_config_stream_test.clone())))
    ));
    let mut app = FredAgentApp::new(repo_manager, test_config.clone(), app_core_config_stream_test);
    let ctx = egui::Context::default();
    
    // Simulate the exact sequence that causes message overwriting
    
    // 1. User sends first message
    app.chat_input_buffer = "What is Rust?".to_string();
    app.chat_on_submit = true;
    
    // Simulate processing the first message (without actual agent)
    let user_msg_1 = app.chat_input_buffer.clone();
    app.chat_manager.add_user_message(user_msg_1);
    app.chat_input_buffer.clear();
    app.chat_on_submit = false;
    
    // Simulate Fred starting to respond to first message
    let response_id_1 = app.chat_manager.start_agent_response();
    app.current_response_id = Some(response_id_1.clone());
    
    // Simulate streaming chunks for first response
    app.handle_llm_chunk("Rust is a systems".to_string(), false, &ctx);
    app.handle_llm_chunk(" programming language".to_string(), false, &ctx);
    app.handle_llm_chunk(" that focuses on safety.".to_string(), true, &ctx);
    
    // First response should be complete now
    assert!(app.current_response_id.is_none(), "current_response_id should be cleared after final chunk");
    
    // 2. User sends second message
    app.chat_input_buffer = "Can you give me an example?".to_string();
    app.chat_on_submit = true;
    
    // Simulate processing the second message
    let user_msg_2 = app.chat_input_buffer.clone();
    app.chat_manager.add_user_message(user_msg_2);
    app.chat_input_buffer.clear();
    app.chat_on_submit = false;
    
    // CRITICAL: Fred should start a NEW response, not reuse the old one
    let response_id_2 = app.chat_manager.start_agent_response();
    app.current_response_id = Some(response_id_2.clone());
    
    // The new response ID should be different from the first one
    assert_ne!(response_id_1, response_id_2, "Second response should have different ID");
    
    // Simulate streaming chunks for second response
    app.handle_llm_chunk("Here's a simple".to_string(), false, &ctx);
    app.handle_llm_chunk(" Rust example:".to_string(), false, &ctx);
    app.handle_llm_chunk("\n\nfn main() { println!(\"Hello!\"); }".to_string(), true, &ctx);
    
    // Get all messages
    let messages = app.chat_manager.get_all_messages();
    
    // CRITICAL TEST: Should have 4 separate messages
    assert_eq!(messages.len(), 4, "Should have 4 messages: user1, fred1, user2, fred2");
    
    // Check message sequence
    assert_eq!(messages[0].author, sagitta_code::gui::chat::view::MessageAuthor::User);
    assert_eq!(messages[0].content, "What is Rust?");
    
    assert_eq!(messages[1].author, sagitta_code::gui::chat::view::MessageAuthor::Agent);
    assert_eq!(messages[1].content, "Rust is a systems programming language that focuses on safety.");
    
    assert_eq!(messages[2].author, sagitta_code::gui::chat::view::MessageAuthor::User);
    assert_eq!(messages[2].content, "Can you give me an example?");
    
    assert_eq!(messages[3].author, sagitta_code::gui::chat::view::MessageAuthor::Agent);
    assert_eq!(messages[3].content, "Here's a simple Rust example:\n\nfn main() { println!(\"Hello!\"); }");
    
    // CRITICAL: Fred's messages should have different IDs
    assert_ne!(messages[1].id, messages[3].id, "Fred's responses should have different IDs");
    
    // CRITICAL: Timestamps should be in order
    assert!(messages[1].timestamp <= messages[2].timestamp);
    assert!(messages[2].timestamp <= messages[3].timestamp);
}

/// Test config file loading directly from files
#[tokio::test]
async fn test_config_loading_direct() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create sagitta_code_config.json
    let app_config_path = temp_dir.path().join("sagitta_code_config.json");
    fs::write(&app_config_path, r#"{
  "gemini": {
    "api_key": "custom-api-key",
    "model": "custom-model"
  },
  "ui": {
    "theme": "macchiato"
  }
}"#).await.unwrap();
    
    // Test loading the config directly
    let config = load_config_from_path(&app_config_path).expect("Should load config");
    
    // Verify values were loaded correctly
    assert_eq!(config.gemini.api_key, Some("custom-api-key".to_string()));
    assert_eq!(config.gemini.model, "custom-model");
    assert_eq!(config.ui.theme, "macchiato");
}

/// Test that reproduces the EXACT message overwriting issue described by the user
#[tokio::test]
async fn test_fred_message_overwriting_exact_scenario() {
    let temp_dir = TempDir::new().unwrap();
    let test_config = FredAgentConfig::default();
    
    let app_core_config_overwrite_test = sagitta_search::config::AppConfig::default();
    let repo_manager = Arc::new(Mutex::new(
        RepositoryManager::new(Arc::new(Mutex::new(app_core_config_overwrite_test.clone())))
    ));
    let mut app = FredAgentApp::new(repo_manager, test_config.clone(), app_core_config_overwrite_test);
    let ctx = egui::Context::default();
    
    // SCENARIO: User asks first question, Fred responds, then user asks second question
    
    // 1. User asks first question
    app.chat_manager.add_user_message("What is Rust?".to_string());
    
    // 2. Fred starts streaming first response
    app.current_response_id = None; // Ensure clean state
    app.handle_llm_chunk("Rust is a systems".to_string(), false, &ctx);
    let first_response_id = app.current_response_id.clone();
    app.handle_llm_chunk(" programming language".to_string(), false, &ctx);
    app.handle_llm_chunk(" that focuses on safety.".to_string(), true, &ctx);
    
    // At this point, current_response_id should be None (cleared after final chunk)
    assert!(app.current_response_id.is_none(), "current_response_id should be cleared after first response");
    
    // 3. User asks second question
    app.chat_manager.add_user_message("Can you give me an example?".to_string());
    
    // 4. Fred starts streaming second response - THIS SHOULD CREATE A NEW MESSAGE
    app.handle_llm_chunk("Here's a simple".to_string(), false, &ctx);
    let second_response_id = app.current_response_id.clone();
    app.handle_llm_chunk(" Rust example:".to_string(), false, &ctx);
    app.handle_llm_chunk("\n\nfn main() { println!(\"Hello!\"); }".to_string(), true, &ctx);
    
    // CRITICAL ASSERTIONS
    assert_ne!(first_response_id, second_response_id, "Second response should have different ID than first");
    
    let messages = app.chat_manager.get_all_messages();
    
    // Should have exactly 4 messages: user1, fred1, user2, fred2
    assert_eq!(messages.len(), 4, "Should have 4 messages: user1, fred1, user2, fred2");
    
    // Check that Fred's responses are separate messages with different content
    let fred_messages: Vec<_> = messages.iter()
        .filter(|m| m.author == sagitta_code::gui::chat::view::MessageAuthor::Agent)
        .collect();
    
    assert_eq!(fred_messages.len(), 2, "Should have exactly 2 Fred messages");
    assert_ne!(fred_messages[0].id, fred_messages[1].id, "Fred's messages should have different IDs");
    assert_ne!(fred_messages[0].content, fred_messages[1].content, "Fred's messages should have different content");
    
    // Verify the actual content
    assert_eq!(fred_messages[0].content, "Rust is a systems programming language that focuses on safety.");
    assert_eq!(fred_messages[1].content, "Here's a simple Rust example:\n\nfn main() { println!(\"Hello!\"); }");
    
    // Verify timestamps are in order
    assert!(fred_messages[1].timestamp > fred_messages[0].timestamp, "Second message should have later timestamp");
    
    println!("✅ FIXED: Fred creates separate messages instead of overwriting!");
}

/// Test that reproduces the EXACT conversation chain scenario from the user
#[tokio::test]
async fn test_exact_user_conversation_scenario() {
    let temp_dir = TempDir::new().unwrap();
    let test_config = FredAgentConfig::default();
    
    let app_core_config_scenario_test = sagitta_search::config::AppConfig::default();
    let repo_manager = Arc::new(Mutex::new(
        RepositoryManager::new(Arc::new(Mutex::new(app_core_config_scenario_test.clone())))
    ));
    let mut app = FredAgentApp::new(repo_manager, test_config.clone(), app_core_config_scenario_test);
    let ctx = egui::Context::default();
    
    // EXACT SCENARIO FROM USER'S CONVERSATION CHAIN:
    
    // 1. Check initial state (may or may not have welcome message)
    let initial_messages = app.chat_manager.get_all_messages();
    let initial_count = initial_messages.len();
    
    // 2. User says "hello"
    app.chat_input_buffer = "hello".to_string();
    app.chat_on_submit = true;
    
    // Simulate processing the first message
    let user_msg_1 = app.chat_input_buffer.clone();
    app.chat_manager.add_user_message(user_msg_1);
    
    // CRITICAL: Force clear current_response_id (this is what the fix does)
    if let Some(old_response_id) = app.current_response_id.take() {
        app.chat_manager.finish_streaming(&old_response_id);
    }
    app.current_response_id = None;
    app.chat_input_buffer.clear();
    app.chat_on_submit = false;
    
    // 3. Fred starts thinking and responding to "hello"
    // Simulate state change to Thinking (this should create NEW response ID)
    app.handle_agent_state_change(sagitta_code::agent::state::types::AgentState::Thinking { message: "Test thinking".to_string() }, &ctx);
    
    // Simulate thinking content (first chunk for this response turn)
    app.handle_llm_chunk("THINKING: I should respond helpfully to this greeting".to_string(), false, &ctx); 
    
    // NOW check the response ID
    let first_response_id = app.current_response_id.clone(); 
    assert!(first_response_id.is_some(), "Should have created response ID for first message after first chunk");
    
    // Simulate the long response about being a code assistant
    app.handle_llm_chunk("Hello! I'm Fred AI, a code assistant".to_string(), false, &ctx);
    app.handle_llm_chunk(" powered by Gemini and sagitta-search.".to_string(), false, &ctx);
    app.handle_llm_chunk(" I can help you understand and work with code repositories.".to_string(), true, &ctx);
    
    // First response should be complete now
    assert!(app.current_response_id.is_none(), "current_response_id should be cleared after first response");
    
    // 4. User says "tell me a joke"
    app.chat_input_buffer = "tell me a joke".to_string();
    app.chat_on_submit = true;
    
    // Simulate processing the second message
    let user_msg_2 = app.chat_input_buffer.clone();
    app.chat_manager.add_user_message(user_msg_2);
    
    // CRITICAL: Force clear current_response_id (this is what the fix does)
    if let Some(old_response_id) = app.current_response_id.take() {
        app.chat_manager.finish_streaming(&old_response_id);
    }
    app.current_response_id = None;
    app.chat_input_buffer.clear();
    app.chat_on_submit = false;
    
    // 5. Fred starts thinking and responding to "tell me a joke"
    // Simulate state change to Thinking (this should create DIFFERENT response ID)
    app.handle_agent_state_change(sagitta_code::agent::state::types::AgentState::Thinking { message: "Test thinking again".to_string() }, &ctx);
    
    // Simulate thinking content for joke (first chunk for this response turn)
    app.handle_llm_chunk("THINKING: I'm thinking of a joke to respond to the request".to_string(), false, &ctx);

    // NOW check the second response ID
    let second_response_id = app.current_response_id.clone();
    assert!(second_response_id.is_some(), "Should have created response ID for second message after first chunk");
    assert_ne!(first_response_id.as_ref().unwrap(), second_response_id.as_ref().unwrap(), "Second response ID should be different from first"); // Compare actual IDs

    // Simulate the joke response
    app.handle_llm_chunk("Why did the developer go broke?".to_string(), false, &ctx);
    app.handle_llm_chunk("\n\nBecause he used up all his cache!".to_string(), true, &ctx);
    
    // Get all messages
    let messages = app.chat_manager.get_all_messages();
    
    // CRITICAL ASSERTIONS: Should have initial_count + 4 messages total
    // initial messages + User: "hello" + Fred: response + User: "tell me a joke" + Fred: joke
    let expected_count = initial_count + 4;
    assert_eq!(messages.len(), expected_count, "Should have {} messages: initial + user1 + fred1 + user2 + fred2", expected_count);
    
    // Find the user and Fred messages (skip any initial messages)
    let user_and_fred_messages: Vec<_> = messages.iter().skip(initial_count).collect();
    assert_eq!(user_and_fred_messages.len(), 4, "Should have 4 new messages after initial");
    
    // Check message sequence and content
    assert_eq!(user_and_fred_messages[0].author, sagitta_code::gui::chat::view::MessageAuthor::User);
    assert_eq!(user_and_fred_messages[0].content, "hello");
    
    assert_eq!(user_and_fred_messages[1].author, sagitta_code::gui::chat::view::MessageAuthor::Agent);
    assert!(user_and_fred_messages[1].content.contains("code assistant"));
    
    assert_eq!(user_and_fred_messages[2].author, sagitta_code::gui::chat::view::MessageAuthor::User);
    assert_eq!(user_and_fred_messages[2].content, "tell me a joke");
    
    assert_eq!(user_and_fred_messages[3].author, sagitta_code::gui::chat::view::MessageAuthor::Agent);
    assert!(user_and_fred_messages[3].content.contains("developer go broke"));
    assert!(user_and_fred_messages[3].content.contains("cache"));
    
    // CRITICAL: Fred's messages should have different IDs and timestamps
    assert_ne!(user_and_fred_messages[1].id, user_and_fred_messages[3].id, "Fred's two responses should have different IDs");
    
    // Timestamps should be in order
    assert!(user_and_fred_messages[1].timestamp >= user_and_fred_messages[0].timestamp);
    assert!(user_and_fred_messages[2].timestamp >= user_and_fred_messages[1].timestamp);
    assert!(user_and_fred_messages[3].timestamp >= user_and_fred_messages[2].timestamp);
    
    println!("✅ FIXED: Exact user scenario - Fred creates separate messages with different timestamps!");
}

/// Test that verifies the Gemini streaming parser handles empty parts arrays
#[tokio::test]
async fn test_gemini_streaming_empty_parts_handling() {
    // Create a mock response that simulates the problematic JSON from Gemini
    let mock_response_json = r#"{"candidates": [{"content": {"role": "model"},"finishReason": "STOP","index": 0}],"usageMetadata": {"promptTokenCount": 5050,"candidatesTokenCount": 19,"totalTokenCount": 5069}}"#;
    
    // This test verifies that the JSON can be parsed with empty parts array
    let parsed: Result<GeminiResponse, _> = serde_json::from_str(mock_response_json);
    
    match parsed {
        Ok(response) => {
            assert_eq!(response.candidates.len(), 1);
            let candidate = &response.candidates[0];
            assert_eq!(candidate.content.parts.len(), 0); // Empty parts array
            
            // Debug what we actually got
            println!("Parsed finish_reason: {:?}", candidate.finish_reason);
            println!("Parsed content role: {:?}", candidate.content.role);
            println!("Parsed parts length: {}", candidate.content.parts.len());
            
            // The key fix is that empty parts arrays don't cause parsing errors
            println!("✅ FIXED: Gemini response with empty parts array parses correctly!");
        },
        Err(e) => {
            panic!("Failed to parse Gemini response with empty parts: {}", e);
        }
    }
}

#[tokio::test]
async fn test_agent_initialization_with_corrupted_conversations() {
    // Create temporary directory for test
    let temp_dir = TempDir::new().unwrap();
    let storage_path = temp_dir.path().to_path_buf();
    
    // Create conversations directory
    let conversations_dir = storage_path.join("conversations");
    fs::create_dir_all(&conversations_dir).await.unwrap();
    
    // Create a corrupted conversation file
    let conversation_id = Uuid::new_v4();
    let corrupted_file_path = conversations_dir.join(format!("{}.json", conversation_id));
    let corrupted_content = r#"{
        "id": "550e8400-e29b-41d4-a716-446655440000",
        "title": "Test Conversation",
        "created_at": "2023-01-01T00:00:00Z",
        "last_active": "2023-01-01T00:00:00Z",
        "messages": [],
        "status": "Active",
        "workspace_id": null,
        "tags": [],
        "branches": [],
        "checkpoints": [],
        "project_context": null
    "#; // Missing closing brace - this should cause JSON parsing to fail
    
    fs::write(&corrupted_file_path, corrupted_content).await.unwrap();
    
    // Create a corrupted index file as well
    let index_path = storage_path.join("index.json");
    let corrupted_index = r#"{
        "conversations": {
            "550e8400-e29b-41d4-a716-446655440000": {
                "id": "550e8400-e29b-41d4-a716-446655440000",
                "title": "Test Conversation",
                "workspace_id": null,
                "created_at": "2023-01-01T00:00:00Z",
                "last_active": "2023-01-01T00:00:00Z",
                "status": "Active",
                "message_count": 0,
                "tags": []
            }
        },
        "archived_conversations": {},
        "version": 0
    "#; // Missing closing brace
    
    fs::write(&index_path, corrupted_index).await.unwrap();
    
    // Test that persistence can handle corrupted files gracefully
    let persistence_result = DiskConversationPersistence::new(storage_path.clone()).await;
    assert!(persistence_result.is_ok(), "DiskConversationPersistence creation should succeed despite corrupted files");
    
    let persistence = persistence_result.unwrap();
    
    // Test loading conversations - should handle corruption gracefully
    let conversation_ids_result = persistence.list_conversation_ids().await;
    assert!(conversation_ids_result.is_ok(), "Listing conversation IDs should succeed despite corrupted files");
    
    // Try to load the corrupted conversation - should return None instead of error
    let load_result = persistence.load_conversation(conversation_id).await;
    assert!(load_result.is_ok(), "Loading corrupted conversation should not return error");
    assert!(load_result.unwrap().is_none(), "Loading corrupted conversation should return None");
    
    // Verify that corrupted files were moved to backup
    let backup_conversation_path = storage_path.join("corrupted").join(format!("{}.json.corrupted", conversation_id));
    let backup_index_path = storage_path.join("index.json.corrupted");
    
    // The corrupted conversation file should be moved to backup
    assert!(backup_conversation_path.exists(), "Corrupted conversation file should be moved to backup");
    assert!(!corrupted_file_path.exists(), "Original corrupted conversation file should be removed");
    
    // The corrupted index should be moved to backup and a new one created
    assert!(backup_index_path.exists(), "Corrupted index file should be moved to backup");
    
    println!("Conversation corruption handling test passed successfully");
} 