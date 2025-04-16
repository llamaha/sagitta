// This example demonstrates how to use the vectordb-cli and vectordb-client crates

use std::error::Error;
// When using the client, it's recommended to use the dedicated crate
use vectordb_client::VectorDBClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Create a client with default configuration (localhost:50051)
    let mut client = VectorDBClient::default().await?;
    
    // Get server info
    let server_info = client.get_server_info().await?;
    println!("Connected to server version: {}", server_info.version);
    
    // List collections
    let collections = client.list_collections().await?;
    println!("Available collections:");
    for collection in collections.collections {
        println!("  - {}", collection);
    }
    
    // Create a test collection
    let result = client.create_collection(
        "test_collection".to_string(),
        384,
        "cosine".to_string()
    ).await?;
    
    if result.success {
        println!("Collection created successfully: {}", result.message);
    } else {
        println!("Failed to create collection: {}", result.message);
    }
    
    // Clean up: delete the test collection
    let result = client.delete_collection("test_collection".to_string()).await?;
    
    if result.success {
        println!("Collection deleted successfully: {}", result.message);
    } else {
        println!("Failed to delete collection: {}", result.message);
    }
    
    Ok(())
}