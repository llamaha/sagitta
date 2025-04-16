# Server Mode: Setup and Usage Guide

VectorDB-CLI supports running as a server with a gRPC API, enabling you to integrate the semantic code search functionality into your own applications. This guide explains how to set up and use the server mode, along with client examples.

## Table of Contents

- [Table of Contents](#table-of-contents)
- [Building with Server Support](#building-with-server-support)
- [Starting the Server](#starting-the-server)
- [Server Configuration](#server-configuration)
- [Client Options](#client-options)
- [API Overview](#api-overview)
- [Code Examples](#code-examples)
  - [Basic Client Connection](#basic-client-connection)
  - [Repository Management](#repository-management)
  - [Collection Management](#collection-management)
  - [Searching Code](#searching-code)
- [Running the Example Client](#running-the-example-client)
- [Troubleshooting](#troubleshooting)

## Building with Server Support

Before you can use server mode, you must compile vectordb-cli with the server feature enabled:

```bash
# Basic server support
cargo build --release --features server

# Server with GPU support (CUDA on Linux)
cargo build --release --features ort/cuda,server

# Server with GPU support (Metal on macOS)
cargo build --release --features ort/coreml,server
```

The resulting binary will include the server functionality, which isn't available in the default build.

For a comprehensive reference of all available compilation options and feature flags, see the [Compilation Options Guide](./compile_options.md).

## Starting the Server

You can start VectorDB-CLI in server mode using the following command:

```bash
vectordb-cli server start --host 127.0.0.1 --port 50051
```

By default, if you don't specify host and port, the server will bind to `127.0.0.1:50051`.

**Important**: Note that the `start` subcommand is required. Using just `vectordb-cli server` without the `start` subcommand will show the help menu.

### Server Command Options

The `server start` command accepts the following options:

- `--host <host>`: Host address to bind to (default: `0.0.0.0`)
- `--port <port>`: Port to listen on (default: `50051`)
- `--api-key <key>`: API key for client authentication
- `--api-key-file <path>`: Alternatively, path to a file containing the API key
- `--require-auth`: Require authentication for all requests
- `--tls-cert <path>`: Path to TLS certificate file (for HTTPS)
- `--tls-key <path>`: Path to TLS private key file (for HTTPS)
- `--tls`: Enable TLS/HTTPS
- `--max-concurrent-requests <num>`: Maximum number of concurrent requests (default: 100)

*Note*: Currently TLS support is temporarily disabled for compilation reasons. This will be re-enabled in a future release.

## Server Configuration

The server uses the same configuration sources as the CLI:

1. Command-line arguments
2. Environment variables
3. Configuration file (`config.toml`)

The server requires:
- A running Qdrant instance (configured via `qdrant_url` in config or `QDRANT_URL` env var)
- ONNX model and tokenizer paths (if you intend to use indexing/querying features)

### Git Repository Access

When running in server mode, there are important considerations for Git repository access:

1. **Use HTTPS URLs**: Prefer HTTPS URLs over SSH for repositories when possible
   ```
   # Preferred format for server mode
   https://github.com/organization/repository.git
   
   # Avoid in server mode unless SSH keys are configured
   git@github.com:organization/repository.git
   ```

2. **Configure SSH Keys Explicitly**: If you must use SSH URLs, explicitly configure SSH keys:
   - When adding a repository via API: include `ssh_key_path` in the request
   - In the configuration file: add `ssh_key_path` to the repository configuration
   
   ```toml
   [[repositories]]
   name = "my-repo"
   url = "git@github.com:organization/repository.git"
   local_path = "/path/to/repo"
   ssh_key_path = "/path/to/private_key"
   # ssh_key_passphrase = "optional-passphrase"
   ```

3. **Server Behavior**: The server will automatically reject SSH URLs that don't have explicitly configured SSH keys to avoid hanging on password prompts.

## Client Options

VectorDB offers multiple ways to connect to a running server:

1. **Official Rust Client**: Use the `vectordb-client` crate for a full-featured client with type safety and error handling.
2. **Generated gRPC Client**: Use the protobuf definitions to generate a client in your preferred language.
3. **grpcurl**: Use the grpcurl command-line tool for testing the API directly.

For detailed examples of using the gRPC interface from various languages, see the [gRPC Interface Documentation](./grpc_interface.md).

## API Overview

The VectorDB server provides the following main functionalities:

### Server Information
- `get_server_info()`: Get server version, health status, and model information

### Collection Management
- `create_collection(name, vector_size, distance)`: Create a new vector collection
- `list_collections()`: List all available collections
- `delete_collection(name)`: Delete a collection
- `clear_collection(name)`: Clear all vectors in a collection

### Indexing and Searching
- `index_files(collection_name, paths, extensions)`: Index files into a collection
- `query_collection(collection_name, query_text, limit, language_filter, element_type_filter)`: Perform semantic search

### Repository Management
- `add_repository(url, ...)`: Add a Git repository to be managed
- `list_repositories()`: List all managed repositories
- `use_repository(name)`: Set a repository as active
- `remove_repository(name)`: Remove a managed repository
- `sync_repository(name, extensions, force)`: Update the index for a repository
- `use_branch(branch_name, repository_name)`: Switch to a different branch

## Code Examples

### Basic Client Connection

```rust
// Import the client from the dedicated crate
use vectordb_client::VectorDBClient;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Connect to the server
    let mut client = VectorDBClient::connect("http://127.0.0.1:50051").await?;
    
    // For TLS connections:
    // let mut client = VectorDBClient::new(
    //     ClientConfig::new("https://127.0.0.1:50051")
    //         .with_tls(true)
    // ).await?;
    
    // Get server information
    let server_info = client.get_server_info().await?;
    println!("Connected to server version: {}", server_info.version);
    println!("Server is healthy: {}", server_info.is_healthy);
    
    Ok(())
}
```

### Repository Management

```rust
use vectordb_client::VectorDBClient;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Create a client with default configuration (http://localhost:50051)
    let mut client = VectorDBClient::default().await?;
    
    // Add a new repository
    let add_result = client.add_repository(
        "https://github.com/example/repo.git".to_string(), // URL
        None,                                              // Local path (optional)
        Some("my-project".to_string()),                    // Name
        Some("main".to_string()),                          // Branch
        None,                                              // Remote (optional)
        None,                                              // SSH key path (optional)
        None,                                              // SSH passphrase (optional)
    ).await?;
    
    println!("Add repo result: {}", add_result.message);
    
    // List repositories
    let repos = client.list_repositories().await?;
    println!("Repositories:");
    for repo in repos.repositories {
        println!(" - {}: {}", repo.name, repo.url);
        println!("   Active branch: {}", repo.active_branch);
    }
    
    // Set active repository
    let use_result = client.use_repository("my-project".to_string()).await?;
    println!("Set active repo: {}", use_result.message);
    
    // Sync repository
    let sync_result = client.sync_repository(
        Some("my-project".to_string()),  // Repository name
        vec!["rs".to_string(), "py".to_string()], // Extensions to index
        false,                           // Force flag
    ).await?;
    
    println!("Sync result: {}", sync_result.message);
    
    Ok(())
}
```

### Collection Management

```rust
use vectordb_client::VectorDBClient;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Create a client with default configuration
    let mut client = VectorDBClient::default().await?;
    
    // Create a collection
    let create_result = client.create_collection(
        "my_collection".to_string(),  // Collection name
        384,                          // Vector size (depends on the model)
        "cosine".to_string(),         // Distance function (cosine, euclidean, dot)
    ).await?;
    
    println!("Collection created: {}", create_result.message);
    
    // List all collections
    let collections = client.list_collections().await?;
    println!("Available collections:");
    for collection in collections.collections {
        println!("  - {}", collection);
    }
    
    // Index files into the collection
    let index_result = client.index_files(
        "my_collection".to_string(),              // Collection name
        vec!["/path/to/source/code".to_string()], // Paths to index
        vec!["rs".to_string(), "py".to_string()], // File extensions to include
    ).await?;
    
    println!("Indexed {} files with {} chunks", 
             index_result.indexed_files, 
             index_result.indexed_chunks);
    
    // Delete a collection
    let delete_result = client.delete_collection("my_collection".to_string()).await?;
    println!("Collection deleted: {}", delete_result.message);
    
    Ok(())
}
```

### Searching Code

```rust
use vectordb_client::VectorDBClient;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut client = VectorDBClient::default().await?;
    
    // Query a collection
    let results = client.query_collection(
        "my_collection".to_string(),                     // Collection name
        "implement a function to parse JSON".to_string(), // Query text
        10,                                              // Limit
        Some("python".to_string()),                      // Optional language filter
        Some("function".to_string()),                    // Optional element type filter
    ).await?;
    
    println!("Found {} results in {:.2}ms", 
             results.total_results, 
             results.query_time_ms);
    
    // Process results
    for (i, result) in results.results.into_iter().enumerate() {
        println!("\nResult #{}:", i + 1);
        println!("  File: {}", result.file_path);
        println!("  Lines: {}-{}", result.start_line, result.end_line);
        println!("  Language: {}", result.language);
        println!("  Type: {}", result.element_type);
        println!("  Score: {:.4}", result.score);
        
        // Print first few lines of content
        let preview = result.content.lines()
            .take(3)
            .collect::<Vec<_>>()
            .join("\n  ");
        println!("  Preview:\n  {}", preview);
        
        // If this is a repository result, show branch info
        if let Some(branch) = result.branch {
            println!("  Branch: {}", branch);
        }
    }
    
    Ok(())
}
```

## Running the Example Client

The project includes several example applications that demonstrate how to use the client API:

1. Make sure the server is running first:
   ```bash
   # Terminal 1
   vectordb-cli server start
   ```

2. In another terminal, run one of the examples:
   ```bash
   # Terminal 2
   cargo run --example server_test --features=server
   ```

For more comprehensive examples, check the `examples` directory in the `vectordb-client` crate.

## Troubleshooting

### Connection Issues

1. Ensure the server is running with `ps aux | grep vectordb-cli`
2. Check the server address and port match your client configuration
3. Verify TLS settings if using secure connections
4. Confirm API key configuration if authentication is enabled

### API Errors

Common API errors include:

1. **Collection not found**: Verify the collection name exists with `list_collections()`
2. **Repository not found**: Check repository names with `list_repositories()`
3. **Embedding errors**: Ensure ONNX model and tokenizer paths are correctly configured

For more detailed help, run the server with debug logging enabled:

```bash
RUST_LOG=debug vectordb-cli server
```