use anyhow::Result;
use sagitta_code::agent::conversation::clustering::{ConversationCluster, ConversationClusteringManager, ClusteringConfig};
use sagitta_code::agent::conversation::cluster_namer::{ClusterNamer, ClusterNamerConfig};
use sagitta_code::agent::conversation::types::{ConversationSummary, ProjectType};
use sagitta_code::agent::state::types::ConversationStatus;
use std::sync::Arc;
use uuid::Uuid;
use chrono::Utc;

/// Test that every ConversationCluster has non-empty, >3-char name after clustering
#[tokio::test]
async fn test_cluster_names_are_descriptive() -> Result<()> {
    let cluster_namer = create_test_cluster_namer().await?;
    
    // Create test conversations with related themes
    let conversations = vec![
        create_test_conversation("Rust Error Handling Help", vec!["rust".to_string(), "error".to_string()]),
        create_test_conversation("Rust Panic Recovery", vec!["rust".to_string(), "panic".to_string()]),
        create_test_conversation("Rust Result Type Usage", vec!["rust".to_string(), "result".to_string()]),
    ];
    
    // Create a cluster from these conversations
    let cluster = ConversationCluster {
        id: Uuid::new_v4(),
        title: "Placeholder Title".to_string(), // This should be replaced
        conversation_ids: conversations.iter().map(|c| c.id).collect(),
        centroid: vec![0.1, 0.2, 0.3],
        cohesion_score: 0.85,
        common_tags: vec!["rust".to_string(), "error".to_string()],
        dominant_project_type: Some(ProjectType::Rust),
        time_range: (Utc::now() - chrono::Duration::days(7), Utc::now()),
    };
    
    // Generate name for the cluster
    let generated_name = cluster_namer.generate_cluster_name(&cluster, &conversations).await?;
    
    // Verify the generated name meets requirements
    assert!(!generated_name.is_empty(), "Cluster name should not be empty");
    assert!(generated_name.len() > 3, "Cluster name should be longer than 3 characters, got: '{}'", generated_name);
    assert_ne!(generated_name, "Placeholder Title", "Generated name should replace placeholder");
    
    // Should be thematic and descriptive
    assert!(
        generated_name.to_lowercase().contains("rust") || 
        generated_name.to_lowercase().contains("error") ||
        generated_name.to_lowercase().contains("handling"),
        "Generated name should be thematic: '{}'", generated_name
    );
    
    Ok(())
}

/// Test cluster naming with different conversation themes
#[tokio::test]
async fn test_cluster_naming_different_themes() -> Result<()> {
    let cluster_namer = create_test_cluster_namer().await?;
    
    // Test Python data analysis cluster
    let python_conversations = vec![
        create_test_conversation("Pandas DataFrame Operations", vec!["python".to_string(), "pandas".to_string()]),
        create_test_conversation("NumPy Array Processing", vec!["python".to_string(), "numpy".to_string()]),
        create_test_conversation("Data Visualization with Matplotlib", vec!["python".to_string(), "visualization".to_string()]),
    ];
    
    let python_cluster = ConversationCluster {
        id: Uuid::new_v4(),
        title: "Generic Cluster".to_string(),
        conversation_ids: python_conversations.iter().map(|c| c.id).collect(),
        centroid: vec![0.4, 0.5, 0.6],
        cohesion_score: 0.78,
        common_tags: vec!["python".to_string(), "data".to_string()],
        dominant_project_type: Some(ProjectType::Python),
        time_range: (Utc::now() - chrono::Duration::days(5), Utc::now()),
    };
    
    let python_name = cluster_namer.generate_cluster_name(&python_cluster, &python_conversations).await?;
    
    // Test JavaScript web development cluster
    let js_conversations = vec![
        create_test_conversation("React Component Design", vec!["javascript".to_string(), "react".to_string()]),
        create_test_conversation("Express API Development", vec!["javascript".to_string(), "express".to_string()]),
        create_test_conversation("Node.js Backend Setup", vec!["javascript".to_string(), "nodejs".to_string()]),
    ];
    
    let js_cluster = ConversationCluster {
        id: Uuid::new_v4(),
        title: "Another Generic Cluster".to_string(),
        conversation_ids: js_conversations.iter().map(|c| c.id).collect(),
        centroid: vec![0.7, 0.8, 0.9],
        cohesion_score: 0.82,
        common_tags: vec!["javascript".to_string(), "web".to_string()],
        dominant_project_type: Some(ProjectType::JavaScript),
        time_range: (Utc::now() - chrono::Duration::days(3), Utc::now()),
    };
    
    let js_name = cluster_namer.generate_cluster_name(&js_cluster, &js_conversations).await?;
    
    // Names should be different and thematic
    assert_ne!(python_name, js_name, "Different themed clusters should have different names");
    
    // Python cluster name should relate to data/analysis
    assert!(
        python_name.to_lowercase().contains("python") ||
        python_name.to_lowercase().contains("data") ||
        python_name.to_lowercase().contains("analysis"),
        "Python cluster name should be thematic: '{}'", python_name
    );
    
    // JavaScript cluster name should relate to web/development
    assert!(
        js_name.to_lowercase().contains("javascript") ||
        js_name.to_lowercase().contains("web") ||
        js_name.to_lowercase().contains("development"),
        "JavaScript cluster name should be thematic: '{}'", js_name
    );
    
    Ok(())
}

