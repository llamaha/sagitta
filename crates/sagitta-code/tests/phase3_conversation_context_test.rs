use std::sync::Arc;
use tokio;
use uuid::Uuid;
use chrono::Utc;
use std::collections::HashMap;

use sagitta_code::agent::conversation::context_manager::{
    ConversationContextManager, FailureType, ConversationFlowState, StepStatus,
};

/// Test basic context manager functionality
#[tokio::test]
async fn test_context_manager_basic_functionality() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    let conversation_id = Uuid::new_v4();
    let context_manager = ConversationContextManager::new(conversation_id);

    // Test initial state
    let insights = context_manager.get_conversation_insights().await;
    assert_eq!(insights.flow_state, ConversationFlowState::Normal);
    assert_eq!(insights.frustration_level, 0.0);
    assert!(insights.recent_failures.is_empty());
    assert!(insights.current_plan_status.is_none());

    println!("✅ Basic context manager functionality test passed");
}

/// Test failure recording and pattern detection
#[tokio::test]
async fn test_failure_recording_and_pattern_detection() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    let conversation_id = Uuid::new_v4();
    let context_manager = ConversationContextManager::new(conversation_id);

    // Record first failure
    context_manager.record_failure(
        "web_search".to_string(),
        serde_json::json!({"query": "test"}),
        "Network timeout".to_string(),
        FailureType::ToolExecution,
        HashMap::new(),
    ).await.unwrap();

    let insights = context_manager.get_conversation_insights().await;
    assert_eq!(insights.recent_failures.len(), 1);
    assert_eq!(insights.frustration_level, 0.1); // 1 failure * 0.1
    assert_eq!(insights.flow_state, ConversationFlowState::Normal);

    // Record the same failure again
    context_manager.record_failure(
        "web_search".to_string(),
        serde_json::json!({"query": "test"}),
        "Network timeout".to_string(),
        FailureType::ToolExecution,
        HashMap::new(),
    ).await.unwrap();

    // Record a third consecutive failure - should trigger struggling state
    context_manager.record_failure(
        "edit_file".to_string(),
        serde_json::json!({"target_file": "test.txt"}),
        "Permission denied".to_string(),
        FailureType::ToolExecution,
        HashMap::new(),
    ).await.unwrap();

    let insights = context_manager.get_conversation_insights().await;
    assert_eq!(insights.recent_failures.len(), 3);
    assert_eq!(insights.flow_state, ConversationFlowState::Struggling);
    assert!(insights.frustration_level >= 0.3); // Should be higher now

    println!("✅ Failure recording and pattern detection test passed");
}

/// Test success recording and recovery
#[tokio::test]
async fn test_success_recording_and_recovery() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    let conversation_id = Uuid::new_v4();
    let context_manager = ConversationContextManager::new(conversation_id);

    // First, create a struggling state with multiple failures
    for i in 0..3 {
        context_manager.record_failure(
            format!("tool_{}", i),
            serde_json::json!({"param": i}),
            "Failed".to_string(),
            FailureType::ToolExecution,
            HashMap::new(),
        ).await.unwrap();
    }

    let insights_before = context_manager.get_conversation_insights().await;
    assert_eq!(insights_before.flow_state, ConversationFlowState::Struggling);

    // Record a success
    context_manager.record_success(
        "successful_action".to_string(),
        serde_json::json!({"param": "success"}),
        HashMap::new(),
        vec!["Good parameters".to_string()],
    ).await.unwrap();

    let insights_after = context_manager.get_conversation_insights().await;
    assert_eq!(insights_after.flow_state, ConversationFlowState::Recovery);
    assert!(insights_after.last_success.is_some());
    assert_eq!(insights_after.progress_summary.completed, 1);
    assert_eq!(insights_after.progress_summary.total_attempted, 0); // Only completed is tracked in success

    println!("✅ Success recording and recovery test passed");
}

