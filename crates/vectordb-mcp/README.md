# VectorDB MCP Server (`vectordb-mcp`)

This crate implements a server adhering to the Model Context Protocol (MCP), designed to provide an interface for interacting with the core functionalities of `vectordb-core`, such as managing Git repositories, indexing their content, and performing semantic searches.

The server communicates via JSON-RPC 2.0 messages over standard input (stdin) and standard output (stdout).

## Prerequisites

1.  **Build `vectordb` Workspace:** Build the project following the instructions in the [root README.md](../../README.md), ensuring necessary features (`ort`, `cuda`) are enabled and models are downloaded.
2.  **ONNX Runtime (if using `ort` feature):** Ensure the ONNX Runtime library is installed and accessible. See the [Full Setup Guide](../../docs/SETUP.md) for details.
3.  **Qdrant:** A running Qdrant vector database instance must be accessible.

## Configuration

The server loads its configuration (Qdrant URL, repository paths, model settings) from the standard `vectordb-cli` configuration file, typically located at `~/.config/vectordb/vectordb-cli/config.toml`. Ensure this file is correctly configured before running the MCP server.

## Capabilities (JSON-RPC Methods)

The server implements the following core MCP methods:

*   `initialize`: Standard MCP method to initialize the connection and exchange capabilities (basic implementation).
*   `ping`: Checks if the server is running and responsive. Returns `{"message": "pong"}`.
*   `repository/add`: Adds a new repository configuration. Clones the repository if it doesn't exist locally. Requires `name` and `url` (or `local_path`).
    *   Accepts an optional `target_ref` parameter (string). If provided, the server will attempt to `git checkout` this specific ref (tag, commit hash, or branch name) after cloning or locating the existing repository. The provided `target_ref` will be stored in the configuration and used as the identifier for this static version of the repository. Indexing and syncing will then operate *only* on this specific ref, not the default branch head.
    *   **Use Case:** Use `target_ref` to index and query specific versions (tags, commits) of a library or codebase alongside its evolving main branch. For example, add `my-lib` (tracks `main`) and `my-lib-v1.0` (with `target_ref="v1.0"`) to search both the latest code and the tagged v1.0 release.
*   `repository/list`: Lists all repositories currently configured in the server. The `branch` field in the response reflects the `active_branch` stored in the configuration (which will be the `target_ref` for statically versioned repos).
*   `repository/sync`:
    *   **If `target_ref` is configured for the repository:** Checks out the specified static `target_ref`, gets its commit hash, and indexes its content. It **does not** fetch updates from the remote for these static refs.
    *   **If no `target_ref` is configured:** Fetches updates for the repository's active branch, merges changes, indexes the new/modified content, and updates the vector store.
    *   Requires the repository `name`.
*   `repository/remove`: Removes a repository's configuration from the server and attempts to delete associated data (Qdrant collection, local files - though local deletion might be skipped for safety). Requires `name`.
*   `query`: Performs a semantic search against an indexed repository. Requires `repository_name`, `query_text`. Optional: `limit`, `branch_name`. When querying a repository added with `target_ref`, use the specific repository `name` (e.g., `my-lib-v1.0`) to query that version. The `branch_name` parameter is ignored for `target_ref` repositories.

*(Refer to `mcp-e2e-test-plan.md` or the MCP specification for detailed request/response formats.)*

## Future Development

This crate might be moved into its own dedicated Git repository in the future to decouple it further from the `vectordb-cli` tool.

## Development & Testing

*   **Build:** See [root README.md](../../README.md)
*   **Test:** `cargo test -p vectordb-mcp` 

Currently serves as the JSON-RPC backend for `vectordb` commands.

## MCP Integration

Add a new server configuration with the following settings:

```json
{
  "mcpServers": {
    "vectordb-mcp": {
      "command": "/path/to/your/vectordb-workspace/target/release/vectordb-mcp",
      "args": [],
      "cwd": "/path/to/your/vectordb-workspace",
      "env": {
        "LD_LIBRARY_PATH": "/path/to/your/onnxruntime/lib:/optional/path/to/cuda/lib64",
        "RAYON_NUM_THREADS": "8"
      }
    }
  }
}
```

This configuration defines an MCP server named `vectordb-mcp` with the following properties:

*   **`command`**: The *absolute path* to the `vectordb-mcp` executable, found in your workspace's `target/release/` directory.
    *   Example (development): `"/home/user/projects/vectordb/target/release/vectordb-mcp"`
    *   Example (installation): `"/usr/local/bin/vectordb-mcp"` (if installed to PATH)
*   **`args`**: An array of command-line arguments to pass to the executable (currently empty in the example).
*   **`cwd` (Current Working Directory)**: The *absolute path* to the root of your `vectordb` workspace.
    *   Example: `"/home/user/projects/vectordb"`
*   **`env`**: A dictionary for environment variables. 
    *   `LD_LIBRARY_PATH`: Use this to specify paths to necessary shared libraries, primarily the ONNX Runtime libraries. If you have a separate CUDA installation providing cuDNN, include its library path as well. Paths are colon-separated on Linux/macOS. (For Windows, use `PATH` and semicolon separation).
        *   Example: `"/opt/onnxruntime/lib:/usr/local/cuda/lib64"`
    *   `RAYON_NUM_THREADS`: Optionally, set the number of threads for Rayon to use (e.g., `"8"`). This might be beneficial for performance and resource management during parallel operations like indexing.

**Explanation:**

*   The installation process should place the `vectordb-mcp` binary in your system's `PATH` (e.g., `/usr/local/bin` but it's not necessary to call it from here).

All logs and non-JSON-RPC output from the server will be directed to `stderr`. You can monitor your tools logs for communication details and potential errors. 
