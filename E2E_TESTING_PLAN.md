# VectorDB CLI E2E Testing Plan

This document outlines the steps for end-to-end testing of the `vectordb-cli` binary.

## Prerequisites

1.  **Compiled Binary:** The `vectordb-cli` binary compiled in release mode (e.g., `./target/release/vectordb-cli`).
2.  **ONNX Models:** Default embedding model (`all-minilm-l6-v2.onnx` or similar) and tokenizer (`tokenizer.json`) available. The CLI will look for them via config file (`~/.config/vectordb-cli/config.toml`), environment variables (`VECTORDB_ONNX_MODEL`, `VECTORDB_ONNX_TOKENIZER_DIR`), or command-line flags (`-m`/`--onnx-model`, `-t`/`--onnx-tokenizer-dir`). Ensure they are accessible.
3.  **Git:** Git command-line tool installed.
4.  **Qdrant:** A running Qdrant instance accessible (default GRPC on port 6334). Use `curl http://localhost:6334/readyz` to check.
5.  **Test Repositories:** Internet access to clone `https://github.com/octocat/Spoon-Knife` and `https://github.com/rust-lang/book`.

## Phase 1: Repo Commands Testing

**Goal:** Test repository management (`add`, `list`, `use`, `sync`, `stats`, `clear`, `remove`) and querying (`query`).

**Setup:**
1.  Create a temporary directory.
2.  Clone test repositories (`octocat/Spoon-Knife`, `rust-lang/book`) into the temp directory.
3.  Generate unique names for the test repositories based on the current timestamp (e.g., `spoon_knife_$(date +%s)`, `rust_book_$(date +%s)`).

**Test Steps (Execute these commands manually):**
1.  **Basic CLI:** Check `--help` and `--version` flags.
    ```bash
    ./target/release/vectordb-cli --help
    ./target/release/vectordb-cli --version
    ```
2.  **Add Repositories:** Add both cloned repositories using their unique names. Use the appropriate path to your cloned repo.
    ```bash
    # Example, replace with your generated names and paths
    ./target/release/vectordb-cli repo add --name <unique_spoon_knife_name> -p /path/to/Spoon-Knife
    ./target/release/vectordb-cli repo add --name <unique_rust_book_name> -p /path/to/book
    ```
    *   Verify success messages.
    *   Test error handling by omitting `--name` or `-p`. (*Note:* Omitting `--name` might default to using the path as the name instead of erroring).
3.  **List Repositories:** Verify both added repositories are listed.
    ```bash
    ./target/release/vectordb-cli repo list
    ./target/release/vectordb-cli repo list --json
    ```
    *   Test plain text and `--json` output.
4.  **Use Repository:** Set one repository (`Spoon-Knife`) as active using its unique name.
    ```bash
    ./target/release/vectordb-cli repo use <unique_spoon_knife_name>
    ```
    *   Verify success message.
    *   Verify `(active)` marker in `repo list` output.
    *   Test error handling by providing a non-existent repository name.
5.  **Sync Repositories:** Sync content for both repositories. (*Note:* `sync` uses a positional argument for the name, unlike `clear`).
    ```bash
    # Sync non-active by name (positional argument)
    ./target/release/vectordb-cli repo sync <unique_rust_book_name>
    # Sync active (no name needed)
    ./target/release/vectordb-cli repo sync
    # Sync active again with flags (comma-separated extensions)
    ./target/release/vectordb-cli repo sync --force --extensions md,txt
    ```
    *   Verify output indicates syncing activity.
6.  **Repository Stats:** Get statistics for the active repository.
    ```bash
    ./target/release/vectordb-cli repo stats
    ```
    *   Verify command runs and outputs stats-related text.
7.  **Query Repositories:** Query both repositories. (*Note:* Use `repo query` subcommand. Takes name via `--name` unlike `sync` and `remove`).
    ```bash
    # Query active (Spoon-Knife)
    ./target/release/vectordb-cli repo query "Spoon-Knife"
    # Query non-active (rust-lang/book) by name with filters
    ./target/release/vectordb-cli repo query --name <unique_rust_book_name> --lang rust --json "borrow checker"
    ```
    *   **Manual Step:** Review the results for each query and assess their relevance on a scale of 1-10.
    *   Test error handling by omitting the query text.
8.  **Clear Repositories:** Clear the indexed content for both repositories. (*Note:* `clear` uses `--name` flag, unlike `sync` and `remove`).
    ```bash
    # Clear active (no name needed)
    ./target/release/vectordb-cli repo clear -y
    # Clear non-active by name
    ./target/release/vectordb-cli repo clear --name <unique_rust_book_name> -y
    ```
    *   Verify success messages.
9.  **Remove Repositories:** Remove the configuration for both repositories. (*Note:* `remove` uses a positional argument for the name, unlike `clear`).
    ```bash
    # Remove by name (positional argument)
    ./target/release/vectordb-cli repo remove <unique_spoon_knife_name> -y
    ./target/release/vectordb-cli repo remove <unique_rust_book_name> -y
    ```
    *   Verify success messages.
    *   Test error handling by providing a non-existent repository name.

## Phase 2: Simple Commands Testing

**Goal:** Test the non-repository index commands (`simple index`, `simple query`, `simple clear`).

**Setup:**
1.  Create a simple text file (`simple_test.txt`) with test content in the temp directory. Example content: "This is a file for the E2E test."