/// Test multi-turn planning functionality
#[tokio::test]
async fn test_multi_turn_planning() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    let conversation_id = Uuid::new_v4();
    let context_manager = ConversationContextManager::new(conversation_id);

    // Create a multi-turn plan
    let steps = vec![
        (
            "Search for repository".to_string(),
            vec!["web_search".to_string()],
            vec!["Find repository URL".to_string()],
        ),
        (
            "Add repository".to_string(),
            vec!["add_repository".to_string()],
            vec!["Repository added successfully".to_string()],
        ),
        (
            "Sync repository".to_string(),
            vec!["sync_repository".to_string()],
            vec!["Repository synced".to_string()],
        ),
    ];

    let plan_id = context_manager.create_multi_turn_plan(
        "Add and sync a new repository".to_string(),
        steps,
    ).await.unwrap();

    let insights = context_manager.get_conversation_insights().await;
    assert_eq!(insights.flow_state, ConversationFlowState::MultiStepTask);
    assert!(insights.current_plan_status.is_some());
    
    let plan_status = insights.current_plan_status.unwrap();
    assert_eq!(plan_status.plan_id, plan_id);
    assert_eq!(plan_status.current_step, 0);
    assert_eq!(plan_status.total_steps, 3);
    assert!(plan_status.is_active);

    // Get current step
    let current_step = context_manager.get_current_step().await.unwrap();
    assert_eq!(current_step.description, "Search for repository");
    assert_eq!(current_step.status, StepStatus::Pending);

    // Complete first step
    let is_complete = context_manager.complete_current_step(HashMap::new()).await.unwrap();
    assert!(!is_complete); // Plan not complete yet

    // Check we moved to next step
    let current_step = context_manager.get_current_step().await.unwrap();
    assert_eq!(current_step.description, "Add repository");

    // Complete remaining steps
    context_manager.complete_current_step(HashMap::new()).await.unwrap();
    let is_complete = context_manager.complete_current_step(HashMap::new()).await.unwrap();
    assert!(is_complete); // Plan should be complete now

    // Check final state
    let insights_final = context_manager.get_conversation_insights().await;
    assert_eq!(insights_final.flow_state, ConversationFlowState::Normal);
    assert!(insights_final.current_plan_status.is_none() || !insights_final.current_plan_status.unwrap().is_active);

    println!("✅ Multi-turn planning test passed");
}

/// Test proactive assistance detection
#[tokio::test]
async fn test_proactive_assistance_detection() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    let conversation_id = Uuid::new_v4();
    let context_manager = ConversationContextManager::new(conversation_id);

    // Initially, no assistance should be needed
    let recommendation = context_manager.should_offer_proactive_assistance().await;
    assert!(!recommendation.should_assist);
    assert!(recommendation.confidence < 0.4);

    // Create a situation that should trigger assistance
    // Multiple consecutive failures
    for i in 0..4 {
        context_manager.record_failure(
            "same_action".to_string(),
            serde_json::json!({"retry": i}),
            "Keep failing".to_string(),
            FailureType::ParameterValidation,
            HashMap::new(),
        ).await.unwrap();
    }

    let recommendation = context_manager.should_offer_proactive_assistance().await;
    assert!(recommendation.should_assist);
    assert!(recommendation.confidence >= 0.4);
    assert!(!recommendation.recommendations.is_empty());
    assert!(!recommendation.suggested_actions.is_empty());

    // Check that recommendations are relevant
    let recommendations_text = recommendation.recommendations.join(" ");
    assert!(recommendations_text.contains("Multiple consecutive failures") || 
            recommendations_text.contains("stuck repeating the same action"));

    println!("✅ Proactive assistance detection test passed");
}

/// Test context preservation functionality
#[tokio::test]
async fn test_context_preservation() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    let conversation_id = Uuid::new_v4();
    let context_manager = ConversationContextManager::new(conversation_id);

    // Preserve some context
    let test_data = serde_json::json!({
        "last_working_url": "https://github.com/example/repo",
        "successful_parameters": {"branch": "main", "depth": 1}
    });

    context_manager.preserve_context(
        "repository_setup".to_string(),
        test_data.clone(),
    ).await.unwrap();

    // Retrieve the context
    let retrieved = context_manager.get_preserved_context("repository_setup").await;
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap(), test_data);

    // Test non-existent key
    let missing = context_manager.get_preserved_context("non_existent").await;
    assert!(missing.is_none());

    // Clear context
    context_manager.clear_preserved_context().await.unwrap();
    let cleared = context_manager.get_preserved_context("repository_setup").await;
    assert!(cleared.is_none());

    println!("✅ Context preservation test passed");
}

