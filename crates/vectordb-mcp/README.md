# VectorDB MCP Server (`vectordb-mcp`)

This crate implements a server adhering to the Multi-purpose Code Protocol (MCP), designed to provide an interface for interacting with the core functionalities of `vectordb-core`, such as managing Git repositories, indexing their content, and performing semantic searches.

The server communicates via JSON-RPC 2.0 messages over standard input (stdin) and standard output (stdout).

## Prerequisites

1.  **Build `vectordb` Workspace:** Build the project following the instructions in the [root README.md](../../README.md), ensuring necessary features (`ort`, `cuda`) are enabled and models are downloaded (e.g., via `git lfs pull`). **See also:** [Full Setup Guide](../../docs/SETUP.md).
2.  **ONNX Runtime (if using `ort` feature):** Ensure the ONNX Runtime library is installed and accessible. See the [Full Setup Guide](../../docs/SETUP.md) for details.
3.  **Qdrant:** A running Qdrant vector database instance must be accessible.

## Configuration

The server needs a configuration file named `mcp.json` located in the project's root directory (the same directory where you run the server script).

**Create `mcp.json`:**

```json
{
  "qdrant_url": "http://localhost:6334",
  "repositories_base_path": "repositories",
  "model_source": {
      "model_type": "sentence-transformers",
      "model_name": "all-MiniLM-L6-v2"
  }
}
```

*   **`qdrant_url`**: **(Required)** Change this to the URL of your running Qdrant instance.
*   **`repositories_base_path`**: **(Required)** Sets the directory (relative to the project root) where cloned Git repositories will be stored. Make sure this directory exists or is writable.
*   **`model_source`**: Hints to the server which embedding model you intend to use (ensure the corresponding model files were downloaded during setup).

*(Note: The server primarily locates model files using environment variables or paths defined in the main CLI config (`~/.config/vectordb-cli/config.toml`). The included wrapper script helps manage this - see Running the Server.)*

## Running the Server

The easiest way to run the server for testing or development is using the provided wrapper script.

**Recommended Method: Wrapper Script**

A wrapper script (e.g., `run_mcp_server_with_logging.sh` in the project root) is included to simplify running the server. It automatically handles:
*   Setting up detailed logging to a file (e.g., `mcp_stderr.log`).
*   Configuring the environment (like `LD_LIBRARY_PATH`) so the server can find the ONNX Runtime libraries if needed.
*   Launching the `vectordb-mcp` executable.

**How to Use:**

1.  Make the script executable (run this once):
    ```bash
    chmod +x ./run_mcp_server_with_logging.sh 
    ```
2.  Pipe your JSON-RPC request to the script:
    ```bash
    echo '<JSON_REQUEST>' | ./run_mcp_server_with_logging.sh | cat
    ```
    Replace `<JSON_REQUEST>` with your request (e.g., from the [Test Plan](./mcp-e2e-test-plan.md)).
3.  Check the output in your terminal and view detailed logs in `mcp_stderr.log`.

**(IDE Integration Note:** For use within IDEs like Cursor, a local `.cursor/mcp.json` file is configured to use this wrapper script automatically, so you usually don't need to run it manually in that context.)

**Manual Execution (Advanced/Alternative):**

You can also run the executable directly, but you may need to manually set environment variables first if you encounter errors (e.g., `libonnxruntime.so` not found when using `ort,cuda` features):

```bash
# Example: Setting LD_LIBRARY_PATH manually
export LD_LIBRARY_PATH=./target/release/lib:$LD_LIBRARY_PATH 

# Then run the server
echo '<JSON_REQUEST>' | ./target/release/vectordb-mcp | cat
```

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