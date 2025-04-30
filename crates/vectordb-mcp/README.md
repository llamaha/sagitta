# VectorDB MCP Server (`vectordb-mcp`)

This crate implements a server adhering to the Multi-purpose Code Protocol (MCP), designed to provide an interface for interacting with the core functionalities of `vectordb-core`, such as managing Git repositories, indexing their content, and performing semantic searches.

The server communicates via JSON-RPC 2.0 messages over standard input (stdin) and standard output (stdout). It typically processes one request and sends one response before exiting.

## Prerequisites

1.  **Build `vectordb` Workspace:** This server is part of the `vectordb` workspace. Follow the main build and installation instructions in the [root README.md](../../README.md), ensuring you enable necessary features like `ort` and `cuda` if needed.

2.  **ONNX Runtime (if using `ort` feature):** If you enable the `ort` feature for ONNX model support, you must have the ONNX Runtime library installed on your system separately. Download it from the [official ONNX Runtime releases page](https://github.com/microsoft/onnxruntime/releases).

3.  **Qdrant:** A running Qdrant vector database instance must be accessible at the URL specified in the configuration.

4.  **`vectordb-cli` (Recommended):** While MCP provides a programmatic interface, the `vectordb-cli` tool (built from the same workspace, see [root README.md](../../README.md)) is recommended for initial setup, configuration management, and easier handling of embedding model downloads. Building `vectordb-cli` often handles the necessary model setup that MCP relies on.

## Configuration

The server requires a configuration file, typically named `mcp.json`, located in the directory where the `vectordb-mcp` command is executed.

**Example `mcp.json`:**

```json
{
  "qdrant_url": "http://localhost:6334",
  "repositories_base_path": "repositories",
  "model_dir": null,
  "model_source": {
      "model_type": "sentence-transformers",
      "model_name": "all-MiniLM-L6-v2"
  }
}
```

*   `qdrant_url`: The URL of your running Qdrant instance.
*   `repositories_base_path`: The directory where cloned Git repositories will be stored. Relative paths are interpreted relative to the server's working directory.
*   `model_dir`: (Optional) Path to a directory containing pre-downloaded ONNX models. If `null`, `model_source` is used.
*   `model_source`: Specifies the embedding model to use if `model_dir` is not set.
    *   `model_type`: Type of model (e.g., "sentence-transformers").
    *   `model_name`: Name of the model (e.g., "all-MiniLM-L6-v2").

*(Note: The server might load a more comprehensive configuration structure internally, including repository-specific details, often managed via the CLI or previous MCP commands)*

## Running the Server

The server reads a single JSON-RPC request from stdin, processes it, writes the JSON-RPC response to stdout, and then exits.

Pipe the request JSON into the server process:

```bash
echo '<JSON_REQUEST>' | ./target/release/vectordb-mcp | cat
```

Replace `<JSON_REQUEST>` with the actual request object.

**Environment Variables (CUDA/ORT):**

If you built with `ort,cuda` features and encounter shared library errors (e.g., `libonnxruntime.so` not found), you may need to prepend `LD_LIBRARY_PATH`:

```bash
echo '<JSON_REQUEST>' | LD_LIBRARY_PATH=./target/release/lib:$LD_LIBRARY_PATH ./target/release/vectordb-mcp | cat
```
*(Adjust the path `./target/release/lib` if necessary based on your build output)*

## Capabilities (JSON-RPC Methods)

The server implements the following core MCP methods:

*   `initialize`: Standard MCP method to initialize the connection and exchange capabilities (basic implementation).
*   `ping`: Checks if the server is running and responsive. Returns `{"message": "pong"}`.
*   `repository/add`: Adds a new repository configuration. Clones the repository if it doesn't exist locally. Requires `name` and `url` parameters.
*   `repository/list`: Lists all repositories currently configured in the server.
*   `repository/sync`: Fetches updates for a repository's active branch, indexes the content using the configured embedding model, and stores the vectors in Qdrant. Requires `name`.
*   `repository/remove`: Removes a repository's configuration from the server and attempts to delete associated data (Qdrant collection, local files - though local deletion might be skipped for safety). Requires `name`.
*   `query`: Performs a semantic search against an indexed repository. Requires `repository_name`, `query_text`. Optional: `limit`, `branch_name`.

*(Refer to `mcp-e2e-test-plan.md` or the MCP specification for detailed request/response formats.)*

## Future Development

This crate might be moved into its own dedicated Git repository in the future to decouple it further from the `vectordb-cli` tool.

## Development & Testing

*   **Build:** See [root README.md](../../README.md)
*   **Test:** `cargo test -p vectordb-mcp` 