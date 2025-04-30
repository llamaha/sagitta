# Plan: vectordb-mcp Server Crate

This document outlines the plan for creating the `vectordb-mcp` crate, which will act as an MCP (Multi-purpose Cooperative Protocol) server leveraging the `vectordb-core` library.

**Goal:** Provide access to vector database operations (add repository, list repositories, sync repository, query code) via an MCP interface, potentially usable by tools like Cursor.

## Phase 1: Project Setup & Basic Structure - Complete

*Status: Directories created, `Cargo.toml` files initialized, basic `main.rs` created. Build errors resolved.* 

1.  **Create Crate Directory:** - **DONE**
    *   Create `./crates/vectordb-mcp`.
    *   Create `./crates/vectordb-mcp/src`.
2.  **Initialize `Cargo.toml`:** - **DONE**
    *   Create `./crates/vectordb-mcp/Cargo.toml`.
    *   Define it as a binary crate: `[[bin]] name = "vectordb-mcp" path = "src/main.rs"`.
    *   Add basic metadata: `name = "vectordb-mcp"`, `version`, `edition`.
    *   Add dependencies:
        *   `vectordb-core = { path = "../vectordb-core", features = ["onnx"] }`
        *   `anyhow = "1.0"` (for error handling)
        *   `tokio = { version = "1", features = ["full"] }` (async runtime)
        *   `serde = { version = "1.0", features = ["derive"] }` (serialization)
        *   `serde_json = "1.0"` (JSON serialization for messages)
        *   `tracing = "0.1"` (for logging)
        *   `tracing-subscriber = { version = "0.3", features = ["env-filter"] }` (logging setup)
        *   *Placeholder for MCP library (e.g., `mcp-server` or similar, if one exists)*
3.  **Create `main.rs`:** - **DONE**
    *   Create `./crates/vectordb-mcp/src/main.rs`.
    *   Add basic `main` function using `#[tokio::main]`.
    *   Initialize `tracing-subscriber`.
    *   Print a startup message.
    *   Load `vectordb-core` configuration (`AppConfig`).
4.  **Update Workspace `Cargo.toml`:** - **DONE**
    *   Add `crates/vectordb-mcp` to the `workspace.members` list in the root `Cargo.toml`.
5.  **Initial Build & Test:** - **DONE**
    *   Run `cargo check -p vectordb-mcp` to ensure the setup is correct. *(Passed after fixes)*
    *   Run `cargo run -p vectordb-mcp` to verify the basic binary executes. *(Verified)*

## Phase 2: MCP Server Foundation - Complete

*Status: Basic stdin/stdout JSON-RPC protocol defined (`mcp.rs`). Server loop implemented with `ping` handler (`server.rs`). Modules declared in `lib.rs` and used by `main.rs`. Build is successful.* 

1.  **Select/Implement MCP Framework:** - **DONE (stdin/stdout JSON)**
    *   Research suitable Rust libraries for building an MCP server. If none are readily available or suitable, define a basic JSON-RPC-like protocol over a standard transport (e.g., TCP sockets or stdin/stdout).
    *   Integrate the chosen library or implement the basic protocol handling in `main.rs` or separate modules (`server.rs`, `mcp.rs`). - *(Implemented in `mcp.rs`, `server.rs`. `lib.rs` added for module organization.)*
2.  **Server Lifecycle:** - **DONE (stdin reading)**
    *   Implement server startup logic (e.g., listening on a port or waiting for stdin). - *(Implemented basic stdin loop in `server.rs`)*
    *   Implement graceful shutdown handling. - *(TODO: Add signal handling for Ctrl+C)*
3.  **Message Handling:** - **DONE (Basic loop & ping)**
    *   Define request and response structures (using `serde`). - *(Done in `mcp.rs`)*
    *   Implement the core message loop to receive requests, parse them, dispatch to handlers, and send responses. - *(Basic loop in `server.rs`)*
4.  **Basic Health Check:** - **DONE (ping)**
    *   Implement a simple MCP command (e.g., `ping` or `health`) to verify the server is responsive. - *(ping handler implemented in `server.rs`)*

## Phase 3: Implement Core Repository Management Handlers - Complete

*Status: All handlers (`repository_add`, `repository_list`, `repository_sync`, `repository_remove`, `query`) implemented in `server.rs`. Errors mapped.* 

1.  **Define `AppConfig` Structure:** - DONE
    *   Define `AppConfig` in `vectordb-core` (e.g., `crates/vectordb-core/src/config.rs`) to hold global settings and a list of `RepositoryConfig`.
    *   Define `RepositoryConfig` to store details for each repository (name, URL, local path, active branch, sync status, etc.).
    *   Implement functions to load/save `AppConfig` (e.g., from/to a TOML file).
2.  **Implement `repository_add` Handler:** - DONE
    *   Define MCP request/response for adding a repository (`RepositoryAddParams`, `RepositoryAddResult`).
    *   Create handler function in `server.rs` taking request parameters (e.g., repository path/URL).
    *   Call `vectordb_core::repo_add::handle_repo_add` function.
    *   Map the result/error to the MCP response format.
