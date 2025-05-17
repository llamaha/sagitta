# MCP Server End-to-End Test Plan

This document outlines the manual end-to-end test plan for the `vectordb-mcp` server, interacting via its JSON-RPC interface over standard input/output.

## Prerequisites

1.  Build the `vectordb-mcp` binary: `cargo build --all --release --features ort,cuda`
    *   **Note:** If using `--features ort,cuda`, you might need to set the `LD_LIBRARY_PATH` environment variable if the server fails to start due to missing shared libraries (e.g., `libonnxruntime.so`). Example: `export LD_LIBRARY_PATH=./target/release/lib:$LD_LIBRARY_PATH` before running the server.
2.  Have a Qdrant instance running and accessible at the URL specified in the config (default: `http://localhost:6334`).
3.  Ensure the `repositories/` directory (or the configured base path) exists and is writable.

## Test Execution

The server reads a single JSON-RPC request from standard input, processes it, sends the response to standard output, and then exits.

To run each test case, pipe the JSON request directly into the server process using `echo` and capture the output. Ensure `LD_LIBRARY_PATH` is set if needed (see prerequisites).

Example:
```bash
echo '{"jsonrpc": "2.0", "method": "repository/sync", "params": {"name": "test-sinatra"}, "id": 1}' | LD_LIBRARY_PATH=/home/adam/repos/vectordb-cli/target/release/lib RUST_LOG=debug /home/adam/repos/vectordb-cli/target/release/vectordb-mcp
```

Replace `<JSON_REQUEST>` with the specific request JSON for each step below. The server's response (and logs) will be printed to your terminal.

## Test Cases

We will use a sample repository for testing. Adjust URLs and paths as needed.

**Repository Details:**
*   Name: `test-sinatra`
*   URL: `https://github.com/sinatra/sinatra.git`
*   Branch: `main`

### 1. Add Repository (`repository_add`)

**Goal:** Add a new repository configuration to the server and trigger the initial clone/collection creation. This repository will be associated with a specific tenant.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "repository/add",
  "params": {
    "url": "https://github.com/sinatra/sinatra.git",
    "name": "test-sinatra",
    "tenantId": "e2e_tenant_1"
  },
  "id": 1
}
```

**Expected Response (Success for Add):**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "name": "test-sinatra",
    "url": "https://github.com/sinatra/sinatra.git",
    "localPath": "/path/to/your/repositories/test-sinatra", // Actual path will vary
    "defaultBranch": "main",
    "activeBranch": "main"
  },
  "id": 1
}
```

**Verification:**
*   Check server logs for successful clone and collection creation messages.
*   Verify the repository files exist in the `localPath`.
*   Verify the Qdrant collection (e.g., `<tenant_prefix>_e2e_tenant_1_test-sinatra`) exists. The exact naming convention for tenant-specific collections should be confirmed.

### 2. List Repositories (`repository_list`)

**Goal:** Verify the newly added tenant-specific repository is *not* listed when a global user (simulated by direct MCP call) lists repositories.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "repository/list",
  "params": {},
  "id": 2
}
```

**Expected Response (Success, but empty or not containing `test-sinatra`):**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "repositories": [
      // "test-sinatra" should NOT be listed here
      // Potentially other *global* repositories if added previously
    ]
  },
  "id": 2
}
```

**Verification:**
*   Confirm the `test-sinatra` repository is *not* present in the `repositories` array.

### 3. Sync Repository (`repository_sync`)

