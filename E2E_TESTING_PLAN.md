# VectorDB CLI & Server E2E Testing Plan

This document outlines the steps for end-to-end testing of the `vectordb-cli` binary and its associated gRPC server.

## Prerequisites

1.  **Compiled Binary:** The `vectordb-cli` binary compiled in release mode (e.g., `./target/release/vectordb-cli`).
2.  **ONNX Models:** Default embedding model (`all-minilm-l6-v2.onnx` or similar) and tokenizer (`tokenizer.json`) available in the `./onnx/` directory relative to where the CLI is run, OR configured correctly in the default config file (`~/.config/vectordb-cli/config.toml`).
3.  **Git:** Git command-line tool installed.
4.  **grpcurl:** `grpcurl` tool installed for gRPC testing.
5.  **Qdrant:** A running Qdrant instance accessible (usually `http://localhost:6334` by default config).
6.  **Test Repository:** A git repository to use for testing (e.g., `https://github.com/octocat/Spoon-Knife`).

## Phase 1: CLI Testing

**Goal:** Test the core repository management and query commands via the CLI binary.

**Setup:**
1.  **Create Temp Directory & Clone Repo:**
    ```bash
    # Create temp dir, clone test repo, export path
    TEMP_DIR=$(mktemp -d)
    echo "TEMP_DIR=$TEMP_DIR"
    git clone https://github.com/octocat/Spoon-Knife "$TEMP_DIR/Spoon-Knife" # Replace URL if needed
    export TEST_TEMP_DIR="$TEMP_DIR"
    echo "Cloned repo to $TEST_TEMP_DIR/Spoon-Knife"
    ```
2.  **Generate Unique Name:**
    ```bash
    export UNIQUE_NAME="e2e-test-$(date +%s)"
    echo "Unique name: $UNIQUE_NAME"
    ```
3.  **Configure Environment (Optional - if not using default config):**
    *   *Note:* During testing, using a temporary config via `XDG_CONFIG_HOME` failed. It's recommended to rely on the default config `~/.config/vectordb-cli/config.toml` and ensure ONNX paths are correctly set there. If needed, backup and temporarily modify the default config.
    ```bash
    # Example of using temporary config (use with caution - may not work as expected)
    # export XDG_CONFIG_HOME="$TEST_TEMP_DIR/config"
    # mkdir -p "$XDG_CONFIG_HOME/vectordb"
    # echo -e "[onnx]\nonnx_model_path = \"/path/to/your/model.onnx\"\nonnx_tokenizer_path = \"/path/to/your/tokenizer.json\"" > "$XDG_CONFIG_HOME/vectordb/config.toml"
    # echo "Using config dir: $XDG_CONFIG_HOME"

    # Recommended: Ensure ~/.config/vectordb-cli/config.toml has correct [onnx] paths
    ```

**Test Steps:**
1.  **Add Repository:** Add the cloned local repository.
    ```bash
    ./target/release/vectordb-cli repo add --name "$UNIQUE_NAME" -p "$TEST_TEMP_DIR/Spoon-Knife" | cat
    # Expected: Success message, repo added, collection created.
    ```
2.  **Sync Repository:** Index the content of the added repository.
    ```bash
    ./target/release/vectordb-cli repo sync "$UNIQUE_NAME" | cat
    # Expected: Success message, files processed.
    ```
3.  **List Repositories:** Verify the new repository is listed and active.
    ```bash
    ./target/release/vectordb-cli repo list | cat
    # Expected: Output includes the $UNIQUE_NAME repo marked with '*' (active).
    ```
4.  **Query Repository (Initial):** Query for known content in the repo.
    ```bash
    # Replace "Spoon-Knife" with content relevant to your test repo
    ./target/release/vectordb-cli repo query "Spoon-Knife" | cat
    # Expected: Search results including chunks from the repo's files.
    ```
5.  **Simulate Change:** Add a new file and commit it.
    ```bash
    cd "$TEST_TEMP_DIR/Spoon-Knife"
    echo "This is different content." > another_file.md # Use a recognized extension like .md
    git add another_file.md
    git -c user.name='Test User' -c user.email='test@example.com' commit -m 'Add another_file.md'
    cd - # Return to original directory
    echo "Added and committed new file."
    ```
6.  **Sync Repository (Again):** Index the newly committed changes.
    ```bash
    ./target/release/vectordb-cli repo sync "$UNIQUE_NAME" | cat
    # Expected: Success message.
    ```
