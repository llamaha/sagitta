use vectordb_client::VectorDBClient;
use std::error::Error;

/// A simple test program that connects to a VectorDB server and demonstrates
/// basic functionality. This assumes a server is already running.
///
/// To run this example:
/// 1. Start the server in another terminal with:
///    `vectordb-cli server start`
/// 2. Run this example:
///    `cargo run --example server_test --features=server`
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Create a client
    let mut client = VectorDBClient::default().await?;
    
    // Get server info
    let server_info = client.get_server_info().await?;
    println!("Connected to server!");
    println!("Version: {}", server_info.version);
    println!("Build date: {}", server_info.build_date);
    println!("Is healthy: {}", server_info.is_healthy);
    
    if let Some(model_info) = server_info.model_info {
        println!("\nModel information:");
        println!("Model path: {}", model_info.model_path);
        println!("Tokenizer path: {}", model_info.tokenizer_path);
        println!("Vector dimension: {}", model_info.vector_dimension);
        println!("Model type: {}", model_info.model_type);
    }
    
    // List collections
    let collections = client.list_collections().await?;
    println!("\nCollections:");
    if collections.collections.is_empty() {
        println!("No collections found");
    } else {
        for collection in collections.collections {
            println!("- {}", collection);
        }
    }
    
    // List repositories
    let repos = client.list_repositories().await?;
    println!("\nRepositories:");
    if repos.repositories.is_empty() {
        println!("No repositories found");
    } else {
        for repo in repos.repositories {
            let active = if repo.is_active { " (active)" } else { "" };
            println!("- {}{}", repo.name, active);
            println!("  URL: {}", repo.url);
            println!("  Branch: {}", repo.active_branch);
            println!("  Local path: {}", repo.local_path);
        }
        
        if let Some(active_repo) = repos.active_repository {
            println!("\nActive repository: {}", active_repo);
        }
    }
    
    println!("\nServer test completed successfully!");
    Ok(())
} 