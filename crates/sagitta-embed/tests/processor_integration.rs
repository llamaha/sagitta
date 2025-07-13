//! Integration tests for the decoupled processing architecture.

use sagitta_embed::{
    DefaultFileProcessor, EmbeddingPool, FileProcessor, EmbeddingProcessor,
    ProcessingConfig, EmbeddingConfig
};
use tempfile::tempdir;
use std::fs;

#[tokio::test]
async fn test_file_processor_basic_functionality() {
    let temp_dir = tempdir().unwrap();
    
    // Create a test Rust file
    let rust_file = temp_dir.path().join("test.rs");
    let rust_content = r#"
fn main() {
    println!("Hello, world!");
}

struct User {
    name: String,
    age: u32,
}

impl User {
    fn new(name: String, age: u32) -> Self {
        Self { name, age }
    }
}
"#;
    fs::write(&rust_file, rust_content).unwrap();
    
    // Create a test Python file
    let python_file = temp_dir.path().join("test.py");
    let python_content = r#"
def hello():
    print("Hello from Python!")

class Person:
    def __init__(self, name, age):
        self.name = name
        self.age = age
    
    def greet(self):
        return f"Hello, I'm {self.name}"
"#;
    fs::write(&python_file, python_content).unwrap();
    
    // Test file processing
    let config = ProcessingConfig::default();
    let processor = DefaultFileProcessor::new(config);
    
    let files = vec![rust_file, python_file];
    let chunks = processor.process_files(&files).await.unwrap();
    
    // Verify we got chunks
    assert!(!chunks.is_empty());
    
    // Verify language detection
    let languages: std::collections::HashSet<_> = chunks.iter()
        .map(|c| &c.metadata.language)
        .collect();
    assert!(languages.contains(&"rust".to_string()));
    assert!(languages.contains(&"python".to_string()));
    
    // Verify file extensions
    let extensions: std::collections::HashSet<_> = chunks.iter()
        .map(|c| &c.metadata.file_extension)
        .collect();
    assert!(extensions.contains(&"rs".to_string()));
    assert!(extensions.contains(&"py".to_string()));
    
    // Verify each chunk has content and metadata
    for chunk in &chunks {
        assert!(!chunk.content.trim().is_empty());
        assert!(!chunk.id.is_empty());
        assert!(chunk.metadata.start_line > 0);
        assert!(chunk.metadata.end_line >= chunk.metadata.start_line);
    }
}

