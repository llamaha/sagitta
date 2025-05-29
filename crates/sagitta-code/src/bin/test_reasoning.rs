use std::sync::Arc;
use tokio;
use sagitta_code::{
    tools::registry::ToolRegistry,
    reasoning::{AgentToolExecutor, AgentEventEmitter, create_reasoning_config, SagittaCodeIntentAnalyzer},
    reasoning::llm_adapter::ReasoningLlmClientAdapter,
    config::SagittaCodeConfig,
    llm::gemini::client::GeminiClient,
    llm::client::LlmClient,
};
use reasoning_engine::{ReasoningEngine, traits::{LlmMessage, LlmMessagePart}};
use sagitta_search::embedding::provider::onnx::{OnnxEmbeddingModel, ThreadSafeOnnxProvider};
use sagitta_search::embedding::provider::EmbeddingProvider;
use sagitta_search::embedding::EmbeddingModelType;
use sagitta_search::error::SagittaError;
use futures_util::StreamExt;
use std::path::Path;
use tokio::sync::{broadcast, mpsc};

// Mock stream handler for testing
struct TestStreamHandler {
    output: Arc<tokio::sync::Mutex<Vec<String>>>,
}

impl TestStreamHandler {
    fn new() -> Self {
        Self {
            output: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }
    
    async fn get_output(&self) -> Vec<String> {
        self.output.lock().await.clone()
    }
}

#[async_trait::async_trait]
impl reasoning_engine::traits::StreamHandler for TestStreamHandler {
    async fn handle_chunk(&self, chunk: reasoning_engine::streaming::StreamChunk) -> reasoning_engine::Result<()> {
        if let Ok(text) = String::from_utf8(chunk.data) {
            self.output.lock().await.push(text.clone());
            print!("{}", text); // Also print to console
        }
        Ok(())
    }
    
    async fn handle_stream_complete(&self, _stream_id: uuid::Uuid) -> reasoning_engine::Result<()> {
        println!("\n[Stream completed]");
        Ok(())
    }
    
    async fn handle_stream_error(&self, _stream_id: uuid::Uuid, error: reasoning_engine::ReasoningError) -> reasoning_engine::Result<()> {
        eprintln!("Stream error: {}", error);
        Ok(())
    }
}

// Mock embedding provider for testing when ONNX models aren't available
#[derive(Debug)]
struct MockEmbeddingProvider;

impl MockEmbeddingProvider {
    fn new() -> Self {
        Self
    }
}

impl EmbeddingProvider for MockEmbeddingProvider {
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SagittaError> {
        // Return mock embeddings (just random vectors for testing)
        let embeddings = texts.iter().map(|_| {
            (0..384).map(|i| (i as f32) * 0.001).collect()
        }).collect();
        Ok(embeddings)
    }
    
    fn dimension(&self) -> usize {
        384
    }
    