/// Test frustration level calculation
#[tokio::test]
async fn test_frustration_level_calculation() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    let conversation_id = Uuid::new_v4();
    let context_manager = ConversationContextManager::new(conversation_id);

    // Start with no frustration
    let insights = context_manager.get_conversation_insights().await;
    assert_eq!(insights.frustration_level, 0.0);

    // Add one failure - low frustration
    context_manager.record_failure(
        "action1".to_string(),
        serde_json::json!({}),
        "Error".to_string(),
        FailureType::ToolExecution,
        HashMap::new(),
    ).await.unwrap();

    let insights = context_manager.get_conversation_insights().await;
    assert!(insights.frustration_level > 0.0 && insights.frustration_level < 0.2);

    // Add multiple consecutive failures - higher frustration
    for i in 0..4 {
        context_manager.record_failure(
            format!("action{}", i + 2),
            serde_json::json!({}),
            "Error".to_string(),
            FailureType::ToolExecution,
            HashMap::new(),
        ).await.unwrap();
    }

    let insights = context_manager.get_conversation_insights().await;
    assert!(insights.frustration_level >= 0.4);

    // Add success - should reduce frustration
    context_manager.record_success(
        "success_action".to_string(),
        serde_json::json!({}),
        HashMap::new(),
        vec![],
    ).await.unwrap();

    let insights = context_manager.get_conversation_insights().await;
    // Frustration should be lower now (consecutive failures reset to 0)
    assert!(insights.frustration_level < 0.4);

    println!("✅ Frustration level calculation test passed");
}

/// Test conversation flow state transitions
#[tokio::test]
async fn test_conversation_flow_state_transitions() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    let conversation_id = Uuid::new_v4();
    let context_manager = ConversationContextManager::new(conversation_id);

    // Start in Normal state
    let insights = context_manager.get_conversation_insights().await;
    assert_eq!(insights.flow_state, ConversationFlowState::Normal);

    // Create multi-step plan -> MultiStepTask
    context_manager.create_multi_turn_plan(
        "Test plan".to_string(),
        vec![("Step 1".to_string(), vec![], vec![])],
    ).await.unwrap();

    let insights = context_manager.get_conversation_insights().await;
    assert_eq!(insights.flow_state, ConversationFlowState::MultiStepTask);

    // Complete the plan -> Normal
    context_manager.complete_current_step(HashMap::new()).await.unwrap();

    let insights = context_manager.get_conversation_insights().await;
    assert_eq!(insights.flow_state, ConversationFlowState::Normal);

    // Add multiple failures -> Struggling
    for _ in 0..3 {
        context_manager.record_failure(
            "failing_action".to_string(),
            serde_json::json!({}),
            "Error".to_string(),
            FailureType::ToolExecution,
            HashMap::new(),
        ).await.unwrap();
    }

    let insights = context_manager.get_conversation_insights().await;
    assert_eq!(insights.flow_state, ConversationFlowState::Struggling);

    // Add success -> Recovery
    context_manager.record_success(
        "recovery_action".to_string(),
        serde_json::json!({}),
        HashMap::new(),
        vec![],
    ).await.unwrap();

    let insights = context_manager.get_conversation_insights().await;
    assert_eq!(insights.flow_state, ConversationFlowState::Recovery);

    // Add another success -> Normal
    context_manager.record_success(
        "normal_action".to_string(),
        serde_json::json!({}),
        HashMap::new(),
        vec![],
    ).await.unwrap();

    let insights = context_manager.get_conversation_insights().await;
    assert_eq!(insights.flow_state, ConversationFlowState::Normal);

    println!("✅ Conversation flow state transitions test passed");
}