7.  **Query Repository (Updated):** Query for content in the new file.
    ```bash
    ./target/release/vectordb-cli repo query "different content" | cat
    # Expected: Search results including chunks from the new file (another_file.md).
    # Note: Previous testing showed issues indexing simple .txt files; use recognized extensions.
    ```
8.  **Remove Repository:** Remove the test repository config and its data.
    ```bash
    ./target/release/vectordb-cli repo remove "$UNIQUE_NAME" -y | cat
    # Expected: Success message, repo removed, active repo possibly reset.
    ```

**Cleanup:**
1.  **Remove Temp Directory:**
    ```bash
    rm -rf "$TEST_TEMP_DIR"
    unset TEST_TEMP_DIR
    unset UNIQUE_NAME
    # unset XDG_CONFIG_HOME # If it was set earlier
    echo "Cleaned up CLI test temporary directory."
    ```

## Phase 2: gRPC Testing

**Goal:** Test the core gRPC service methods.

**Setup:**
1.  **Start the Server:** Run the server on a known address/port (e.g., `0.0.0.0:9021`).
    ```bash
    # Example command (adjust as needed)
    ./target/release/vectordb-cli server start --port 9021
    ```
2.  **Create Temp Directory & Clone Repo (Separate from CLI phase):**
    ```bash
    TEMP_DIR_GRPC=$(mktemp -d)
    echo "TEMP_DIR_GRPC=$TEMP_DIR_GRPC"
    git clone https://github.com/octocat/Spoon-Knife "$TEMP_DIR_GRPC/Spoon-Knife-gRPC" # Replace URL if needed
    export TEST_TEMP_DIR_GRPC="$TEMP_DIR_GRPC"
    echo "Cloned repo to $TEST_TEMP_DIR_GRPC/Spoon-Knife-gRPC"
    ```
3.  **Generate Unique Name:**
    ```bash
    export UNIQUE_NAME_GRPC="e2e-grpc-test-$(date +%s)"
    echo "Unique gRPC name: $UNIQUE_NAME_GRPC"
    ```
4.  **Define Server Address:**
    ```bash
    export SERVER_ADDR="0.0.0.0:9021" # Adjust if server is running elsewhere
    ```

**Test Steps:**
1.  **List Services:** Verify server is reachable and services are registered.
    ```bash
    grpcurl -plaintext $SERVER_ADDR list
    # Expected: editing.EditingService, grpc.reflection.v1.ServerReflection, vectordb.VectorDBService
    ```
2.  **List Repositories (Initial):** Check current repositories via gRPC.
    ```bash
    grpcurl -plaintext -d '{}' $SERVER_ADDR vectordb.VectorDBService/ListRepositories
    # Expected: JSON response listing repositories from default config.
    ```
3.  **Add Repository:** Add the test repository via RPC using `local_path`.
    ```bash
    grpcurl -plaintext -d '{"name": "'"$UNIQUE_NAME_GRPC"'", "local_path": "'"$TEST_TEMP_DIR_GRPC/Spoon-Knife-gRPC"'"}' $SERVER_ADDR vectordb.VectorDBService/AddRepository
    # Expected: Success response.
    ```
4.  **Sync Repository:** Sync the newly added (and likely active) repository.
    ```bash
    # Assumes added repo is now active. Can add '{"name": "'"$UNIQUE_NAME_GRPC"'"}' if needed.
    grpcurl -plaintext -d '{}' $SERVER_ADDR vectordb.VectorDBService/SyncRepository
    # Expected: Success response.
    ```
5.  **Query Collection:** Query the collection associated with the repository.
    ```bash
    # Replace "Spoon-Knife" with relevant query text. Collection name derived from repo name.
    grpcurl -plaintext -d '{"collection_name": "repo_'"$UNIQUE_NAME_GRPC"'", "query_text": "Spoon-Knife", "limit": 5}' $SERVER_ADDR vectordb.VectorDBService/QueryCollection
    # Expected: JSON response with search results.
    ```
6.  **Remove Repository:** Remove the test repository via RPC.
    ```bash
    grpcurl -plaintext -d '{"name": "'"$UNIQUE_NAME_GRPC"'", "skip_confirmation": true}' $SERVER_ADDR vectordb.VectorDBService/RemoveRepository
    # Expected: Success response.
    ```

**Cleanup:**
1.  **Stop the Server:** Terminate the server process (e.g., Ctrl+C).
2.  **Remove Temp Directory:**
    ```bash
    rm -rf "$TEST_TEMP_DIR_GRPC"
    unset TEST_TEMP_DIR_GRPC
    unset UNIQUE_NAME_GRPC
    unset SERVER_ADDR
    echo "Cleaned up gRPC test temporary directory."
    ``` 