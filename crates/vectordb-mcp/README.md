# VectorDB MCP Server (`vectordb-mcp`)

This crate implements a server adhering to the Model Context Protocol (MCP), providing a JSON-RPC 2.0 interface for `vectordb-core` functionalities. It can operate in two modes: Stdio and HTTP/SSE.

Refer to the main [root README.md](../../README.md) for prerequisites and setup.

## Configuration

The server loads its configuration (Qdrant URL, repository paths, model settings) from the central configuration file, typically located at `~/.config/vectordb/config.toml`. Ensure this file is correctly configured before running the MCP server.

## Capabilities (JSON-RPC Methods)

*   **`initialize`**: Standard MCP method to initialize connection and capabilities.
*   **`ping`**: Checks server responsiveness. Returns `{"message": "pong"}`.
*   **`repository/add`**: Adds/clones a repository.
    *   *Params*: `name` (string, required), `url` (string, required if no `local_path`), `local_path` (string, required if no `url`), `target_ref` (string, optional - tag/commit/branch to checkout and track statically).
*   **`repository/list`**: Lists configured repositories.
    *   *Params*: None.
*   **`repository/sync`**: Indexes content for a repository.
    *   *Params*: `name` (string, required).
    *   *Behavior*: If `target_ref` was set during add, checks out and indexes that ref only. Otherwise, fetches the active branch, merges, and indexes changes.
*   **`repository/remove`**: Removes repository configuration and attempts data cleanup.
    *   *Params*: `name` (string, required).
*   **`query`**: Performs semantic search.
    *   *Params*: `repository_name` (string, required), `query_text` (string, required), `limit` (integer, optional), `branch_name` (string, optional - ignored if repo uses `target_ref`).
*   **`tools/list`**: Lists all available tools (methods) the server offers.
    *   *Params*: None.

## MCP Integration with Cursor

You can integrate `vectordb-mcp` with Cursor in two ways:

### 1. Stdio Mode (Managed by Cursor)

In this mode, Cursor directly runs and manages the `vectordb-mcp` process.

Add a server configuration like this to your Cursor `mcp.json` file (e.g., `~/.cursor/mcp.json` or `.cursor/mcp.json` in your project):

```json
{
  "mcpServers": {
    "vectordb-mcp-stdio": { // Changed name to distinguish from HTTP mode
      "command": "/path/to/your/vectordb-workspace/target/release/vectordb-mcp",
      "args": ["stdio"], // Explicitly specify stdio mode
      "cwd": "/path/to/your/vectordb-workspace",
      "env": {
        "LD_LIBRARY_PATH": "/usr/local/cuda-12.8/lib64:/home/adam/onnxruntime-linux-x64-gpu-1.20.0/lib/", // Adjust to your environment
        "RAYON_NUM_THREADS": "12", // Adjust to your environment
        "RUST_LOG": "info,vectordb_mcp=info" // Optional: Set log level
      }
    }
  }
}
```

**Key Configuration Fields for Stdio:**

*   `command`: Absolute path to the `vectordb-mcp` binary.
*   `args`: Should include `"stdio"` to run in stdio mode.
*   `cwd`: Absolute path to the workspace root (where `vectordb-mcp` can find its own config, etc.).
*   `env.LD_LIBRARY_PATH`: Path(s) to ONNX Runtime libraries (and CUDA if applicable). Adjust to your environment. Use `PATH` on Windows.
*   `env.RAYON_NUM_THREADS` (optional): Limit parallel threads for Rayon. Adjust to your environment.
*   `env.RUST_LOG` (optional): Configure logging. Example: `info,vectordb_mcp=debug` for more verbose logs from this server.

*(Server logs are typically sent to stderr and might be visible in Cursor's MCP logs or developer tools depending on Cursor's version).* 

### 2. HTTP/SSE Mode (User-Managed Server)

In this mode, you run the `vectordb-mcp` server as a separate, persistent process. Cursor then connects to it over the network using Server-Sent Events (SSE).

**Step 1: Manually Run the `vectordb-mcp` Server**

Open your terminal. You need to set necessary environment variables and then run the `vectordb-mcp` binary with the `http` subcommand. It's recommended to run it in the background using `nohup` and `&` for persistence.

```bash
# Adjust these paths and values for your specific environment
export LD_LIBRARY_PATH="/usr/local/cuda-12.8/lib64:/home/adam/onnxruntime-linux-x64-gpu-1.20.0/lib/:$LD_LIBRARY_PATH"
export RAYON_NUM_THREADS="12"
export RUST_LOG="info,vectordb_mcp=info" # Optional: for logging

# Navigate to your workspace if the binary needs to resolve paths relative to it (e.g., for config)
# cd /path/to/your/vectordb-workspace

# Run the server (example with nohup for backgrounding)
# Ensure the path to vectordb-mcp is correct.
# It will log to nohup.out by default.
nohup /path/to/your/vectordb-workspace/target/release/vectordb-mcp http --host 0.0.0.0 --port 8080 & 
```

*   Replace `/path/to/your/vectordb-workspace/target/release/vectordb-mcp` with the actual path to your compiled binary.
*   `--host 0.0.0.0` makes the server accessible from any network interface (important if Cursor is in a different container/VM, though for local Cursor, `127.0.0.1` is also fine).
*   `--port 8080` is the default, but you can change it. Ensure it's not blocked by a firewall.
*   Check `nohup.out` (or the specified output file) for server logs and to confirm it started correctly.

**Step 2: Configure Cursor to Use the HTTP/SSE Server**

Add a server configuration like this to your Cursor `mcp.json` file:

```json
{
  "mcpServers": {
    "vectordb-mcp-http": { // Unique name for this server configuration
      "url": "http://127.0.0.1:8080/sse" // Use the host and port your server is listening on
      // "env" is not typically used here as Cursor doesn't manage the process.
      // API keys or other secrets for the server itself should be managed
      // in the environment where you run the server process (Step 1).
    }
  }
}
```

*   `url`: Must point to the `/sse` endpoint of your running `vectordb-mcp` server.

**Important Notes for HTTP/SSE Mode:**

*   You are responsible for starting, stopping, and managing the `vectordb-mcp` server process, including ensuring it has the correct environment variables (`LD_LIBRARY_PATH`, `RAYON_NUM_THREADS`, `RUST_LOG`, etc.).
*   If you update the `vectordb-mcp` binary, you'll need to manually restart your server process.
*   Ensure the host and port are accessible to Cursor.

## General Prerequisites

1.  **Qdrant:** A running Qdrant vector database instance must be accessible by `vectordb-mcp` (configured in its `config.toml`).
