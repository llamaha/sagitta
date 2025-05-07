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

**Goal:** Add a new repository configuration to the server and trigger the initial clone/collection creation.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "repository/add",
  "params": {
    "url": "https://github.com/sinatra/sinatra.git",
    "name": "test-sinatra"
  },
  "id": 1
}
```

**Expected Response (Success):**
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
*   Verify the Qdrant collection `test-sinatra` exists (may be empty initially).

### 2. List Repositories (`repository_list`)

**Goal:** Verify the newly added repository is listed.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "repository/list",
  "params": {},
  "id": 2
}
```

**Expected Response (Success):**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "repositories": [
      {
        "name": "test-sinatra",
        "remote": "https://github.com/sinatra/sinatra.git",
        "branch": "main"
      }
      // Potentially other repositories if added previously
    ]
  },
  "id": 2
}
```

**Verification:**
*   Confirm the `test-sinatra` repository is present in the `repositories` array with correct details.

### 3. Sync Repository (`repository_sync`)

**Goal:** Fetch latest changes (if any) from the remote, update the local copy, and index the repository contents.

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

**Expected Response (Success):**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "message": "Successfully synced repository 'test-sinatra' branch/ref 'main' to commit <COMMIT_HASH>" // Example: "91cfb548c9e50a65324a9ce9e4ea5f10cd897027"
  },
  "id": 3
}
```

**Verification:**
*   Check server logs for git pull/fetch, parsing, embedding, and upsert operations.
*   Verify the Qdrant collection `test-sinatra` now contains points (check Qdrant UI or API). The number should correspond to the code chunks found (e.g., > 0).

### 4. Query Repository (`query`)

**Goal:** Perform a semantic search query against the indexed repository data.

**Request (Note: parameters are camelCase):**
```json
{
  "jsonrpc": "2.0",
  "method": "query",
  "params": {
    "repositoryName": "test-sinatra",
    "queryText": "define a route",
    "limit": 5
    // "branchName": "main" // Optional
  },
  "id": 4
}
```

**Expected Response (Success):**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "results": [
      {
        "filePath": "test/route_added_hook_test.rb", // Example result for Sinatra
        "startLine": 5,
        "endLine": 11,
        "score": 0.5, // Example score
        "content": "def self.routes ; @routes ; end\ndef self.procs ; @procs ; end\ndef self.route_added(verb, path, proc)\n    @routes << [verb, path]\n    @procs << proc\n  end"
      }
      // ... other results up to limit
    ]
  },
  "id": 4
}
```

**Verification:**
*   Check the `results` array contains relevant code snippets or file content matching the query.
*   Verify `filePath`, line numbers, and `content` seem correct.
*   Verify the number of results matches the `limit` requested (or fewer if less results available).

### 5. Add Repository with Target Ref (`repository_add`)

**Goal:** Add a repository configuration targeting a specific tag or commit.

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
    "targetRef": "f74f968a007a3a578064d079c23631f90cb63404" 
  },
  "id": 6
}
```

**Expected Response (Success):**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "name": "test-sinatra-tag",
    "url": "https://github.com/sinatra/sinatra.git",
    "localPath": "/path/to/your/repositories/test-sinatra-tag", // Actual path will vary
    "defaultBranch": "f74f968a007a3a578064d079c23631f90cb63404", // Should reflect the targetRef
    "activeBranch": "f74f968a007a3a578064d079c23631f90cb63404"  // Should reflect the targetRef
  },
  "id": 6
}
```

**Verification:**
*   Check server logs for successful clone and checkout messages.
*   Verify the repository files exist in the `localPath`.
*   Run `git -C /path/to/your/repositories/test-sinatra-tag rev-parse HEAD` and verify it matches the `targetRef`.
*   Verify the Qdrant collection `test-sinatra-tag` exists.
*   Run `repository/list` and verify `test-sinatra-tag` appears with the `targetRef` as its `branch` (or `activeBranch` if list output changes).

### 6. Sync Repository with Target Ref (`repository_sync`)

**Goal:** Ensure syncing a repository with a `targetRef` checks out the correct ref and indexes its content without fetching/pulling.

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

**Expected Response (Success):**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "message": "Successfully synced repository 'test-sinatra-tag' branch/ref 'f74f968a007a3a578064d079c23631f90cb63404' to commit f74f968a007a3a578064d079c23631f90cb63404"
  },
  "id": 7
}
```

**Verification:**
*   Check server logs: Verify `git checkout` and `git rev-parse` messages appear, but **NO** `git fetch` or `git pull` messages.
*   Verify logs show parsing, embedding, and upsert operations for the content at the specified commit.
*   Verify the Qdrant collection `test-sinatra-tag` contains points corresponding to the code at the target commit.

### 7. Query Repository with Target Ref (`query`)

**Goal:** Query the specifically indexed static version of the repository.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "query",
  "params": {
    "repositoryName": "test-sinatra-tag",
    "queryText": "get request", // Query relevant to the content at the specific tag
    "limit": 3
  },
  "id": 8
}
```

**Expected Response (Success):**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "results": [
      // Results should reflect content present *only* at the target commit/tag,
      // NOT content added in later commits on the main branch.
      {
        "filePath": "lib/sinatra/base.rb", // Example from Sinatra v3.0.0
        "startLine": 690, // Line numbers are illustrative
        "endLine": 695,
        "score": 0.6, // Example score
        "content": "      # Defining a `GET` handler also automatically defines\n      # a `HEAD` handler.\n      def get(path, opts = {}, &block)\n        conditions = @conditions.dup\n        route('GET', path, opts, &block)\n\n        @conditions = conditions\n        route('HEAD', path, opts, &block)\n      end"
      }
      // ... other relevant results ...
    ]
  },
  "id": 8
}
```

**Verification:**
*   Verify the results are relevant to the code state at the `targetRef` (`f74f968...`).
*   Verify results *do not* include content added after that commit (e.g., changes in the `main` branch of `sinatra/sinatra.git` after v3.0.0).

### 8. Remove Repository with Target Ref (`repository_remove`)

**Goal:** Remove the tagged repository configuration and data.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "repository/remove",
  "params": {
    "name": "test-sinatra-tag"
  },
  "id": 9 // Original plan used 9 for both removes, let's keep this one 9
}
```

**Expected Response (Success):**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "name": "test-sinatra-tag",
    "status": "Removed"
  },
  "id": 9
}
```

**Verification:**
*   Run `repository_list` again; `test-sinatra-tag` should no longer be listed.
*   Verify the local directory `/path/to/your/repositories/test-sinatra-tag` is deleted.
*   Verify the Qdrant collection `test-sinatra-tag` is deleted.

### 9. Remove Repository (`repository_remove`)

(Renumbering to avoid ID conflict, though test plan re-used ID 9)

**Goal:** Remove the original `test-sinatra` repository configuration and associated data.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "repository/remove",
  "params": {
    "name": "test-sinatra"
  },
  "id": 10 // New ID to avoid conflict with previous step 8 which used ID 9.
}
```

**Expected Response (Success):**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "name": "test-sinatra",
    "status": "Removed"
  },
  "id": 10
}
```

**Verification:**
*   Run `repository_list` again; `test-sinatra` should no longer be listed.
*   Check the server logs for deletion messages.
*   Verify the local repository directory (`/path/to/your/repositories/test-sinatra`) has been deleted.
*   Verify the Qdrant collection `test-sinatra` has been deleted or cleared.

