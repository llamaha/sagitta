use std::path::Path;
use anyhow::{Result, Context};
use ort::{GraphOptimizationLevel, Session};

fn main() -> Result<()> {
    // Path to ONNX model
    let model_path = Path::new("onnx/all-minilm-l12-v2.onnx");
    println!("Analyzing model: {}", model_path.display());
    
    // Load model without optimization to inspect raw structure
    let session = Session::builder()?
        .with_optimization_level(GraphOptimizationLevel::Disable)?
        .commit_from_file(model_path)
        .context("Failed to load ONNX model")?;
    
    // Print model metadata
    println!("\nModel Metadata:");
    if let Some(metadata) = session.metadata() {
        println!("  Name: {}", metadata.name().unwrap_or("Unknown"));
        println!("  Producer: {}", metadata.producer().unwrap_or("Unknown"));
        println!("  Domain: {}", metadata.domain().unwrap_or("Unknown"));
        println!("  Description: {}", metadata.description().unwrap_or("Unknown"));
        println!("  Version: {}", metadata.version());
    } else {
        println!("  No metadata available");
    }
    
    // Print input information
    println!("\nInput Nodes:");
    for (i, input) in session.inputs.iter().enumerate() {
        let shape_str = format_shape(&input.dimensions);
        println!("  [{}] Name: {}", i, input.name);
        println!("      Shape: {}", shape_str);
        println!("      Type: {:?}", input.input_type);
    }
    
    // Print output information
    println!("\nOutput Nodes:");
    for (i, output) in session.outputs.iter().enumerate() {
        let shape_str = format_shape(&output.dimensions);
        println!("  [{}] Name: {}", i, output.name);
        println!("      Shape: {}", shape_str);
        println!("      Type: {:?}", output.output_type);
    }
    
    println!("\nAnalysis complete.");
    Ok(())
}

// Helper function to format shape information
fn format_shape(dimensions: &Option<Vec<i64>>) -> String {
    match dimensions {
        Some(dims) => {
            let dims_str: Vec<String> = dims.iter()
                .map(|d| if *d < 0 { "?".to_string() } else { d.to_string() })
                .collect();
            format!("[{}]", dims_str.join(", "))
        },
        None => "[unknown]".to_string(),
    }
}
