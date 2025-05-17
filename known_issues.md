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

## 5. `vectordb-cli` as a Cross-Tenant Admin Tool

-   **Issue:** The `vectordb-cli` tool is currently designed to operate within the context of a single tenant, as defined by its local `config.toml` (or `--tenant-id` argument). It does not have dedicated commands or mechanisms to directly administer all tenants or manage tenant-specific resources (like repositories) across an entire VectorDB-MCP server in a comprehensive administrative capacity.
-   **Impact:** An administrator running `vectordb-cli` on the server machine cannot easily list all tenants, or list/add/remove repositories for a *specific, different* tenant directly through the CLI. Administrative actions on the MCP server (like tenant creation, cross-tenant API key management) are primarily intended to be done via the server's HTTP API using an admin-level API key (e.g., the bootstrap admin key).
-   **Future Enhancement Considerations:**
    -   Introduce new `vectordb-cli admin <subcommand>` set of commands.
    -   These commands would authenticate to a specified MCP server's HTTP API using an admin API key.
    -   Provide functionality such as:
        -   `list-tenants`
        -   `create-tenant --name <name>`
        -   `list-api-keys --tenant-id <tenant_id_or_all>`
        -   `create-api-key --tenant-id <tenant_id> ...`
        -   `list-repos --tenant-id <tenant_id_or_all>` (requires corresponding MCP server API)
        -   `add-repo --tenant-id <tenant_id> --name <name> --url <url>` (requires corresponding MCP server API)
    -   This would make `vectordb-cli` a more complete client for both user-scoped operations (via local config) and administrative operations against an MCP server instance. 