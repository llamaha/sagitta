use vectordb_client::VectorDBClient;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Create a client with default configuration (localhost:50051)
    let mut client = VectorDBClient::default().await?;
    
    // Get server info
    let server_info = client.get_server_info().await?;
    println!("Connected to server version: {}", server_info.version);
    println!("Server is healthy: {}", server_info.is_healthy);
    
    if let Some(model_info) = server_info.model_info {
        println!("Model information:");
        println!("  Model path: {}", model_info.model_path);
        println!("  Tokenizer path: {}", model_info.tokenizer_path);
        println!("  Vector dimension: {}", model_info.vector_dimension);
        println!("  Model type: {}", model_info.model_type);
    }
    
    // List collections
    let collections = client.list_collections().await?;
    println!("\nAvailable collections:");
    if collections.collections.is_empty() {
        println!("  No collections found");
    } else {
        for collection in collections.collections {
            println!("  - {}", collection);
        }
    }
    
    Ok(())
} 