    fn model_type(&self) -> EmbeddingModelType {
        EmbeddingModelType::Onnx
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();
    
    println!("🧪 Testing Reasoning Engine with Sidekiq Scenario");
    println!("{}", "=".repeat(60));
    
    // Create config
    let config = SagittaCodeConfig::default();
    
    // Check if we have the required API key
    if config.gemini.api_key.as_ref().map_or(true, |key| key.is_empty()) {
        eprintln!("❌ Error: GEMINI_API_KEY environment variable not set");
        eprintln!("Please set your Gemini API key: export GEMINI_API_KEY=your_key_here");
        std::process::exit(1);
    }
    
    // Create embedding provider (use a fallback if models don't exist)
    let embedding_provider = {
        let model_path = Path::new("./models/all-MiniLM-L6-v2.onnx");
        let tokenizer_path = Path::new("./models/tokenizer.json");
        
        if model_path.exists() && tokenizer_path.exists() {
            println!("✓ Using ONNX embedding model");
            let onnx_model = OnnxEmbeddingModel::new(model_path, tokenizer_path)?;
            Arc::new(ThreadSafeOnnxProvider::new(onnx_model))
        } else {
            println!("⚠️  ONNX models not found, using mock embedding provider");
            // For testing, we'll just use the mock provider directly
            // In a real scenario, we'd need to handle this type mismatch properly
            println!("⚠️  Note: Using mock provider for testing - this may cause type issues");
            Arc::new(ThreadSafeOnnxProvider::new(
                OnnxEmbeddingModel::new(
                    Path::new("./dummy.onnx"), 
                    Path::new("./dummy.json")
                ).unwrap_or_else(|_| {
                    // If we can't create a real ONNX model, we'll need to skip this test
                    eprintln!("❌ Cannot create ONNX model for testing. Please ensure ONNX models are available or run with proper embedding setup.");
                    std::process::exit(1);
                })
            ))
        }
    };
    
    // Create LLM client
    let llm_client: Arc<dyn LlmClient> = Arc::new(GeminiClient::new(&config)?);
    
    // Create tool registry
    let tool_registry = Arc::new(ToolRegistry::new());
    
    // Create reasoning components
    let tool_executor = Arc::new(AgentToolExecutor::new(tool_registry.clone()));
    let (event_sender, _event_receiver) = broadcast::channel(100);
    let event_emitter = Arc::new(AgentEventEmitter::new(event_sender.clone()));
    let stream_handler = Arc::new(TestStreamHandler::new());
    
    // Create reasoning engine
    let llm_adapter = Arc::new(ReasoningLlmClientAdapter::new(llm_client.clone(), tool_registry.clone()));
    let intent_analyzer = Arc::new(SagittaCodeIntentAnalyzer::new(embedding_provider.clone()));
    let reasoning_config = create_reasoning_config(&config);
    
    let mut reasoning_engine = ReasoningEngine::new(
        reasoning_config,
        llm_adapter,
        intent_analyzer,
    ).await?;
    
    // The Sidekiq test scenario
    let test_message = r#"User has reported this bug in sidekiq:

Ruby version: 3.4.2
Rails version: 8.0.2
Sidekiq / Pro / Enterprise version(s): sidekiq (8.0.3) / sidekiq-pro (8.0.1)

The web UI is defaulting the first 5 character match of the Accept-Language header. This is not how the spec is defined. It should be matching in order of quality value.

For example, this value is returning zh-TW when it should be matching on en first.

en-US,en;q=0.9,es;q=0.8,zh-TW;q=0.7,zh;q=0.6

Other weirdness comes into play with zh-XX locales because there is no zh locale. So an accept header with a zh variant that is not zh-CN or zh-TW will match zh-CN. This is not a valid fallback language and would be better to just fallback to the default en.

The user_prefered_languages method already sorts locales in the header based on their quality value, so the locale method just needs to respect it.

Please web search for the clone url and correct branch / target_ref then add / sync the repo then do an investigation using the various tools you have available and produce an analysis."#;

    println!("📝 Test Input:");
    println!("{}", test_message);
    println!("\n🤖 Reasoning Engine Response:");
    println!("{}", "-".repeat(60));
    
    // Create LLM message history
    let llm_history = vec![
        LlmMessage {
            role: "user".to_string(),
            parts: vec![LlmMessagePart::Text(test_message.to_string())],
        }
    ];
    
    // Process with reasoning engine
    let start_time = std::time::Instant::now();
    let result = reasoning_engine.process(
        llm_history,
        tool_executor,
        event_emitter,
        stream_handler.clone(),
    ).await;
    
    let duration = start_time.elapsed();
    
    println!("\n{}", "=".repeat(60));
    println!("📊 Test Results:");
    
    match result {
        Ok(state) => {
            println!("✅ Reasoning completed successfully!");
            println!("   Session ID: {}", state.session_id);
            println!("   Success: {}", state.is_successful());
            println!("   Steps: {}", state.history.len());
            println!("   Duration: {:?}", duration);
            
            if let Some(reason) = &state.completion_reason {
                println!("   Completion reason: {}", reason);
            }
            
            // Check for specific issues
            let mut issues_found = Vec::new();
            
            for step in &state.history {
                if !step.success {
                    issues_found.push(format!("Failed step: {:?}", step.step_type));
                }
                if let Some(error) = &step.error {
                    if error.contains("Tool reported failure despite overall orchestration success claim") {
                        issues_found.push("Tool failure detection issue found".to_string());
                    }
                }
            }
            
            if issues_found.is_empty() {
                println!("✅ No issues detected in reasoning process!");
            } else {
                println!("⚠️  Issues found:");
                for issue in issues_found {
                    println!("   - {}", issue);
                }
            }
        }
        Err(e) => {
            println!("❌ Reasoning failed: {}", e);
        }
    }
    
    // Print captured output
    let output = stream_handler.get_output().await;
    if !output.is_empty() {
        println!("\n📄 Captured Output:");
        for line in output {
            print!("{}", line);
        }
    }
    
    Ok(())
} 