# Known Issues and Testing Gaps for vectordb-mcp

This document outlines known issues, limitations in the current testing strategy, and areas for future improvement related to `vectordb-mcp` testing and tenant handling.

## 1. `target_ref` Functionality Not Tested via MCP Tools

-   **Issue:** The `mcp_vectordb-mcp-stdio_repository_add` tool does not currently support a `target_ref` parameter.
-   **Impact:** The server's logic for cloning a repository and checking out a specific commit/tag via `target_ref` during `repository/add` cannot be directly tested using the standard MCP tools available in the current automated environment.
-   **Workaround/Future Testing:** To test this server feature, direct JSON-RPC calls must be used (e.g., via `run_terminal_cmd` with `echo`). Ensure `snake_case` parameters (`target_ref`, `tenant_id`) are used in the JSON payload. The toolset could be enhanced to support this parameter.

## 2. Limited E2E Testing for True Multi-Tenant Isolation (Tool vs. Other Specific Tenant)

-   **Issue:** The current e2e test run (using only MCP tools) primarily validates operations within the scope of the server's configured default tenant ID. When tools are used, they act under this default tenant.
-   **Impact:** Scenarios where a tool (acting as the default tenant) attempts to access a repository explicitly created under a *different, specific* tenant ID (and should be denied) are not part of the automated tool-only e2e flow.
-   **Workaround/Future Testing:** To test these cross-tenant denial scenarios:
    1.  Add a repository with a specific `tenant_id` (e.g., "tenant_X") via `run_terminal_cmd` (using `snake_case` JSON `params`).
    2.  Attempt operations (list, sync, query, remove) on this "tenant_X" repository using the standard MCP tools (which will act as the server's default tenant, e.g., "tenant_default").
    3.  These attempts should correctly result in "Access Denied" errors if "tenant_X" is different from "tenant_default".

## 3. JSON-RPC Parameter Case Sensitivity for `target_ref` and `tenant_id`

-   **Issue:** When using direct JSON-RPC calls (e.g., via `echo` piped to `vectordb-mcp stdio`), parameter keys in the JSON `params` object must be `snake_case` (e.g., `target_ref`, `tenant_id`) to align with the Rust struct field names in `RepositoryAddParams`. The current `serde` setup does not automatically handle `camelCase` for these specific structs/fields.
-   **Impact:** If `camelCase` (e.g., `targetRef`, `tenantId`) is used in the JSON `params` for direct calls, these parameters will not be deserialized correctly by the server, leading to them being ignored or treated as `None`.
-   **Recommendation:**
    -   Ensure all direct JSON-RPC test examples or client implementations use `snake_case` for these parameters.
    -   Alternatively, consider updating server-side structs (like `RepositoryAddParams`) with `#[serde(alias = "camelCaseEquivalent")]` or `#[serde(rename_all = "camelCase")]` if `camelCase` is the preferred external JSON API style.

## 4. Test Environment Potentially Overwrites User Configuration

-   **Issue:** Observations during e2e testing (especially when using `run_terminal_cmd` to execute `vectordb-mcp stdio` for single commands) suggest that these operations can modify the main user configuration file (e.g., `~/.config/vectordb/config.toml`). This was also noted in some Rust unit test setups if they save config without specifying a path.
-   **Impact:** This can lead to unexpected states persisting between test runs or inadvertently altering the user's development configuration. It caused issues where repositories seemed to "already exist" despite attempts to clean the config for a new test run.
-   **Recommendation:**
    -   Investigate and ensure that test environments (both e2e tool-based tests and Rust unit tests) use temporary or test-specific configuration files.
    -   For Rust tests, utilize `tempfile::tempdir()` and ensure any calls to `save_config` are directed to a path within the temporary directory.
    -   For e2e tests involving direct `vectordb-mcp` execution, explore mechanisms to point the binary to a test-specific config file if possible (e.g., via command-line arguments or environment variables if the server supports overriding the default config path). 