3.  **Implement `repository_list` Handler:** - DONE
    *   Define MCP request/response (`RepositoryListParams` - likely empty, `RepositoryListResult` - list of repository info).
    *   Create handler function in `server.rs`.
    *   Read the `repositories` field from `AppConfig`.
    *   Format the list of `RepositoryConfig` into the MCP response format (include key info: name, url, local path, active branch).
4.  **Implement `repository_sync` Handler:** - DONE
    *   Define MCP request/response (`RepositorySyncParams` - repo name, maybe branch, sync options; `RepositorySyncResult` - status/success).
    *   Create handler function in `server.rs`.
    *   Find the specified `RepositoryConfig` in `AppConfig`.
    *   Call a core function (e.g., `vectordb_core::repo_sync::sync_repository`) to perform the git pull/fetch and potentially re-index changed files.
    *   Update the sync status in `AppConfig` and save it.
    *   Return success/failure/status in the MCP response.
5.  **`repository_remove` Handler:** - DONE
    *   Define the MCP request/response for removing a repository.
    *   Create a handler function that takes the repository name/ID.
    *   Load `AppConfig`.
    *   Find the corresponding `RepositoryConfig`.
    *   Call `vectordb_core::repo_helpers::delete_repository_data` to clean up Qdrant/cache data.
    *   Remove the `RepositoryConfig` entry from the main `AppConfig` instance.
    *   Call `vectordb_core::save_config` to persist the change.
    *   Map the result/error to the MCP response format.
6.  **`query` Handler:** - **DONE**
    *   Define the MCP request/response for querying. - *(DONE in `mcp.rs`)*
    *   Create a handler function that takes the query text and potentially repository context. - *(DONE in `server.rs` as `handle_query`)*
    *   Load `AppConfig`.
    *   Initialize `EmbeddingHandler` and `QdrantClientTrait`.
    *   Call `vectordb_core::search_impl::search_collection`. - *(DONE in `handle_query`)*
    *   Format the search results into the MCP response. - *(DONE in `handle_query`)*
7.  **Error Handling:** - DONE
    *   Ensure errors from `vectordb-core` (which uses `anyhow::Result` and `VectorDBError`) are properly caught and translated into meaningful MCP error responses.

## Phase 4: Refinement & Testing - In Progress

*Status: Query handler added. Integration tests need fixing. MCP connection failing on initialize.* 

1.  **Configuration:** - DONE (Uses AppConfig)
    *   Decide how the MCP server itself will be configured (e.g., port number, log level). Potentially integrate with the existing `AppConfig` or use command-line arguments/environment variables.
2.  **Logging & Tracing:** - DONE (Basic tracing added, output fixed to stderr)
    *   Ensure informative logs are emitted for requests, responses, errors, and key operations using the `tracing` crate.
    *   Logs correctly directed to stderr.
3.  **MCP `initialize` Handler:** - **NEW**
    *   Define `InitializeParams` and `InitializeResult` structs in `mcp/types.rs` based on MCP spec/observed request.
    *   The `InitializeResult` must declare the server's capabilities, specifically listing the available methods (`ping`, `repository_add`, `repository_list`, `repository_sync`, `repository_remove`, `query`) as MCP "tools" with appropriate names/descriptions.
    *   Implement the `initialize` method handler within `Server::handle_request` (`server.rs`).
4.  **Server Persistence (for stdio MCP):** - DONE 
    *   The main server loop (`server.rs`) correctly handles multiple requests and EOF.
5.  **Integration Testing:** - **IN PROGRESS**
    *   Develop integration tests (potentially in a separate test suite or using `#[tokio::test]`) that start the server and send MCP commands to verify end-to-end functionality.
    *   Include testing the `initialize` sequence.
    *   *Current state: `tests/mcp_integration_test.rs` created with mocks. `cargo test -p vectordb-mcp` fails with multiple errors related to trait implementation signatures, mock setup (invalid `impl EmbeddingHandler`), filter access in mock, Server instantiation, and private method access.* 
    *   *Next step: Fix the test setup issues.* 
6.  **Documentation:** - TODO
    *   Document the MCP protocol (commands, request/response formats) within the crate's README or code comments, including the `initialize` step.
    *   Document how to run and configure the server.
7.  **Code Quality:** - TODO (Skipped for now)
    *   Run `cargo fmt` and `cargo clippy`.
    *   Refactor for clarity and maintainability.

## Future Considerations (Post-MVP)

*   **Authentication/Authorization:** Secure access to the MCP server if needed.
*   **Concurrency Handling:** Optimize handling of multiple simultaneous requests.
*   **Advanced `vectordb-core` Features:** Expose other features like specific indexing control, configuration updates, etc., via MCP commands if required.
*   **Streaming Responses:** For potentially long-running operations like initial indexing.
*   **Notifications:** Server-initiated messages (e.g., indexing progress). 
