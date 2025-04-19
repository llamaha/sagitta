# VectorDB gRPC Interface Documentation

VectorDB provides a gRPC API that allows you to interact with the service from any language. This document provides examples of using the API with different tools and client libraries.

## gRPC Service Definition

VectorDB offers two main services:
- `VectorDBService` - provides methods for managing collections, indexing files, and performing searches
- `EditingService` - provides methods for precise code editing with semantic understanding

The VectorDB service is defined in the `vectordb.proto` file. The main service is `VectorDBService`, which provides methods for managing collections, indexing files, and performing searches.

## Using grpcurl

[grpcurl](https://github.com/fullstorydev/grpcurl) is a command-line tool that lets you interact with gRPC servers. It's like curl, but for gRPC.

### Installation

```bash
# MacOS with Homebrew
brew install grpcurl

# Ubuntu/Debian - download and install the binary
curl -L https://github.com/fullstorydev/grpcurl/releases/download/v1.8.9/grpcurl_1.8.9_linux_x86_64.tar.gz | sudo tar -xz -C /usr/local/bin

# Or using pre-built package on newer Ubuntu versions
sudo apt update && sudo apt install -y grpcurl

# Alternative: Install using Go
go install github.com/fullstorydev/grpcurl/cmd/grpcurl@latest
```

### Server Information

Get information about the server:

```bash
grpcurl -plaintext localhost:50051 vectordb.VectorDBService/GetServerInfo
```

Response:
```json
{
  "version": "1.3.3-stable",
  "buildDate": "2023-09-15",
  "isHealthy": true,
  "modelInfo": {
    "modelPath": "onnx/all-minilm-l12-v2.onnx",
    "tokenizerPath": "onnx/minilm_tokenizer.json",
    "vectorDimension": 384,
    "modelType": "MINILM"
  }
}
```

### Collection Management

List all collections:

```bash
grpcurl -plaintext localhost:50051 vectordb.VectorDBService/ListCollections
```

Create a collection:

```bash
grpcurl -plaintext -d '{
  "name": "my_collection",
  "vector_size": 384,
  "distance": "cosine"
}' localhost:50051 vectordb.VectorDBService/CreateCollection
```

Delete a collection:

```bash
grpcurl -plaintext -d '{
  "name": "my_collection"
}' localhost:50051 vectordb.VectorDBService/DeleteCollection
```

### Indexing Files

Index files in a collection:

```bash
grpcurl -plaintext -d '{
  "collection_name": "my_collection",
  "paths": ["/path/to/source/code"],
  "extensions": ["rs", "py", "js"]
}' localhost:50051 vectordb.VectorDBService/IndexFiles
```

### Searching

Query a collection:

```bash
grpcurl -plaintext -d '{
  "collection_name": "my_collection",
  "query_text": "function to handle errors",
  "limit": 5
}' localhost:50051 vectordb.VectorDBService/QueryCollection
```

### Repository Management

Add a Git repository:

```bash
grpcurl -plaintext -d '{
  "url": "https://github.com/example/repo.git",
  "name": "example-repo",
  "branch": "main"
}' localhost:50051 vectordb.VectorDBService/AddRepository
```

List repositories:

```bash
grpcurl -plaintext localhost:50051 vectordb.VectorDBService/ListRepositories
```

### Code Editing

The EditingService provides two key methods for code editing:

Validate an edit without applying it:

```bash
grpcurl -plaintext -d '{
  "file_path": "/path/to/file.py",
  "target": {
    "semantic_element": {
      "element_query": "class:MyClass"
    }
  },
  "content": "class MyClass:\n    def __init__(self):\n        pass\n\n    def new_method(self):\n        return True",
  "options": {
    "format_code": false,
    "update_references": false
  }
}' localhost:50051 editing.EditingService/ValidateEdit
```

Apply a validated edit:

```bash
grpcurl -plaintext -d '{
  "file_path": "/path/to/file.py",
  "target": {
    "semantic_element": {
      "element_query": "class:MyClass"
    }
  },
  "content": "class MyClass:\n    def __init__(self):\n        pass\n\n    def new_method(self):\n        return True",
  "options": {
    "format_code": false,
    "update_references": false
  }
}' localhost:50051 editing.EditingService/EditCode
```

You can also use line-based targeting by changing the target field:

```bash
grpcurl -plaintext -d '{
  "file_path": "/path/to/file.py",
  "target": {
    "line_range": {
      "start_line": 10,
      "end_line": 15
    }
  },
  "content": "    def new_method(self):\n        return True",
  "options": {
    "format_code": false,
    "update_references": false
  }
}' localhost:50051 editing.EditingService/EditCode
```

## Client Libraries

### Python Client

You can use the `grpcio` package to interact with the VectorDB service:

```python
import grpc
import vectordb_pb2
import vectordb_pb2_grpc

def main():
    # Create a gRPC channel
    with grpc.insecure_channel('localhost:50051') as channel:
        # Create a stub (client)
        stub = vectordb_pb2_grpc.VectorDBServiceStub(channel)
        
        # Get server info
        response = stub.GetServerInfo(vectordb_pb2.Empty())
        print(f"Server version: {response.version}")
        print(f"Server is healthy: {response.is_healthy}")
        
        # List collections
        collections = stub.ListCollections(vectordb_pb2.Empty())
        print("Collections:")
        for collection in collections.collections:
            print(f"  - {collection}")
        
        # Query a collection
        query_response = stub.QueryCollection(vectordb_pb2.QueryRequest(
            collection_name="my_collection",
            query_text="function to handle errors",
            limit=5
        ))
        
        print(f"Found {query_response.total_results} results in {query_response.query_time_ms}ms")
        for result in query_response.results:
            print(f"  {result.file_path} (Lines {result.start_line}-{result.end_line})")
            print(f"  Score: {result.score}")
            print(f"  Language: {result.language}")
            print(f"  Content: {result.content[:100]}...")

if __name__ == '__main__':
    main()
```

### Generate Python Client

To generate Python client code from the proto file:

```bash
python -m pip install grpcio-tools
python -m grpc_tools.protoc -I. --python_out=. --grpc_python_out=. vectordb.proto
```

### Node.js Client

For Node.js, you can use the `@grpc/grpc-js` package:

```javascript
const grpc = require('@grpc/grpc-js');
const protoLoader = require('@grpc/proto-loader');

// Load the proto file
const packageDefinition = protoLoader.loadSync('vectordb.proto', {
  keepCase: true,
  longs: String,
  enums: String,
  defaults: true,
  oneofs: true
});

const vectordb = grpc.loadPackageDefinition(packageDefinition).vectordb;

function main() {
  // Create a client
  const client = new vectordb.VectorDBService(
    'localhost:50051',
    grpc.credentials.createInsecure()
  );
  
  // Get server info
  client.getServerInfo({}, (err, response) => {
    if (err) {
      console.error('Error:', err);
      return;
    }
    
    console.log(`Server version: ${response.version}`);
    console.log(`Server is healthy: ${response.isHealthy}`);
    
    // List collections
    client.listCollections({}, (err, response) => {
      if (err) {
        console.error('Error:', err);
        return;
      }
      
      console.log('Collections:');
      if (response.collections.length === 0) {
        console.log('  No collections found');
      } else {
        response.collections.forEach(collection => {
          console.log(`  - ${collection}`);
        });
      }
    });
  });
}

main();
```

### Generate Node.js Client

To generate Node.js client code:

```bash
npm install -g @grpc/proto-loader grpc-tools
grpc_tools_node_protoc --js_out=import_style=commonjs,binary:. --grpc_out=grpc_js:. vectordb.proto
```

### Go Client

Go example using the standard gRPC library:

```go
package main

import (
	"context"
	"fmt"
	"log"
	"time"

	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	pb "path/to/generated/vectordb"
)

func main() {
	// Set up a connection to the server
	conn, err := grpc.Dial("localhost:50051", grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		log.Fatalf("Failed to connect: %v", err)
	}
	defer conn.Close()

	// Create a client
	client := pb.NewVectorDBServiceClient(conn)

	// Set a timeout for our API call
	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()

	// Get server info
	info, err := client.GetServerInfo(ctx, &pb.Empty{})
	if err != nil {
		log.Fatalf("Could not get server info: %v", err)
	}
	fmt.Printf("Server version: %s\n", info.Version)
	fmt.Printf("Server is healthy: %t\n", info.IsHealthy)
	
	// List collections
	collections, err := client.ListCollections(ctx, &pb.Empty{})
	if err != nil {
		log.Fatalf("Could not list collections: %v", err)
	}
	
	fmt.Println("Collections:")
	for _, collection := range collections.Collections {
		fmt.Printf("  - %s\n", collection)
	}
}
```

### Generate Go Client

To generate Go client code:

```bash
# Install protoc plugin for Go
go install google.golang.org/protobuf/cmd/protoc-gen-go@latest
go install google.golang.org/grpc/cmd/protoc-gen-go-grpc@latest

# Generate code
protoc --go_out=. --go_opt=paths=source_relative \
       --go-grpc_out=. --go-grpc_opt=paths=source_relative \
       vectordb.proto
```

## Using the Official Rust Client

VectorDB provides an official Rust client crate that simplifies connecting to the service:

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
    
    // Query a collection
    let results = client.query_collection(
        "my_collection".to_string(),
        "function to handle errors".to_string(),
        5,
        None,
        None,
    ).await?;
    
    println!("Found {} results in {:.2}ms", results.total_results, results.query_time_ms);
    
    for result in results.results {
        println!("\nResult:");
        println!("  File: {}", result.file_path);
        println!("  Lines: {}-{}", result.start_line, result.end_line);
        println!("  Score: {:.4}", result.score);
        println!("  Language: {}", result.language);
        println!("  Content: {}", result.content.lines().next().unwrap_or_default());
    }
    
    Ok(())
}
``` 