**Goal:** Verify that attempting to sync the tenant-specific repository by a global user results in an access denied error.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "repository/sync",
  "params": {
    "name": "test-sinatra"
  },
  "id": 3
}
```

**Expected Response (Error - Access Denied):**
```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32030, // ACCESS_DENIED
    "message": "Access denied: This repository requires a tenant ID for sync." // Or similar message
    // "data": { ... } // Optional error data
  },
  "id": 3
}
```

**Verification:**
*   Check server logs for access denied messages for `test-sinatra`.
*   Verify the Qdrant collection for `test-sinatra` remains unchanged (not synced).

### 4. Query Repository (`query`)

**Goal:** Verify that attempting to query the tenant-specific repository by a global user results in an access denied error.

**Request (Note: parameters are camelCase):**
```json
{
  "jsonrpc": "2.0",
  "method": "query",
  "params": {
    "repositoryName": "test-sinatra",
    "queryText": "define a route",
    "limit": 5
  },
  "id": 4
}
```

**Expected Response (Error - Access Denied):**
```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32030, // ACCESS_DENIED
    "message": "Access denied: Tenant ID mismatch for query operation." // Or similar query-specific denial message
    // "data": { ... } // Optional error data
  },
  "id": 4
}
```

**Verification:**
*   Check server logs for access denied messages.

### 5. Add Repository with Target Ref (`repository_add`)

**Goal:** Add another tenant-specific repository configuration targeting a specific tag or commit.

**Repository Details:**
*   Name: `test-sinatra-tag`
*   URL: `https://github.com/sinatra/sinatra.git`
*   Target Ref: `f74f968a007a3a578064d079c23631f90cb63404` (Commit hash for v3.0.0 tag)

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "repository/add",
  "params": {
    "url": "https://github.com/sinatra/sinatra.git",
    "name": "test-sinatra-tag",
    "targetRef": "f74f968a007a3a578064d079c23631f90cb63404",
    "tenantId": "e2e_tenant_2" 
  },
  "id": 6
}
```

**Expected Response (Success for Add):**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "name": "test-sinatra-tag",
    "url": "https://github.com/sinatra/sinatra.git",
    "localPath": "/path/to/your/repositories/test-sinatra-tag", // Actual path will vary
    "defaultBranch": "f74f968a007a3a578064d079c23631f90cb63404", 
    "activeBranch": "f74f968a007a3a578064d079c23631f90cb63404"
  },
  "id": 6
}
```

**Verification:**
*   Verify Qdrant collection (e.g., `<tenant_prefix>_e2e_tenant_2_test-sinatra-tag`) exists.

### 6. Sync Repository with Target Ref (`repository_sync`)

**Goal:** Verify that attempting to sync the second tenant-specific repository by a global user results in an access denied error.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "repository/sync",
  "params": {
    "name": "test-sinatra-tag"
  },
  "id": 7
}
```

**Expected Response (Error - Access Denied):**
```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32030, // ACCESS_DENIED
    "message": "Access denied: This repository requires a tenant ID for sync." // Or similar
  },
  "id": 7
}
```

**Verification:**
*   Check server logs for access denied messages.

### 7. Query Repository with Target Ref (`query`)

**Goal:** Verify that attempting to query the second tenant-specific repository by a global user results in an access denied error.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "query",
  "params": {
    "repositoryName": "test-sinatra-tag",
    "queryText": "get request", 
    "limit": 3
  },
  "id": 8
}
```

**Expected Response (Error - Access Denied):**
```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32030, // ACCESS_DENIED
    "message": "Access denied: Tenant ID mismatch for query operation." // Or similar
  },
  "id": 8
}
```

**Verification:**
*   Check server logs for access denied messages.

### 8. Remove Repository with Target Ref (`repository_remove`)

**Goal:** Verify that attempting to remove the second tenant-specific repository by a global user results in an access denied error.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "repository/remove",
  "params": {
    "name": "test-sinatra-tag"
  },
  "id": 9 
}
```

**Expected Response (Error - Access Denied):**
```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32030, // ACCESS_DENIED
    "message": "Access denied: This repository requires a tenant ID for removal." // Or similar
  },
  "id": 9
}
```

**Verification:**
*   Run `repository_list` again; `test-sinatra-tag` should still not be listed (as it was tenant-specific).
*   Verify the local directory and Qdrant collection for `test-sinatra-tag` still exist (as removal was denied).

### 9. Remove Repository (`repository_remove`)

**Goal:** Verify that attempting to remove the first tenant-specific repository by a global user results in an access denied error.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "repository/remove",
  "params": {
    "name": "test-sinatra"
  },
  "id": 10 
}
```

**Expected Response (Error - Access Denied):**
```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32030, // ACCESS_DENIED
    "message": "Access denied: This repository requires a tenant ID for removal." // Or similar
  },
  "id": 10
}
```

**Verification:**
*   Verify `test-sinatra` local directory and Qdrant collection still exist.

---

**Note on E2E Tenant Isolation Testing:**
The above tests reflect the behavior when interacting with the MCP server directly via its JSON-RPC stdin/stdout interface. In this mode, no authenticated user context (like `AuthenticatedUser` from HTTP middleware) is available.
- `repository/add` requires a `tenantId` in its parameters.
- Subsequent operations (`list`, `sync`, `query`, `remove`) on these tenant-specific repositories will be treated as attempted by a "global" user (no tenant ID). The implemented tenant isolation logic will deny these operations, which is the expected behavior being tested here.

To test the full lifecycle (add, sync, query, remove) for a tenant-specific repository *by a user belonging to that tenant*, tests would need to be run through an interface that populates the `AuthenticatedUser` context (e.g., an HTTP server with authentication middleware) or use a test harness that can mock this context for the MCP handlers.

