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
    
    // Example: Create a collection
    let collection_name = format!("test_collection_{}", fastrand::u64(0..1000));
    println!("\nCreating collection '{}'...", collection_name);
    
    let create_result = client.create_collection(
        collection_name.clone(),
        384,
        "cosine".to_string(),
    ).await?;
    
    if create_result.success {
        println!("Collection created successfully");
    } else {
        println!("Failed to create collection: {}", create_result.message);
        return Ok(());
    }
    
    // Example: Index some files
    println!("\nIndexing current directory...");
    let index_result = client.index_files(
        collection_name.clone(),
        vec![".".to_string()], // Index current directory
        vec!["rs".to_string()], // Only Rust files
    ).await?;
    
    if index_result.success {
        println!("Indexed {} files with {} chunks",
            index_result.indexed_files,
            index_result.indexed_chunks);
    } else {
        println!("Failed to index files: {}", index_result.message);
    }
    
    // Example: Query the collection
    if index_result.indexed_chunks > 0 {
        println!("\nQuerying for 'client implementation'...");
        let query_result = client.query_collection(
            collection_name.clone(),
            "client implementation".to_string(),
            5, // Limit to 5 results
            None,
            None,
        ).await?;
        
        println!("Found {} results in {:.2}ms",
            query_result.total_results,
            query_result.query_time_ms);
        
        for (i, result) in query_result.results.into_iter().enumerate() {
            println!("\nResult #{}:", i + 1);
            println!("  File: {}", result.file_path);
            println!("  Lines: {}-{}", result.start_line, result.end_line);
            println!("  Language: {}", result.language);
            println!("  Type: {}", result.element_type);
            println!("  Score: {:.4}", result.score);
            println!("  Content: {}", result.content.lines().next().unwrap_or_default());
        }
    }
    
    // Clean up - delete the collection
    println!("\nDeleting collection '{}'...", collection_name);
    let delete_result = client.delete_collection(collection_name).await?;
    
    if delete_result.success {
        println!("Collection deleted successfully");
    } else {
        println!("Failed to delete collection: {}", delete_result.message);
    }
    
    Ok(())
} 