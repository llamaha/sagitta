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

## 6. Query Quality Benchmark Script Issues

-   **Issue:** The benchmark script (`query_benchmark/run_benchmark.sh`) fails to add repositories `tsnode-typescript` and `tsnode-javascript`.
-   **Details:** Both repositories point to `https://github.com/microsoft/TypeScript-Node-Starter`. The `repo add` command fails with the error `fatal: Remote branch main not found in upstream origin`.
-   **Cause:** The `TypeScript-Node-Starter` repository likely does not have a branch named `main` (its default might be `master` or another name). The script is configured to use `main` for these repositories.
-   **Recommendation:**
    -   Verify the actual default branch for `https://github.com/microsoft/TypeScript-Node-Starter`.
    -   Update `query_benchmark/benchmark_config.yaml` to specify the correct `default_branch` for `tsnode-typescript` and `tsnode-javascript`.
    -   Consider enhancing `vectordb-cli repo add` or the server's `repository/add` logic to automatically fall back to `master` if `main` is not found (or vice-versa), or to query the default branch from the remote if no branch is specified. 

### 6.1. Query Result Scores Appear Low

-   **Observation:** Query result scores in `benchmark_results.md` frequently appear to max out around 0.5, even for seemingly relevant results. For instance, the top results for `ripgrep-rust` query `"how does ripgrep handle large regex patterns?"` both had a score of `0.5`.
-   **Impact:** This makes it difficult to use the raw score as a direct measure of confidence or top-tier relevancy if the effective maximum is not 1.0. Users rating results might be confused if a "good" match still receives a relatively low numerical score.
-   **Questions/Areas for Investigation:**
    -   Is this score scaling expected behavior for the current embedding models and scoring algorithms?
    -   What does a score of 0.5, 0.7, or 0.9 truly represent in terms of match quality?
    -   Could there be a normalization issue, or is the scale inherently compressed?
    -   This needs further investigation with the `vectordb-core` team to understand the scoring mechanism and expected score distributions.

### 6.2. Initial Query Relevancy Assessment (Example)

-   **Repository & Query:** `ripgrep-rust`, `"how does ripgrep handle large regex patterns?" --lang rust`
-   **Result 1 (Score: 0.5):** `crates/core/main.rs` (main `run` function).
    -   **Assessment:** Moderately relevant (5/10). It shows the main program flow but lacks specific details on large regex handling techniques (e.g., specific algorithms, engine details, memory management for large patterns).
-   **Result 2 (Score: 0.5):** `crates/core/searcher/glue.rs` (a test module).
    -   **Assessment:** Low relevancy (3/10). Contains test cases for various search functionalities but doesn\'t address the query about handling large regex patterns.
-   **General Observation:** For this specific query, while the files are from the core of `ripgrep`, the returned snippets are not highly specific to the "large regex patterns" aspect. This might indicate a need for more targeted queries, or it could reflect limitations in how the current indexing/search surfaces deep technical details.
-   **Recommendation:** 
    -   Continue analysis across more queries and repositories in `benchmark_results.md` to identify patterns.
    -   Consider if query refinement in `benchmark_config.yaml` (e.g., adding more specific keywords or using different `type` filters) could improve results for certain types of questions.
    -   If relevancy issues are widespread, it may point to areas for improvement in the core `vectordb-core` search algorithms or embedding models. 