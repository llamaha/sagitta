use std::path::Path;
use ort::{Environment, SessionBuilder, OrtResult};

fn main() -> OrtResult<()> {
    // Initialize environment
    let environment = Environment::builder()
        .with_name("model_inspector")
        .build()?
        .into_arc();
    
    // Path to ONNX model
    let model_path = Path::new("onnx/all-minilm-l12-v2.onnx");
    println!("Analyzing model: {}", model_path.display());
    
    // Load model without optimization to inspect raw structure
    let session = SessionBuilder::new(&environment)?
        .with_optimization_level(ort::GraphOptimizationLevel::Disable)?
        .with_intra_threads(1)?
        .with_model_from_file(model_path)?;
    
    // Print input information
    println!("\nInput Nodes:");
    for (i, input) in session.inputs.iter().enumerate() {
        println!("  [{}] Name: {}", i, input.name);
        println!("      Shape: {:?}", input.dimensions);
        println!("      Type: {:?}", input.input_type);
    }
    
    // Print output information
    println!("\nOutput Nodes:");
    for (i, output) in session.outputs.iter().enumerate() {
        println!("  [{}] Name: {}", i, output.name);
        println!("      Shape: {:?}", output.dimensions);
        println!("      Type: {:?}", output.output_type);
    }
    
    println!("\nAnalysis complete.");
    Ok(())
} 