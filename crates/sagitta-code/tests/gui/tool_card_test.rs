use sagitta_code::gui::chat::types::{ToolCard, ToolCardStatus};
use serde_json::Value;
use std::collections::HashMap;

#[test]
fn test_tool_card_creation() {
    let input_params = HashMap::from([
        ("file_path".to_string(), Value::String("/test/path".to_string())),
        ("content".to_string(), Value::String("test content".to_string())),
    ]);
    
    let tool_card = ToolCard {
        id: "test_id".to_string(),
        tool_name: "write_file".to_string(),
        input_params,
        status: ToolCardStatus::Completed { success: true },
        result: Some("File written successfully".to_string()),
        duration: Some(std::time::Duration::from_millis(150)),
    };
    
    assert_eq!(tool_card.id, "test_id");
    assert_eq!(tool_card.tool_name, "write_file");
    assert!(matches!(tool_card.status, ToolCardStatus::Completed { success: true }));
    assert!(tool_card.result.is_some());
    assert!(tool_card.duration.is_some());
}

#[test] 
fn test_tool_card_status_variants() {
    let test_cases = vec![
        ToolCardStatus::Running,
        ToolCardStatus::Completed { success: true },
        ToolCardStatus::Completed { success: false },
        ToolCardStatus::Failed { error: "Test error".to_string() },
        ToolCardStatus::Cancelled,
    ];
    
    for status in test_cases {
        let tool_card = ToolCard {
            id: "test".to_string(),
            tool_name: "test_tool".to_string(),
            input_params: HashMap::new(),
            status,
            result: None,
            duration: None,
        };
        
        // Basic validation that the tool card can be created with each status
        assert_eq!(tool_card.tool_name, "test_tool");
    }
}

#[test]
fn test_tool_card_with_complex_params() {
    let mut complex_params = HashMap::new();
    complex_params.insert("simple_string".to_string(), Value::String("hello".to_string()));
    complex_params.insert("number".to_string(), Value::Number(serde_json::Number::from(42)));
    complex_params.insert("boolean".to_string(), Value::Bool(true));
    
    let nested_object = serde_json::json!({
        "nested_key": "nested_value",
        "nested_array": [1, 2, 3]
    });
    complex_params.insert("object".to_string(), nested_object);
    
    let tool_card = ToolCard {
        id: "complex_test".to_string(),
        tool_name: "semantic_code_search".to_string(),
        input_params: complex_params,
        status: ToolCardStatus::Completed { success: true },
        result: Some("Search completed".to_string()),
        duration: Some(std::time::Duration::from_millis(500)),
    };
    
    assert_eq!(tool_card.input_params.len(), 4);
    assert!(tool_card.input_params.contains_key("simple_string"));
    assert!(tool_card.input_params.contains_key("object"));
}

#[test]
fn test_tool_card_duration_formatting() {
    let durations = vec![
        std::time::Duration::from_millis(50),
        std::time::Duration::from_millis(1500),
        std::time::Duration::from_secs(5),
        std::time::Duration::from_secs(65),
    ];
    
    for duration in durations {
        let tool_card = ToolCard {
            id: "duration_test".to_string(),
            tool_name: "test_tool".to_string(),
            input_params: HashMap::new(),
            status: ToolCardStatus::Completed { success: true },
            result: None,
            duration: Some(duration),
        };
        
        // Verify duration is stored correctly
        assert_eq!(tool_card.duration, Some(duration));
    }
}