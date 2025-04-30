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
echo '<JSON_REQUEST>' | LD_LIBRARY_PATH=./target/release/lib:$LD_LIBRARY_PATH target/release/vectordb-mcp | cat
```

Replace `<JSON_REQUEST>` with the specific request JSON for each step below. The server's response (and logs) will be printed to your terminal.

## Test Cases

We will use a sample repository for testing. Adjust URLs and paths as needed.

**Repository Details:**
*   Name: `test-basic`
*   URL: `https://github.com/git-fixtures/basic.git` (Example public repo)
*   Branch: `master`

### 1. Add Repository (`repository_add`)

**Goal:** Add a new repository configuration to the server and trigger the initial clone/collection creation.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "repository/add",
  "params": {
    "url": "https://github.com/git-fixtures/basic.git",
    "name": "test-basic"
  },
  "id": 1
}
```

**Expected Response (Success):**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "name": "test-basic",
    "url": "https://github.com/git-fixtures/basic.git",
    "local_path": "/path/to/your/repositories/test-basic", // Actual path will vary
    "default_branch": "master",
    "active_branch": "master" // Or null initially, depending on implementation detail
  },
  "id": 1
}
```

**Verification:**
*   Check server logs for successful clone and collection creation messages.
*   Verify the repository files exist in the `local_path`.
*   Verify the Qdrant collection `test-basic` exists (may be empty initially).

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
        "name": "test-basic",
        "url": "https://github.com/git-fixtures/basic.git",
        "local_path": "/path/to/your/repositories/test-basic",
        "active_branch": "master"
      }
      // Potentially other repositories if added previously
    ]
  },
  "id": 2
}
```

**Verification:**
*   Confirm the `test-basic` repository is present in the `repositories` array with correct details.

### 3. Sync Repository (`repository_sync`)

**Goal:** Fetch latest changes (if any) from the remote, update the local copy, and index the repository contents.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "repository/sync",
  "params": {
    "name": "test-basic"
  },
  "id": 3
}
```

**Expected Response (Success):**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "name": "test-basic",
    "status": "Synced and Indexed", // Or similar success status
    "commit_hash": "..." // The commit hash that was indexed
  },
  "id": 3
}
```

**Verification:**
*   Check server logs for git pull/fetch, parsing, embedding, and upsert operations.
*   Verify the Qdrant collection `test-basic` now contains points (check Qdrant UI or API). The number should correspond to the code chunks found.

### 4. Query Repository (`query`)

**Goal:** Perform a semantic search query against the indexed repository data.

**Request (Note: parameters are snake_case):**
```json
{
  "jsonrpc": "2.0",
  "method": "query",
  "params": {
    "repository_name": "test-basic",
    "query_text": "read file content",
    "limit": 5
    // "branch_name": "master" // Optional, snake_case if used
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
        "file_path": "CHANGELOG", // Example result
        "start_line": 1,
        "end_line": 5,
        "score": 0.85, // Example score
        "content": "== 0.1.1 / 2013-02-17\n\n* Fix issue #1.\n\n== 0.1.0 / 2013-02-14\n\n* Added foo().\n* Added bar()."
      }
      // ... other results up to limit
    ]
  },
  "id": 4
}
```

**Verification:**
*   Check the `results` array contains relevant code snippets or file content matching the query.
*   Verify `file_path`, line numbers, and `content` seem correct.
*   Verify the number of results matches the `limit` requested (or fewer if less results available).

### 5. Add Repository with Target Ref (`repository_add`)

**Goal:** Add a repository configuration targeting a specific tag or commit.

**Repository Details:**
*   Name: `test-basic-tag`
*   URL: `https://github.com/git-fixtures/basic.git`
*   Target Ref: `b029517f6300c2da0f4b651b8642506cd6aaf45d` (Commit hash for v1.0.0 tag)

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "repository/add",
  "params": {
    "url": "https://github.com/git-fixtures/basic.git",
    "name": "test-basic-tag",
    "target_ref": "b029517f6300c2da0f4b651b8642506cd6aaf45d" 
  },
  "id": 6
}
```

**Expected Response (Success):**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "name": "test-basic-tag",
    "url": "https://github.com/git-fixtures/basic.git",
    "local_path": "/path/to/your/repositories/test-basic-tag", // Actual path will vary
    "default_branch": "b029517f6300c2da0f4b651b8642506cd6aaf45d", // Should reflect the target ref
    "active_branch": "b029517f6300c2da0f4b651b8642506cd6aaf45d"  // Should reflect the target ref
  },
  "id": 6
}
```

