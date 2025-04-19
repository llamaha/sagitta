# vectordb-client

A Rust client library for interacting with the VectorDB semantic code search service.

## Features

- Easy connection to VectorDB servers
- Full API coverage for all VectorDB operations
- Type-safe API with proper error handling
- TLS support for secure connections
- API key authentication
- Code editing with semantic understanding

## Usage

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
vectordb-client = "0.1.0"
tokio = { version = "1.0", features = ["full"] }
```

### Basic Example

```rust
use vectordb_client::VectorDBClient;
use std::error::Error;

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
        println!("Collection created successfully");
    } else {
        println!("Failed to create collection: {}", result.message);
    }
    
    Ok(())
}
```

### Custom Configuration

```rust
use vectordb_client::{VectorDBClient, ClientConfig};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Create a custom client configuration
    let config = ClientConfig::new("http://my-server.example.com:50051")
        .with_tls(true)
        .with_api_key("my-api-key");
    
    // Connect with the custom configuration
    let mut client = VectorDBClient::new(config).await?;
    
    // Use the client...
    
    Ok(())
}
```

### Code Editing Example

```rust
use vectordb_client::VectorDBClient;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut client = VectorDBClient::default().await?;

    // Edit a specific function by element name
    let result = client.edit_file_by_element(
        "src/main.rs".to_string(),                      // File path
        "function:process_data".to_string(),            // Element query
        "fn process_data(input: &str) -> String {\n    // Add better documentation\n    format!(\"processed: {}\", input)\n}".to_string(), // New content
        true,   // Format the code
        false,  // Don't update references
    ).await?;
    
    if result.success {
        println!("Edit applied successfully!");
    } else {
        println!("Edit failed: {}", result.error_message.unwrap_or_default());
    }
    
    // Edit specific lines in a file
    let result = client.edit_file_by_lines(
        "src/lib.rs".to_string(),       // File path
        10, 15,                         // Start and end line (inclusive)
        "    // Replace these lines with a comment".to_string(), // New content
        false,  // Don't format
        false,  // Don't update references
    ).await?;
    
    if result.success {
        println!("Line edit applied successfully!");
    } else {
        println!("Line edit failed: {}", result.error_message.unwrap_or_default());
    }
    
    Ok(())
}
```

## API Documentation

The client provides methods for all VectorDB operations:

### Server Operations

- `get_server_info()` - Get information about the server

### Collection Management

- `create_collection(name, vector_size, distance)` - Create a new collection
- `list_collections()` - List all collections
- `delete_collection(name)` - Delete a collection
- `clear_collection(name)` - Clear a collection

### Indexing and Searching

- `index_files(collection_name, paths, extensions)` - Index files into a collection
- `query_collection(collection_name, query_text, limit, language, element_type)` - Search a collection

### Repository Management

- `add_repository(url, local_path, name, branch, remote, ssh_key_path, ssh_passphrase)` - Add a Git repository
- `list_repositories()` - List all repositories
- `use_repository(name)` - Set the active repository
- `remove_repository(name, skip_confirmation)` - Remove a repository
- `sync_repository(name, extensions, force)` - Sync a repository
- `use_branch(branch_name, repository_name)` - Set the active branch

### Code Editing

- `edit_file_by_lines(file_path, start_line, end_line, content, format, update_references)` - Edit a file by line range
- `edit_file_by_element(file_path, element_query, content, format, update_references)` - Edit a semantic element
- `validate_edit_by_lines(file_path, start_line, end_line, content, format, update_references)` - Validate a line edit
- `validate_edit_by_element(file_path, element_query, content, format, update_references)` - Validate an element edit

#### Element Query Syntax

When targeting semantic elements, use the following syntax:

- `function:name` - Target a function
- `class:ClassName` - Target a class
- `method:ClassName.method_name` - Target a class method
- `struct:StructName` - Target a struct (Rust)
- `impl:StructName` - Target an implementation block (Rust)

## License

This project is licensed under the MIT License. 