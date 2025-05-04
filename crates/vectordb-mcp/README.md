# VectorDB MCP Server (`vectordb-mcp`)

This crate implements a server adhering to the Multi-purpose Code Protocol (MCP), designed to provide an interface for interacting with the core functionalities of `vectordb-core`, such as managing Git repositories, indexing their content, and performing semantic searches.

The server communicates via JSON-RPC 2.0 messages over standard input (stdin) and standard output (stdout).

## Prerequisites

1.  **Build `vectordb` Workspace:** Build the project following the instructions in the [root README.md](../../README.md), ensuring necessary features (`ort`, `cuda`) are enabled and models are downloaded (e.g., via `git lfs pull`). **See also:** [Full Setup Guide](../../docs/SETUP.md).
2.  **ONNX Runtime (if using `ort` feature):** Ensure the ONNX Runtime library is installed and accessible. See the [Full Setup Guide](../../docs/SETUP.md) for details.
3.  **Qdrant:** A running Qdrant vector database instance must be accessible.

## Configuration

The server loads its configuration (Qdrant URL, repository paths, model settings) from the standard `vectordb-cli` configuration file, typically located at `~/.config/vectordb/vectordb-cli/config.toml`. Ensure this file is correctly configured before running the MCP server.

## Running the Server

You can run the `vectordb-mcp` executable directly from your terminal after building the project or installing a release build.

**Direct Execution:**

If you built the project locally (e.g., using `cargo build --release`):

```bash
# Ensure necessary libraries like ONNX Runtime are findable
# Example: Setting LD_LIBRARY_PATH manually if needed
export LD_LIBRARY_PATH=./target/release/lib:$LD_LIBRARY_PATH

# Pipe your JSON-RPC request to the executable
echo '<JSON_REQUEST>' | ./target/release/vectordb-mcp | cat
```

Replace `<JSON_REQUEST>` with your request (e.g., from the [Test Plan](./mcp-e2e-test-plan.md)). Check the output in your terminal. Non-JSON-RPC output (logs, errors) will go to stderr.

If you installed a release build where `vectordb-mcp` is in your `PATH` and libraries are correctly installed:

```bash
echo '<JSON_REQUEST>' | vectordb-mcp | cat
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

Currently serves as the JSON-RPC backend for `vectordb` commands.

## Cursor MCP Integration

To use `vectordb-mcp` as a backend for Cursor's AI features (like repository-aware chat), you first need to install the release binaries and dependencies. Follow the installation instructions (link to be added later, perhaps in the main README or a dedicated INSTALL.md).

Once installed, configure a custom MCP server in Cursor's global settings (File -> Settings -> Search for "MCP").

Add a new server configuration with the following settings:

```json
{
    "serverName": "vectordb",
    "workingDirectory": "/path/to/your/installation",
    "command": [
        "/path/to/your/installation/vectordb-mcp"
    ],
    "env": {
         "LD_LIBRARY_PATH": "/path/to/onnx/libs:/path/to/cuda/libs"
    }
}
```

*   **`serverName`:** `vectordb` (or any name you prefer)
*   **`workingDirectory`:** Set this to the *absolute path* where `vectordb-mcp` and its associated libraries (like the ONNX runtime libs, if packaged together) are installed. For development builds within the repo, this would be the absolute path to the repo root (e.g., `/home/user/repos/vectordb-cli`).
*   **`command`:** An array containing the *absolute path* to the `vectordb-mcp` executable (e.g., `["/path/to/your/installation/vectordb-mcp"]` or `["/home/user/repos/vectordb-cli/target/release/vectordb-mcp"]` for development).
*   **`env` (Optional):** A dictionary for environment variables. Use this to set `LD_LIBRARY_PATH` if the necessary libraries (like ONNX runtime, CUDA) aren't automatically found by the system linker. Replace the example paths with the actual absolute paths to the directories containing the required `.so` files, separated by colons (`:`). If your installation script correctly configures system library paths (e.g., using `ldconfig` or RPATH), you might not need to set this manually.

**Explanation:**

*   The installation process should place the `vectordb-mcp` binary in your system's `PATH` (e.g., `/usr/local/bin`).
*   The necessary ONNX runtime libraries should also be installed in a location where the system's dynamic linker can find them (e.g., `/usr/local/lib`, potentially requiring `ldconfig` to be run after installation). If the installer handles setting `LD_LIBRARY_PATH` or uses techniques like `RPATH`, direct execution should work. If not, you might need to manually adjust `LD_LIBRARY_PATH` or modify the command like `env LD_LIBRARY_PATH=/path/to/libs vectordb-mcp`.

All logs and non-JSON-RPC output from the server will be directed to `stderr`. You can monitor Cursor's logs for communication details and potential errors. 