/// Test edge cases: single conversation clusters and very similar conversations
#[tokio::test]
async fn test_cluster_naming_edge_cases() -> Result<()> {
    let cluster_namer = create_test_cluster_namer().await?;
    
    // Test single conversation cluster
    let single_conversation = vec![
        create_test_conversation("Unique Database Query Problem", vec!["database".to_string(), "sql".to_string()]),
    ];
    
    let single_cluster = ConversationCluster {
        id: Uuid::new_v4(),
        title: "Single Item".to_string(),
        conversation_ids: vec![single_conversation[0].id],
        centroid: vec![0.1, 0.1, 0.1],
        cohesion_score: 1.0, // Perfect cohesion for single item
        common_tags: vec!["database".to_string()],
        dominant_project_type: None,
        time_range: (Utc::now() - chrono::Duration::hours(1), Utc::now()),
    };
    
    let single_name = cluster_namer.generate_cluster_name(&single_cluster, &single_conversation).await?;
    
    assert!(!single_name.is_empty(), "Single conversation cluster should have a name");
    assert!(single_name.len() > 3, "Single conversation cluster name should be descriptive");
    
    // Test very similar conversations
    let similar_conversations = vec![
        create_test_conversation("How to handle Rust errors", vec!["rust".to_string(), "error".to_string()]),
        create_test_conversation("How to handle Rust error cases", vec!["rust".to_string(), "error".to_string()]),
        create_test_conversation("How to handle Rust error scenarios", vec!["rust".to_string(), "error".to_string()]),
    ];
    
    let similar_cluster = ConversationCluster {
        id: Uuid::new_v4(),
        title: "Similar Items".to_string(),
        conversation_ids: similar_conversations.iter().map(|c| c.id).collect(),
        centroid: vec![0.9, 0.9, 0.9],
        cohesion_score: 0.95, // Very high cohesion
        common_tags: vec!["rust".to_string(), "error".to_string()],
        dominant_project_type: Some(ProjectType::Rust),
        time_range: (Utc::now() - chrono::Duration::hours(2), Utc::now()),
    };
    
    let similar_name = cluster_namer.generate_cluster_name(&similar_cluster, &similar_conversations).await?;
    
    assert!(!similar_name.is_empty(), "Similar conversations cluster should have a name");
    assert!(similar_name.len() > 3, "Similar conversations cluster name should be descriptive");
    
    // Should still be able to generate meaningful names for very similar content
    assert!(
        similar_name.to_lowercase().contains("rust") ||
        similar_name.to_lowercase().contains("error"),
        "Similar conversations cluster should identify common theme: '{}'", similar_name
    );
    
    Ok(())
}