**Verification:**
*   Check server logs for successful clone and checkout messages.
*   Verify the repository files exist in the `local_path`.
*   Run `git -C /path/to/your/repositories/test-basic-tag rev-parse HEAD` and verify it matches the `target_ref`.
*   Verify the Qdrant collection `test-basic-tag` exists.
*   Run `repository/list` and verify `test-basic-tag` appears with the `target_ref` as its `active_branch`.

### 6. Sync Repository with Target Ref (`repository_sync`)

**Goal:** Ensure syncing a repository with a `target_ref` checks out the correct ref and indexes its content without fetching/pulling.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "repository/sync",
  "params": {
    "name": "test-basic-tag"
  },
  "id": 7
}
```

**Expected Response (Success):**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "name": "test-basic-tag",
    "status": "Indexed static ref 'b029517f6300c2da0f4b651b8642506cd6aaf45d'", // Status reflects static ref
    "commit_hash": "b029517f6300c2da0f4b651b8642506cd6aaf45d" // Commit hash matches the target ref
  },
  "id": 7
}
```

**Verification:**
*   Check server logs: Verify `git checkout` and `git rev-parse` messages appear, but **NO** `git fetch` or `git pull` messages.
*   Verify logs show parsing, embedding, and upsert operations for the content at the specified commit.
*   Verify the Qdrant collection `test-basic-tag` contains points corresponding to the code at the target commit.

### 7. Query Repository with Target Ref (`query`)

**Goal:** Query the specifically indexed static version of the repository.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "query",
  "params": {
    "repository_name": "test-basic-tag",
    "query_text": "file content", // Query relevant to the content at the specific tag
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
        "file_path": ".gitignore", // Example from v1.0.0
        "start_line": 1,
        "end_line": 1,
        "score": 0.7, // Example score
        "content": "*.rbc"
      }
      // ... other relevant results ...
    ]
  },
  "id": 8
}
```

**Verification:**
*   Verify the results are relevant to the code state at the `target_ref` (`b029517...`).
*   Verify results *do not* include content added after that commit (e.g., changes in the `master` branch of `git-fixtures/basic.git`).

### 8. Remove Repository with Target Ref (`repository_remove`)

**Goal:** Remove the tagged repository configuration and data.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "repository/remove",
  "params": {
    "name": "test-basic-tag"
  },
  "id": 9
}
```

**Expected Response (Success):**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "name": "test-basic-tag",
    "status": "Removed"
  },
  "id": 9
}
```

**Verification:**
*   Run `repository_list` again; `test-basic-tag` should no longer be listed.
*   Verify the local directory `/path/to/your/repositories/test-basic-tag` is deleted.
*   Verify the Qdrant collection `test-basic-tag` is deleted.

### 9. Remove Repository (`repository_remove`)

(Renumbering the original remove test case)

**Goal:** Remove the original `test-basic` repository configuration and associated data.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "repository/remove",
  "params": {
    "name": "test-basic"
  },
  "id": 9
}
```

**Expected Response (Success):**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "name": "test-basic",
    "status": "Removed"
  },
  "id": 9
}
```

**Verification:**
*   Run `repository_list` again; `test-basic` should no longer be listed.
*   Check the server logs for deletion messages.
*   Verify the local repository directory (`/path/to/your/repositories/test-basic`) has been deleted.
*   Verify the Qdrant collection `test-basic` has been deleted or cleared.