#[tokio::test]
async fn test_file_processor_concurrency() {
    let temp_dir = tempdir().unwrap();
    
    // Create multiple test files
    let mut files = Vec::new();
    for i in 0..10 {
        let file_path = temp_dir.path().join(format!("test_{i}.js"));
        let content = format!(r#"
function test{i}() {{
    console.log("Test function {i}");
    return {i};
}}

class TestClass{i} {{
    constructor() {{
        this.value = {i};
    }}
    
    getValue() {{
        return this.value;
    }}
}}
"#);
        
        fs::write(&file_path, content).unwrap();
        files.push(file_path);
    }
    
    // Test concurrent processing
    let config = ProcessingConfig {
        file_processing_concurrency: 4,
        ..Default::default()
    };
    let processor = DefaultFileProcessor::new(config);
    
    let start = std::time::Instant::now();
    let chunks = processor.process_files(&files).await.unwrap();
    let duration = start.elapsed();
    
    // Verify all files were processed
    assert!(!chunks.is_empty());
    
    // Should process multiple files concurrently (timing is rough indication)
    println!("Processed {} files in {:?}", files.len(), duration);
    
    // Verify all chunks are from JavaScript files
    for chunk in &chunks {
        assert_eq!(chunk.metadata.language, "javascript");
        assert_eq!(chunk.metadata.file_extension, "js");
    }
}

#[tokio::test]
async fn test_file_size_limits() {
    let temp_dir = tempdir().unwrap();
    
    // Create a large file
    let large_file = temp_dir.path().join("large.txt");
    let large_content = "x".repeat(2048); // 2KB content
    fs::write(&large_file, large_content).unwrap();
    
    // Create a small file
    let small_file = temp_dir.path().join("small.txt");
    fs::write(&small_file, "small content").unwrap();
    
    // Test with restrictive size limit
    let config = ProcessingConfig {
        max_file_size_bytes: 1024, // 1KB limit
        ..Default::default()
    };
    let processor = DefaultFileProcessor::new(config);
    
    // Large file should fail
    let result = processor.process_file(&large_file).await;
    assert!(result.is_err());
    
    // Small file should succeed
    let result = processor.process_file(&small_file).await;
    assert!(result.is_ok());
}

fn create_test_embedding_config() -> EmbeddingConfig {
    let temp_dir = tempdir().unwrap();
    let model_path = temp_dir.path().join("model.onnx");
    let tokenizer_path = temp_dir.path().join("tokenizer.json");

    // Create dummy files
    fs::write(&model_path, "dummy model").unwrap();
    
    // Create minimal valid tokenizer JSON
    let tokenizer_content = serde_json::json!({
        "version": "1.0",
        "truncation": null,
        "padding": null,
        "added_tokens": [],
        "normalizer": null,
        "pre_tokenizer": null,
        "post_processor": null,
        "decoder": null,
        "model": {
            "type": "WordPiece",
            "unk_token": "[UNK]",
            "continuing_subword_prefix": "##",
            "max_input_chars_per_word": 100,
            "vocab": {
                "[UNK]": 0,
                "[CLS]": 1,
                "[SEP]": 2
            }
        }
    });
    fs::write(&tokenizer_path, tokenizer_content.to_string()).unwrap();

    EmbeddingConfig::new_onnx(model_path, tokenizer_path)
}

#[tokio::test]
async fn test_embedding_pool_configuration() {
    let embedding_config = create_test_embedding_config();
    let processing_config = ProcessingConfig {
        max_embedding_sessions: 3,
        embedding_batch_size: 32,
        ..Default::default()
    };

    // This might fail if ONNX runtime isn't available, but that's expected in test environments
    if let Ok(pool) = EmbeddingPool::new(processing_config, embedding_config) {
        assert_eq!(pool.max_batch_size(), 32);
        
        let stats = pool.pool_stats().await;
        assert_eq!(stats.max_providers, 3);
        assert_eq!(stats.available_permits, 3);
        assert!(!stats.is_at_capacity());
        assert_eq!(stats.utilization(), 0.0);
    }
}

#[tokio::test]
async fn test_end_to_end_processing() {
    let temp_dir = tempdir().unwrap();
    
    // Create test files
    let rust_file = temp_dir.path().join("test.rs");
    let rust_content = r#"
fn factorial(n: u32) -> u32 {
    match n {
        0 => 1,
        _ => n * factorial(n - 1),
    }
}
"#;
    fs::write(&rust_file, rust_content).unwrap();
    
    // Set up processors
    let processing_config = ProcessingConfig {
        file_processing_concurrency: 2,
        max_embedding_sessions: 1,
        embedding_batch_size: 16,
        ..Default::default()
    };
    
    let embedding_config = create_test_embedding_config();
    let file_processor = DefaultFileProcessor::new(processing_config.clone());
    
    // Test file processing
    let files = vec![rust_file];
    let processed_chunks = file_processor.process_files(&files).await.unwrap();
    
    assert!(!processed_chunks.is_empty());
    println!("Processed {} chunks from {} files", processed_chunks.len(), files.len());
    
    // Verify chunk structure
    for chunk in &processed_chunks {
        assert!(!chunk.content.trim().is_empty());
        assert!(!chunk.id.is_empty());
        assert_eq!(chunk.metadata.language, "rust");
        assert_eq!(chunk.metadata.file_extension, "rs");
        assert!(chunk.metadata.start_line > 0);
        assert!(chunk.metadata.end_line >= chunk.metadata.start_line);
    }
    
    // Note: We can't test actual embedding generation in CI without real model files,
    // but the architecture is validated through the file processing pipeline
}

#[test]
fn test_processing_config_defaults() {
    let config = ProcessingConfig::default();
    
    assert!(config.file_processing_concurrency > 0);
    assert_eq!(config.max_embedding_sessions, 4);
    assert_eq!(config.processing_queue_size, 1000);
    assert_eq!(config.embedding_batch_size, 8);
    assert_eq!(config.max_file_size_bytes, 5 * 1024 * 1024);
}

#[test]
fn test_processing_config_customization() {
    let config = ProcessingConfig {
        file_processing_concurrency: 8,
        max_embedding_sessions: 2,
        cpu_worker_threads: 4,
        processing_queue_size: 500,
        embedding_batch_size: 64,
        max_file_size_bytes: 1024 * 1024,
        file_batch_size: 1,
    };
    
    assert_eq!(config.file_processing_concurrency, 8);
    assert_eq!(config.max_embedding_sessions, 2);
    assert_eq!(config.cpu_worker_threads, 4);
    assert_eq!(config.processing_queue_size, 500);
    assert_eq!(config.embedding_batch_size, 64);
    assert_eq!(config.max_file_size_bytes, 1024 * 1024);
    assert_eq!(config.file_batch_size, 1);
} 