**Test Steps (Execute these commands manually):**
1.  **Index File:** Index the test file using `simple index`. (*Note:* This command clears the index before running).
    ```bash
    ./target/release/vectordb-cli simple index /path/to/simple_test.txt --extension txt
    ./target/release/vectordb-cli simple index /path/to/simple_test.txt
    ```
    *   Verify command runs successfully.
    *   Test error handling by providing a non-existent file path.
2.  **Query Index:** Query the simple index for content from the test file. (*Note:* Check actual `lang` and `type` from results before filtering).
    ```bash
    ./target/release/vectordb-cli simple query "E2E test"
    # Example filter (adjust lang/type based on actual results)
    ./target/release/vectordb-cli simple query --lang fallback --type fallback_chunk_0 --limit 5 --json "E2E test"
    ```
    *   **Manual Step:** Review the results and rate relevance (1-10).
    *   Test error handling by omitting query text.
3.  **Clear Index:** Clear the simple index using `simple clear`. (*Note:* `-y` flag is not supported/needed).
    ```bash
    ./target/release/vectordb-cli simple clear
    ```
    *   Verify success message.

## Phase 3: Edit Command Testing

**Goal:** Test the basic functionality of the `edit validate` and `edit apply` commands.

**Setup:**
1.  Create a Python file (`edit_test.py`) with simple content within the `Spoon-Knife` repository clone (use the path from Phase 1 Setup). Example:
    ```python
    # edit_test.py
    def hello():
        print("Hello, world!")

    def goodbye():
        print("Goodbye, world!")
    ```
2.  Navigate to the `Spoon-Knife` directory and commit the file:
    ```bash
    cd /path/to/Spoon-Knife
    git add edit_test.py
    git commit -m "Add edit_test.py for E2E testing"
    cd - # Return to previous directory
    ```
3.  Run `repo add`, `repo use`, and `repo sync` for the `Spoon-Knife` repository (using its unique name) to ensure the new file is indexed.
    ```bash
    # Assuming repo was removed in Phase 1 cleanup
    ./target/release/vectordb-cli repo add --name <unique_spoon_knife_name> -p /path/to/Spoon-Knife
    ./target/release/vectordb-cli repo use <unique_spoon_knife_name>
    ./target/release/vectordb-cli repo sync
    ```

**Test Steps (Execute these commands manually):**
(*Note:* `--target-file` should be `--file`, `--replacement` should be `--edit-content`. The `-y` flag is not supported for `edit apply`. Verification with `grep` may be unreliable depending on shell.)
(*Note:* Applying multi-line edits via `--edit-content` might treat newlines literally. Using `@file` syntax is not supported.)

1.  **Validate Edit (Lines):** Use `edit validate` with line numbers. (Modify the print statement in `hello`).
    ```bash
    # Corrected arguments: --file, --edit-content
    ./target/release/vectordb-cli edit validate --file /path/to/Spoon-Knife/edit_test.py --start-line 3 --end-line 3 --edit-content '    print("Hello, E2E test!")'
    ```
    *   Verify validation passes.
2.  **Apply Edit (Lines):** Use `edit apply` with line numbers.
    ```bash
    # Corrected arguments, no -y
    ./target/release/vectordb-cli edit apply --file /path/to/Spoon-Knife/edit_test.py --start-line 3 --end-line 3 --edit-content '    print("Hello, E2E test!")'
    ```
    *   Verify edit is applied using `cat`.
    ```bash
    cat /path/to/Spoon-Knife/edit_test.py
    ```
3.  **Validate Edit (Semantic):** Use `edit validate` with `--element-query` (modify the `goodbye` function). (Use shell quoting like `$'...'` for newlines if needed, though effectiveness may vary).
    ```bash
    # Corrected arguments
    ./target/release/vectordb-cli edit validate --file /path/to/Spoon-Knife/edit_test.py --element-query 'function_definition:goodbye' --edit-content $'def goodbye():\\n    print("Farewell, E2E test!")'
    ```
    *   Verify validation passes.
4.  **Apply Edit (Semantic):** Use `edit apply` with `--element-query`.
    ```bash
    # Corrected arguments, no -y
    ./target/release/vectordb-cli edit apply --file /path/to/Spoon-Knife/edit_test.py --element-query 'function_definition:goodbye' --edit-content $'def goodbye():\\n    print("Farewell, E2E test!")'
    ```
    *   Verify edit is applied using `cat`. (*Note:* Check if newline was applied correctly).
    ```bash
    cat /path/to/Spoon-Knife/edit_test.py
    ```
5.  **Apply Edit (ONNX Flags):** Run `edit apply` while providing global ONNX flags (`-m`, `-t`). (Change `hello` back).
    ```bash
    # Corrected arguments, no -y
    ./target/release/vectordb-cli -m /path/to/model.onnx -t /path/to/tokenizer/dir edit apply --file /path/to/Spoon-Knife/edit_test.py --element-query 'function_definition:hello' --edit-content $'def hello():\\n    print("Hello, world!")'
    ```
    *   Verify edit is applied (flags should be accepted but not affect edit logic).
    *   Verify file content using `cat`.

## Cleanup

1.  Remove the temporary directory containing cloned repos and test files.
    ```bash
    rm -rf /path/to/temp_directory
    ```
2.  If you set temporary environment variables, unset them.
3.  Clean up any remaining `vectordb-cli` config/data if desired (check `~/.config/vectordb-cli/` and `~/.local/share/vectordb-cli/`).
4.  Optionally remove the test repositories from the CLI config if they weren't cleaned up by the test itself (e.g., if `repo remove` failed).

## Manual Rating

During the `repo query` and `