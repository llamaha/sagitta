use qdrant_client::qdrant::{Value, ScoredPoint, PointId};
use std::collections::HashMap;
use vectordb_cli::cli::commands::{
    FIELD_FILE_PATH, FIELD_START_LINE, FIELD_LANGUAGE, FIELD_ELEMENT_TYPE, FIELD_CHUNK_CONTENT,
    // FIELD_BRANCH, FIELD_COMMIT_HASH, // Not used in this test file, but available
};
use vectordb_cli::cli::formatters::print_search_results;
use anyhow::Result;
use std::io::Cursor;

// Add test module to ensure tests are discovered
#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_scored_point(
        file_path: &str,
        start_line: i64,
        language: &str,
        element_type: &str,
        content: &str,
        score: f32,
    ) -> ScoredPoint {
        let mut payload = HashMap::new();
        payload.insert(FIELD_FILE_PATH.to_string(), Value { kind: Some(qdrant_client::qdrant::value::Kind::StringValue(file_path.to_string())) });
        payload.insert(FIELD_START_LINE.to_string(), Value { kind: Some(qdrant_client::qdrant::value::Kind::IntegerValue(start_line)) });
        payload.insert(FIELD_LANGUAGE.to_string(), Value { kind: Some(qdrant_client::qdrant::value::Kind::StringValue(language.to_string())) });
        payload.insert(FIELD_ELEMENT_TYPE.to_string(), Value { kind: Some(qdrant_client::qdrant::value::Kind::StringValue(element_type.to_string())) });
        payload.insert(FIELD_CHUNK_CONTENT.to_string(), Value { kind: Some(qdrant_client::qdrant::value::Kind::StringValue(content.to_string())) });

        ScoredPoint {
            id: Some(PointId { point_id_options: Some(qdrant_client::qdrant::point_id::PointIdOptions::Num(1)) }),
            payload,
            score,
            vectors: None,
            shard_key: None,
            version: 0,
            order_value: None,
        }
    }

    #[test]
    fn test_print_search_results_with_results() {
        let points = vec![
            create_test_scored_point(
                "src/main.rs", 
                10, 
                "rust", 
                "function", 
                "fn hello_world() {\n    println!(\"Hello, world!\");\n}", 
                0.95
            ),
            create_test_scored_point(
                "src/lib.rs", 
                20, 
                "rust", 
                "struct", 
                "struct User {\n    name: String,\n    age: u32\n}", 
                0.85
            ),
        ];

        // This test primarily verifies the function doesn't panic
        let result = print_search_results(&points, "test query", false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_print_search_results_empty() {
        let empty_points: Vec<ScoredPoint> = vec![];
        
        let result = print_search_results(&empty_points, "empty query", false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_print_search_results_missing_fields() {
        let mut point = create_test_scored_point(
            "src/main.rs", 
            10, 
            "rust", 
            "function", 
            "fn hello_world() {\n    println!(\"Hello, world!\");\n}", 
            0.95
        );
        
        // Remove some fields to test handling of missing fields
        point.payload.remove(FIELD_LANGUAGE);
        point.payload.remove(FIELD_ELEMENT_TYPE);
        
        let points = vec![point];
        
        // Should still work with missing fields
        let result = print_search_results(&points, "missing fields query", false);
        assert!(result.is_ok());
    }
} 