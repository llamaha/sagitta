# VectorDB CLI E2E Testing Plan

This document outlines the steps for end-to-end testing of the `vectordb-cli` binary.

## Prerequisites

1.  **Compiled Binary:** The `vectordb-cli` binary compiled in release mode (e.g., `./target/release/vectordb-cli`).
2.  **ONNX Models:** Default embedding model (`all-minilm-l6-v2.onnx` or similar) and tokenizer (`tokenizer.json`) available in the `./onnx/` directory relative to where the CLI is run, OR configured correctly in the default config file (`~/.config/vectordb-cli/config.toml`).
3.  **Git:** Git command-line tool installed.
4.  **Qdrant:** A running Qdrant instance accessible (web UI on port 6333, service for the tool on port 6334 by default config).
5.  **Test Repository:** A git repository to use for testing (e.g., `https://github.com/octocat/Spoon-Knife`).

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
    # NOTE: If repo is set active via 'repo use', the --repo flag is not needed.
    ./target/release/vectordb-cli repo query "Spoon-Knife" | cat
    # Expected: Search results including chunks from the repo's files.
    ```
5.  **Use Repository:** Switch to the test repository.
    ```bash
    ./target/release/vectordb-cli repo use "$UNIQUE_NAME" | cat
    # Expected: Success message, repo set as active.
    ```
6.  **Simulate Change:** Add a new file and commit it.
    ```bash
    cd "$TEST_TEMP_DIR/Spoon-Knife"
    echo "This is different content." > another_file.md # Use a recognized extension like .md
    git add another_file.md
    git -c user.name='Test User' -c user.email='test@example.com' commit -m 'Add another_file.md'
    cd - # Return to original directory
    echo "Added and committed new file."
    ```
7.  **Sync Repository (Again):** Index the newly committed changes.
    ```bash
    ./target/release/vectordb-cli repo sync "$UNIQUE_NAME" | cat
    # Expected: Success message.
    ```
8.  **Query Repository (Updated):** Query for content in the new file.
    ```bash
    # NOTE: If repo is set active via 'repo use', the --repo flag is not needed.
    ./target/release/vectordb-cli repo query "different content" | cat
    # Expected: Search results including chunks from the new file (another_file.md).
    # Note: Previous testing showed issues indexing simple .txt files; use recognized extensions.
    ```
9.  **Repository Stats:** Get statistics about the repository.
    ```bash
    # NOTE: If repo is set active via 'repo use', the repo name argument is not needed.
    ./target/release/vectordb-cli repo stats | cat
    # Expected: Output includes number of documents, vectors, and other relevant statistics.
    ```
10. **Clear Repository:** Clear the content of the repository.
    ```bash
    # NOTE: If repo is set active via 'repo use', the repo name argument is not needed.
    ./target/release/vectordb-cli repo clear -y | cat
    # Expected: Success message, repository content cleared.
    ```
11. **Remove Repository:** Remove the test repository config and its data.
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

## Phase 2: Simple Index Testing

**Goal:** Test the non-repository based index commands.

**Setup:**
1.  **Create Test File:**
    ```bash
    echo "Simple index content for E2E test." > "$TEST_TEMP_DIR/simple_test.txt"
    echo "Created simple test file: $TEST_TEMP_DIR/simple_test.txt"
    ```

**Test Steps:**
1.  **Index File:** Index the created test file.
    ```bash
    ./target/release/vectordb-cli simple index "$TEST_TEMP_DIR/simple_test.txt" | cat
    # Expected: Success message, file indexed.
    ```
2.  **Query Index:** Query for content in the indexed file.
    ```bash
    ./target/release/vectordb-cli simple query "E2E test" | cat
    # Expected: Search results including the chunk from simple_test.txt.
    ```
3.  **Clear Index:** Clear the simple index.
    ```bash
    # NOTE: The -y flag is not supported for this command.
    ./target/release/vectordb-cli simple clear | cat
    # Expected: Success message, index cleared.
    ```
4.  **Query Index (After Clear):** Verify the index is empty.
    ```bash
    ./target/release/vectordb-cli simple query "E2E test" | cat
    # Expected: No results found or error indicating empty collection.
    ```

**Cleanup:** (Covered by main cleanup)

## Phase 3: Edit Command Testing (Basic)

**Goal:** Test the basic functionality of the edit command.

**Setup:**
1.  **Ensure Repo Added and Synced:** Assumes Phase 1 (Repo testing) completed successfully up to the sync step.
2.  **Create Target File:**
    ```bash
    # Use the existing cloned repo from Phase 1
    echo -e "def hello():\\n    print(\"Hello World\")" > "$TEST_TEMP_DIR/Spoon-Knife/edit_test.py"
    echo "Created edit test file: $TEST_TEMP_DIR/Spoon-Knife/edit_test.py"
    # Add and commit this file so it's known to the index
    (cd "$TEST_TEMP_DIR/Spoon-Knife" && git add edit_test.py && git -c user.name='Test User' -c user.email='test@example.com' commit -m 'Add edit_test.py')
    ./target/release/vectordb-cli repo sync "$UNIQUE_NAME" | cat
    ```

**Test Steps:**
1.  **Validate Edit Function:** Use the edit validate command to check the modification.
    ```bash
    # NOTE: The 'edit' command requires a subcommand like 'validate'.
    # Similar to apply, needs explicit content and line numbers.
    REPLACEMENT_CONTENT='    print("Hello E2E Test")' # Simple content to avoid escaping issues
    ./target/release/vectordb-cli edit validate --file "$TEST_TEMP_DIR/Spoon-Knife/edit_test.py" --edit-content "$REPLACEMENT_CONTENT" --start-line 2 --end-line 2 | cat
    # Expected: Success message, indicating validation passed (or specific validation errors).
    ```
2.  **Apply Edit Function:** Use the edit apply command to modify the function.
    ```bash
    # NOTE: The 'edit' command requires a subcommand like 'apply'.
    # 'apply' needs explicit content (--edit-content) and line numbers, not a query.
    # The test below verifies applying a direct edit, not semantic generation based on a query.
    # Shell escaping for special characters in REPLACEMENT_CONTENT can be tricky.
    REPLACEMENT_CONTENT='    print("Hello E2E Test")' # Simple content to avoid escaping issues
    ./target/release/vectordb-cli edit apply --file "$TEST_TEMP_DIR/Spoon-Knife/edit_test.py" --edit-content "$REPLACEMENT_CONTENT" --start-line 2 --end-line 2 | cat
    # Expected: Success message, indicating the file was edited.
    # ACTUAL RESULT (during test run): Command ran but did not modify the file. Phase skipped.
    ```
3.  **Verify Edit:** Check the content of the file.
    ```bash
    cat "$TEST_TEMP_DIR/Spoon-Knife/edit_test.py"
    # Expected: Content should show print("Hello E2E Test").
    ```

4.  **Test Semantic Target (Validate):** Validate an edit using a semantic query.
    ```bash
    # Ensure edit_test.py exists and is committed/synced
    ./target/release/vectordb-cli edit validate --file "$TEST_TEMP_DIR/Spoon-Knife/edit_test.py" --element-query "function hello" --edit-content "    # Semantic edit target" | cat
    # Expected: Validation success or failure based on finding the element.
    ```

5.  **Test Semantic Target (Apply):** Apply an edit using a semantic query.
    ```bash
    # Ensure edit_test.py exists and is committed/synced
    ./target/release/vectordb-cli edit apply --file "$TEST_TEMP_DIR/Spoon-Knife/edit_test.py" --element-query "function hello" --edit-content "    print(\"Semantic Edit Applied\")" | cat
    # Expected: Success message, indicating the file was edited.
    ```

6.  **Verify Semantic Edit:** Check the content of the file.
    ```bash
    cat "$TEST_TEMP_DIR/Spoon-Knife/edit_test.py"
    # Expected: Content should show print("Semantic Edit Applied").
    ```

7.  **Test Feature Flags (Apply):** Run apply with unimplemented feature flags.
    ```bash
    # Ensure edit_test.py exists and is committed/synced
    ./target/release/vectordb-cli edit apply --file "$TEST_TEMP_DIR/Spoon-Knife/edit_test.py" --start-line 1 --end-line 1 --edit-content "# Flag Test" --no-format --update-references --no-preserve-docs | cat
    # Expected: Success message, potentially with "Note: ... option is set (not implemented yet)." messages.
    ```

8.  **Test ONNX Flags (Apply):** Run apply with ONNX flags (should have no effect).
    ```bash
    # Ensure edit_test.py exists and is committed/synced
    ./target/release/vectordb-cli -m ./onnx/all-minilm-l6-v2.onnx -t ./onnx edit apply --file "$TEST_TEMP_DIR/Spoon-Knife/edit_test.py" --start-line 1 --end-line 1 --edit-content "# ONNX Flag Test" | cat
    # Expected: Success message. ONNX flags are parsed but ignored by edit logic.
    ```

**Cleanup:** (Covered by main cleanup)