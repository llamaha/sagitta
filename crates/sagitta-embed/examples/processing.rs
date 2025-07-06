use sagitta_embed::{
    EmbeddingConfig, EmbeddingModelType,
    DefaultFileProcessor, EmbeddingPool, FileProcessor, EmbeddingProcessor,
    ProcessingConfig
};
use std::path::PathBuf;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Sagitta-Embed Processing Example");
    println!("=================================");
    
    // Note: This example requires actual model files to run
    let model_path = "path/to/your/model.onnx";
    let tokenizer_path = "path/to/your/tokenizer.json";
    
    // Check if model files exist
    if !std::path::Path::new(model_path).exists() || !std::path::Path::new(tokenizer_path).exists() {
        println!("⚠️  Model files not found. This example requires:");
        println!("   - ONNX model file: {model_path}");
        println!("   - Tokenizer file: {tokenizer_path}");
        println!();
        println!("To run this example:");
        println!("1. Download a compatible ONNX embedding model");
        println!("2. Update the paths in this example");
        println!("3. Run: cargo run --example processing");
        return Ok(());
    }

    // Configure the embedding pipeline
    let config = EmbeddingConfig::default()
        .with_model_type(EmbeddingModelType::Onnx)
        // max_sessions removed - using automatic session management
        .with_embedding_batch_size(128); // Add the new field

    // Set the ONNX paths
    let embedding_config = EmbeddingConfig {
        onnx_model_path: Some(model_path.into()),
        onnx_tokenizer_path: Some(tokenizer_path.into()),
        enable_session_cleanup: true,
        ..config
    };

    // Create processing config from embedding config
    let processing_config = ProcessingConfig::from_embedding_config(&embedding_config);
    
    println!("Configuration:");
    println!("  File processing concurrency: {} CPU cores", processing_config.file_processing_concurrency);
    println!("  Embedding model instances: {} (automatic)", processing_config.max_embedding_sessions);
    println!("  Embedding batch size: {}", processing_config.embedding_batch_size);
    println!();

    // Create the processors
    let file_processor = DefaultFileProcessor::new(processing_config.clone());
    let max_embedding_sessions = processing_config.max_embedding_sessions; // Save this before move
    
    // Create embedding pool with configured sessions
    let embedding_pool = EmbeddingPool::with_configured_sessions(embedding_config)?;

    // Create some test files to process
    let temp_dir = tempfile::tempdir()?;
    let test_files = create_test_files(&temp_dir)?;
    
    println!("Processing {} test files...", test_files.len());

    // Phase 1: File Processing (CPU-intensive, scales to all cores)
    println!("Phase 1: File Processing (CPU-intensive)");
    let file_start = Instant::now();
    
    let processed_chunks = file_processor.process_files(&test_files).await?;
    
    let file_duration = file_start.elapsed();
    println!("  Processed {} files into {} chunks in {:?}", 
             test_files.len(), processed_chunks.len(), file_duration);
    println!("  Average: {:.2} files/second", 
             test_files.len() as f64 / file_duration.as_secs_f64());

    // Show pool stats before embedding
    let stats = embedding_pool.pool_stats().await;
    println!("Embedding pool stats before processing:");
    println!("  Available models: {}/{}", stats.available_providers, stats.max_providers);
    println!("  Available permits: {}", stats.available_permits);
    println!("  Utilization: {:.1}%", stats.utilization() * 100.0);

    // Phase 2: Embedding Generation (GPU-intensive, controlled concurrency)
    println!();
    println!("Phase 2: Embedding Generation (GPU-intensive)");
    let embed_start = Instant::now();
    
    let embedded_chunks = embedding_pool.process_chunks(processed_chunks).await?;
    
    let embed_duration = embed_start.elapsed();
    println!("  Generated {} embeddings in {:?}", 
             embedded_chunks.len(), embed_duration);
    println!("  Average: {:.2} embeddings/second", 
             embedded_chunks.len() as f64 / embed_duration.as_secs_f64());

    // Show pool stats after embedding
    let stats = embedding_pool.pool_stats().await;
    println!("Embedding pool stats after processing:");
    println!("  Available models: {}/{}", stats.available_providers, stats.max_providers);
    println!("  Available permits: {}", stats.available_permits);
    println!("  Utilization: {:.1}%", stats.utilization() * 100.0);

    // Summary
    let total_duration = file_start.elapsed();
    println!();
    println!("Summary:");
    println!("  Total time: {total_duration:?}");
    println!("  File processing: {:?} ({:.1}%)", 
             file_duration, 
             file_duration.as_secs_f64() / total_duration.as_secs_f64() * 100.0);
    println!("  Embedding generation: {:?} ({:.1}%)", 
             embed_duration,
             embed_duration.as_secs_f64() / total_duration.as_secs_f64() * 100.0);
    
    // Verify embeddings
    if !embedded_chunks.is_empty() {
        let first_embedding = &embedded_chunks[0].embedding;
        println!("  First embedding dimension: {}", first_embedding.len());
        println!("  Sample values: {:?}", &first_embedding[..5.min(first_embedding.len())]);
    }

    println!();
    println!("Key Benefits Demonstrated:");
    println!("✅ File processing scales to {} CPU cores", num_cpus::get());
    println!("✅ Embedding models limited to {max_embedding_sessions} instances (controlled GPU memory)");
    println!("✅ Independent scaling of CPU vs GPU intensive work");
    println!("✅ Async pipeline prevents blocking between phases");

    Ok(())
}