/// Test fallback behavior when LLM generation fails
#[tokio::test]
async fn test_cluster_naming_fallback() -> Result<()> {
    let cluster_namer = create_failing_cluster_namer().await?;
    
    let conversations = vec![
        create_test_conversation("Test Conversation 1", vec!["test".to_string()]),
        create_test_conversation("Test Conversation 2", vec!["test".to_string()]),
    ];
    
    let cluster = ConversationCluster {
        id: Uuid::new_v4(),
        title: "Original Title".to_string(),
        conversation_ids: conversations.iter().map(|c| c.id).collect(),
        centroid: vec![0.5, 0.5, 0.5],
        cohesion_score: 0.75,
        common_tags: vec!["test".to_string()],
        dominant_project_type: None,
        time_range: (Utc::now() - chrono::Duration::hours(1), Utc::now()),
    };
    
    let fallback_name = cluster_namer.generate_cluster_name(&cluster, &conversations).await?;
    
    // Should fall back to a reasonable default
    assert!(!fallback_name.is_empty(), "Fallback name should not be empty");
    assert!(fallback_name.len() > 3, "Fallback name should be descriptive");
    
    // Should follow fallback pattern (e.g., "Test Conversations", "Cluster", etc.)
    assert!(
        fallback_name.starts_with("Cluster") ||
        fallback_name.contains("Conversations") ||
        fallback_name.contains("Discussions") ||
        fallback_name.contains("test"),
        "Fallback name should follow expected pattern: '{}'", fallback_name
    );
    
    Ok(())
}

/// Test that cluster names are consistent for the same input
#[tokio::test]
async fn test_cluster_naming_consistency() -> Result<()> {
    let cluster_namer = create_test_cluster_namer().await?;
    
    let conversations = vec![
        create_test_conversation("Machine Learning Model Training", vec!["ml".to_string(), "training".to_string()]),
        create_test_conversation("Neural Network Architecture", vec!["ml".to_string(), "neural".to_string()]),
    ];
    
    let cluster = ConversationCluster {
        id: Uuid::new_v4(),
        title: "ML Cluster".to_string(),
        conversation_ids: conversations.iter().map(|c| c.id).collect(),
        centroid: vec![0.6, 0.7, 0.8],
        cohesion_score: 0.88,
        common_tags: vec!["ml".to_string()],
        dominant_project_type: None,
        time_range: (Utc::now() - chrono::Duration::days(1), Utc::now()),
    };
    
    // Generate name multiple times
    let name1 = cluster_namer.generate_cluster_name(&cluster, &conversations).await?;
    let name2 = cluster_namer.generate_cluster_name(&cluster, &conversations).await?;
    
    // Names should be consistent (or at least similar in theme)
    // Note: With LLM generation, exact consistency might not be guaranteed,
    // but the theme should be similar
    assert!(!name1.is_empty() && !name2.is_empty(), "Both generated names should be non-empty");
    
    // Both should relate to machine learning theme
    for name in [&name1, &name2] {
        assert!(
            name.to_lowercase().contains("ml") ||
            name.to_lowercase().contains("machine") ||
            name.to_lowercase().contains("learning") ||
            name.to_lowercase().contains("neural") ||
            name.to_lowercase().contains("model") ||
            name.to_lowercase().contains("architecture"),
            "Generated name should be thematic: '{}'", name
        );
    }
    
    Ok(())
}

/// Helper function to create a test conversation summary
fn create_test_conversation(title: &str, tags: Vec<String>) -> ConversationSummary {
    ConversationSummary {
        id: Uuid::new_v4(),
        title: title.to_string(),
        created_at: Utc::now() - chrono::Duration::hours(1),
        last_active: Utc::now(),
        message_count: 5,
        status: ConversationStatus::Active,
        tags,
        workspace_id: None,
        has_branches: false,
        has_checkpoints: false,
        project_name: None,
    }
}

/// Helper function to create a test cluster namer
async fn create_test_cluster_namer() -> Result<ClusterNamer> {
    Ok(ClusterNamer::new(ClusterNamerConfig::default()))
}

/// Helper function to create a failing cluster namer for testing fallbacks
async fn create_failing_cluster_namer() -> Result<ClusterNamer> {
    Ok(ClusterNamer::failing())
} 