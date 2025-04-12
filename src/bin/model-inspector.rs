use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use std::time::Instant;

// Use the library for embedding logic
use vectordb_lib::vectordb::embedding::EmbeddingModel;

/// A simple utility to test ONNX model loading and embedding generation.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the ONNX model file.
    #[arg(short, long)]
    model: PathBuf,

    /// Path to the tokenizer configuration file.
    #[arg(short, long)]
    tokenizer: PathBuf,

    /// Text to embed.
    #[arg(short, long)]
    text: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!(
        "Loading model: {}",
        args.model.display()
    );
    println!(
        "Loading tokenizer: {}",
        args.tokenizer.display()
    );

    let start_load = Instant::now();
    // Use new_onnx constructor
    let model = match EmbeddingModel::new_onnx(&args.model, &args.tokenizer) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error initializing ONNX model: {}", e);
            std::process::exit(1);
        }
    };
    let load_duration = start_load.elapsed();
    println!("Model loaded in {:.2?}", load_duration);

    println!("Embedding text: \"{}\"", args.text);
    let start_embed = Instant::now();
    let embedding = model
        .embed(&args.text)
        .context("Failed to generate embedding")?;
    let embed_duration = start_embed.elapsed();

    println!("Embedding generated in {:.2?}", embed_duration);
    println!("Embedding dimension: {}", embedding.len());
    println!(
        "Embedding preview (first 6 values): {:?}...",
        embedding.iter().take(6).collect::<Vec<_>>()
    );

    Ok(())
}