fn create_test_files(temp_dir: &tempfile::TempDir) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    
    // Create various test files
    for i in 0..10 {
        let file_path = temp_dir.path().join(format!("test_{i}.rs"));
        let content = format!(r#"
// Test file {i}
use std::collections::HashMap;

pub struct TestStruct_{i} {{
    data: HashMap<String, i32>,
    counter: usize,
}}

impl TestStruct_{i} {{
    pub fn new() -> Self {{
        Self {{
            data: HashMap::new(),
            counter: 0,
        }}
    }}
    
    pub fn add_item(&mut self, key: String, value: i32) {{
        self.data.insert(key, value);
        self.counter += 1;
    }}
    
    pub fn get_count(&self) -> usize {{
        self.counter
    }}
}}

pub fn test_function_{i}() -> String {{
    let mut test = TestStruct_{i}::new();
    test.add_item("example".to_string(), {i});
    format!("Test {i} completed with count: {{}}", test.get_count())
}}

#[cfg(test)]
mod tests {{
    use super::*;
    
    #[test]
    fn test_creation() {{
        let test = TestStruct_{i}::new();
        assert_eq!(test.get_count(), 0);
    }}
    
    #[test]
    fn test_add_item() {{
        let mut test = TestStruct_{i}::new();
        test.add_item("key".to_string(), 42);
        assert_eq!(test.get_count(), 1);
    }}
}}
"#);
        
        std::fs::write(&file_path, content)?;
        files.push(file_path);
    }
    
    // Add some Python files too
    for i in 0..5 {
        let file_path = temp_dir.path().join(format!("test_{i}.py"));
        let content = format!(r#"
"""
Test Python file {i}
"""

class TestClass{i}:
    def __init__(self):
        self.data = {{}}
        self.counter = 0
    
    def add_item(self, key, value):
        """Add an item to the data dictionary."""
        self.data[key] = value
        self.counter += 1
    
    def get_count(self):
        """Get the current count of items."""
        return self.counter
    
    def process_data(self):
        """Process all data items."""
        result = []
        for key, value in self.data.items():
            result.append(f"{{key}}: {{value * 2}}")
        return result

def test_function_{i}():
    """Test function for file {i}."""
    test = TestClass{i}()
    test.add_item("example", {i})
    return f"Test {i} completed with count: {{test.get_count()}}"

if __name__ == "__main__":
    print(test_function_{i}())
"#);
        
        std::fs::write(&file_path, content)?;
        files.push(file_path);
    }
    
    Ok(files)
} 