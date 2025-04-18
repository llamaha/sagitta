use vectordb_client::VectorDBClient;
use std::error::Error;
use tempfile::tempdir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Create a client with default configuration (localhost:50051)
    let mut client = VectorDBClient::default().await?;
    
    // Get server info
    let server_info = client.get_server_info().await?;
    println!("Connected to server version: {}", server_info.version);
    println!("Server is healthy: {}", server_info.is_healthy);
    
    // List existing repositories
    let repos = client.list_repositories().await?;
    println!("\nExisting repositories:");
    if repos.repositories.is_empty() {
        println!("  No repositories found");
    } else {
        for repo in repos.repositories {
            println!("  - {}: {}", repo.name, repo.url);
            println!("    Branch: {}", repo.active_branch);
        }
    }
    
    // Create a temporary directory for the repository
    let temp_dir = tempdir()?;
    println!("\nCreated temporary directory for clone: {:?}", temp_dir.path());
    
    // Example: Add a repository
    // For this example, we'll use the VectorDB-CLI repo itself
    // In a real application, you might use your own repository
    let repo_name = format!("test_repo_{}", fastrand::u64(0..1000));
    println!("\nAdding repository '{}' from GitHub...", repo_name);
    
    let add_result = client.add_repository(
        "https://github.com/rust-lang/rust-analyzer.git".to_string(), // Public repo URL
        Some(temp_dir.path().to_string_lossy().to_string()), // Clone to temp dir
        Some(repo_name.clone()),                             // Repository name
        Some("master".to_string()),                          // Branch
        None,                                                // Default remote name
        None,                                                // No SSH key
        None,                                                // No passphrase
    ).await?;
    
    if add_result.success {
        println!("Repository added successfully: {}", add_result.message);
    } else {
        println!("Failed to add repository: {}", add_result.message);
        return Ok(());
    }
    
    // List repositories after adding
    let repos = client.list_repositories().await?;
    println!("\nRepositories after adding:");
    for repo in repos.repositories {
        println!("  - {}: {}", repo.name, repo.url);
        println!("    Branch: {}", repo.active_branch);
        println!("    Is active: {}", repo.is_active);
    }
    
    // Set as active repository
    println!("\nSetting as active repository...");
    let use_result = client.use_repository(repo_name.clone()).await?;
    
    if use_result.success {
        println!("Repository set as active: {}", use_result.message);
    } else {
        println!("Failed to set as active: {}", use_result.message);
    }
    
    // List repositories to confirm active status
    let repos = client.list_repositories().await?;
    println!("\nActive repository:");
    println!("  {}", repos.active_repository.unwrap_or_else(|| "None".to_string()));
    
    // Sync repository
    println!("\nSyncing repository...");
    let sync_result = client.sync_repository(
        Some(repo_name.clone()),                        // Repository name
        vec!["rs".to_string(), "md".to_string()],       // Index Rust and Markdown files
        false,                                          // Don't force full rescan
    ).await?;
    
    if sync_result.success {
        println!("Repository synced: {}", sync_result.message);
    } else {
        println!("Failed to sync: {}", sync_result.message);
    }
    
    // Query the repository
    println!("\nQuerying for 'rust analyzer'...");
    let query_result = client.query_collection(
        repo_name.clone(),                               // Repository name is the collection name
        "analyze rust code".to_string(),                 // Query text
        5,                                               // Limit to 5 results
        None,                                            // No language filter
        None,                                            // No element type filter
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
        println!("  Branch: {}", result.branch.unwrap_or_else(|| "unknown".to_string()));
        println!("  Content: {}", result.content.lines().next().unwrap_or_default());
    }
    
    // Clean up - remove the repository
    println!("\nRemoving repository...");
    let remove_result = client.remove_repository(repo_name, true).await?;
    
    if remove_result.success {
        println!("Repository removed successfully: {}", remove_result.message);
    } else {
        println!("Failed to remove repository: {}", remove_result.message);
    }
    
    Ok(())
} 