/// Integration test: Full conversation context workflow
#[tokio::test]
async fn test_full_conversation_context_workflow() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    let conversation_id = Uuid::new_v4();
    let context_manager = ConversationContextManager::new(conversation_id);

    println!("🔄 Starting full conversation context workflow test");

    // Phase 1: User starts with a complex request
    let plan_id = context_manager.create_multi_turn_plan(
        "Find Python repository, add it, and search for examples".to_string(),
        vec![
            (
                "Search for Python repository online".to_string(),
                vec!["web_search".to_string()],
                vec!["Find repository URL".to_string()],
            ),
            (
                "Add repository to system".to_string(),
                vec!["add_repository".to_string()],
                vec!["Repository added".to_string()],
            ),
            (
                "Search for Python examples".to_string(),
                vec!["codebase_search".to_string()],
                vec!["Find examples".to_string()],
            ),
        ],
    ).await.unwrap();

    println!("  - Created multi-turn plan: {}", plan_id);

    // Phase 2: First step fails multiple times
    for attempt in 1..=3 {
        context_manager.record_failure(
            "web_search".to_string(),
            serde_json::json!({"query": "Python repository"}),
            "Network timeout".to_string(),
            FailureType::ToolExecution,
            HashMap::new(),
        ).await.unwrap();
        println!("  - Recorded failure attempt {}", attempt);
    }

    // Check that proactive assistance is offered
    let recommendation = context_manager.should_offer_proactive_assistance().await;
    assert!(recommendation.should_assist);
    println!("  - Proactive assistance triggered: {}", recommendation.confidence);

    // Phase 3: Success on a different approach
    context_manager.preserve_context(
        "search_strategy".to_string(),
        serde_json::json!({"alternative_query": "Python examples repository GitHub"}),
    ).await.unwrap();

    context_manager.record_success(
        "web_search".to_string(),
        serde_json::json!({"query": "Python examples repository GitHub"}),
        HashMap::new(),
        vec!["Used more specific query".to_string()],
    ).await.unwrap();

    println!("  - Recorded successful web search with alternative approach");

    // Complete first step
    context_manager.complete_current_step(
        [("repository_url".to_string(), serde_json::json!("https://github.com/example/python-examples"))].iter().cloned().collect()
    ).await.unwrap();

    // Phase 4: Continue with plan - second step succeeds immediately
    context_manager.record_success(
        "add_repository".to_string(),
        serde_json::json!({"url": "https://github.com/example/python-examples"}),
        HashMap::new(),
        vec!["Used successful URL from previous step".to_string()],
    ).await.unwrap();

    context_manager.complete_current_step(HashMap::new()).await.unwrap();

    // Phase 5: Final step completes the plan
    context_manager.record_success(
        "codebase_search".to_string(),
        serde_json::json!({"query": "Python examples"}),
        HashMap::new(),
        vec!["Repository was properly indexed".to_string()],
    ).await.unwrap();

    let is_complete = context_manager.complete_current_step(HashMap::new()).await.unwrap();
    assert!(is_complete);

    // Phase 6: Verify final state
    let final_insights = context_manager.get_conversation_insights().await;
    
    assert_eq!(final_insights.flow_state, ConversationFlowState::Normal);
    assert_eq!(final_insights.progress_summary.completed, 3); // 3 successful actions
    assert!(final_insights.last_success.is_some());
    assert!(final_insights.recent_failures.len() == 3); // Still tracks recent failures
    
    // Should not need assistance anymore
    assert!(!final_insights.proactive_assistance.should_assist);

    // Context should be preserved
    let preserved = context_manager.get_preserved_context("search_strategy").await;
    assert!(preserved.is_some());

    println!("✅ Full conversation context workflow test passed");
    println!("  Final state: {:?}", final_insights.flow_state);
    println!("  Frustration level: {:.2}", final_insights.frustration_level);
    println!("  Completed tasks: {}", final_insights.progress_summary.completed);
} 