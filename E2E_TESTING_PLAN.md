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
1.  **Generate unique names:** Generate unique names for the test repositories based on the current timestamp (e.g., `spoon_knife_$(date +%s)`, `rust_book_$(date +%s)`). The `repo add` command will clone these into the default CLI repository base path (often `~/.local/share/vectordb-cli/repositories/`).

**Test Steps (Execute these commands manually):**
1.  **Basic CLI:** Check `--help` and `--version` flags.
    ```bash
    ./target/release/vectordb-cli --help
    ./target/release/vectordb-cli --version
    ```
2.  **Add Repositories:** Add both repositories using their unique names and URLs. The CLI will clone them.
    ```bash
    # Example, replace with your generated names
    ./target/release/vectordb-cli repo add --name <unique_spoon_knife_name> --url https://github.com/octocat/Spoon-Knife.git
    ./target/release/vectordb-cli repo add --name <unique_rust_book_name> --url https://github.com/rust-lang/book.git
    ```
    *   Verify success messages (including the clone path).
    *   Test error handling by omitting `--name` or `--url`.
3.  **List Repositories:** Verify both added repositories are listed.
    ```bash
    ./target/release/vectordb-cli repo list
    ./target/release/vectordb-cli repo list --json
    ```
    *   Test plain text and `--json` output. Check reported local paths.
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
9.  **Remove Repositories:** Remove the configuration for both repositories. This should also remove the cloned directories from the repository base path. (*Note:* `remove` uses a positional argument for the name, unlike `clear`).
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
1.  Create a simple text file (`simple_test.txt`) in the current workspace directory. Example content: "This is a file for the E2E test."
    ```bash
    echo "This is a file for the E2E test." > simple_test.txt
    ```

**Test Steps (Execute these commands manually):**
1.  **Index File:** Index the test file using `simple index`. (*Note:* This command clears the index before running).
    ```bash
    ./target/release/vectordb-cli simple index ./simple_test.txt --extension txt
    ./target/release/vectordb-cli simple index ./simple_test.txt
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
1.  **Determine Repo Path:** Identify the path where the `<unique_spoon_knife_name>` repository was cloned by the `repo add` command in Phase 1 (check the output of `repo add` or `repo list`). Let's call this `<spoon_knife_path>`.
2.  Create a Python file (`edit_test.py`) in the current workspace directory. Example:
    ```python
    # edit_test.py
    def hello():
        print("Hello, world!")

    def goodbye():
        print("Goodbye, world!")
    ```
    ```bash
    cat << EOF > edit_test.py
    # edit_test.py
    def hello():
        print("Hello, world!")

    def goodbye():
        print("Goodbye, world!")
    EOF
    ```
3.  **Add and Commit:** Copy the test file into the managed repository, navigate there, commit it, and return.
    ```bash
    cp ./edit_test.py <spoon_knife_path>/edit_test.py
    cd <spoon_knife_path>
    git add edit_test.py
    git commit -m "Add edit_test.py for E2E testing"
    cd - # Return to previous directory
    ```
    *   **Note:** Replace `<spoon_knife_path>` with the actual path identified in step 1.
4.  **Re-Sync:** Run `repo sync` for the `Spoon-Knife` repository (using its unique name) to ensure the new file is indexed.
    ```bash
    # Sync by name (positional argument)
    ./target/release/vectordb-cli repo sync <unique_spoon_knife_name>
    # Ensure it's the active repo if applying edits without --name
    ./target/release/vectordb-cli repo use <unique_spoon_knife_name>
    ```

**Test Steps (Execute these commands manually):**
(*Note:* `--target-file` should be `--file`, `--replacement` should be `--edit-content`. The `-y` flag is not supported for `edit apply`. Verification with `grep` may be unreliable depending on shell.)
(*Note:* Applying multi-line edits via `--edit-content` might treat newlines literally. Using `@file` syntax is not supported.)
(*Note:* Replace `<spoon_knife_path>` with the actual path identified in Setup step 1.)

1.  **Validate Edit (Lines):** Use `edit validate` with line numbers. (Modify the print statement in `hello`).
    ```bash
    # Corrected arguments: --file, --edit-content
    ./target/release/vectordb-cli edit validate --file <spoon_knife_path>/edit_test.py --start-line 3 --end-line 3 --edit-content '    print("Hello, E2E test!")'
    ```
    *   Verify validation passes.
2.  **Apply Edit (Lines):** Use `edit apply` with line numbers.
    ```bash
    # Corrected arguments, no -y
    ./target/release/vectordb-cli edit apply --file <spoon_knife_path>/edit_test.py --start-line 3 --end-line 3 --edit-content '    print("Hello, E2E test!")'
    ```
    *   Verify edit is applied using `cat`.
    ```bash
    cat <spoon_knife_path>/edit_test.py
    ```
3.  **Validate Edit (Semantic):** Use `edit validate` with `--element-query` (modify the `goodbye` function). (Use shell quoting like `$'...'` for newlines if needed, though effectiveness may vary).
    ```bash
    # Corrected arguments
    ./target/release/vectordb-cli edit validate --file <spoon_knife_path>/edit_test.py --element-query 'function_definition:goodbye' --edit-content $'def goodbye():\\n    print("Farewell, E2E test!")'
    ```
    *   Verify validation passes.
4.  **Apply Edit (Semantic):** Use `edit apply` with `--element-query`.
    ```bash
    # Corrected arguments, no -y
    ./target/release/vectordb-cli edit apply --file <spoon_knife_path>/edit_test.py --element-query 'function_definition:goodbye' --edit-content $'def goodbye():\\n    print("Farewell, E2E test!")'
    ```
    *   Verify edit is applied using `cat`. (*Note:* Check if newline was applied correctly).
    ```bash
    cat <spoon_knife_path>/edit_test.py
    ```
5.  **Apply Edit (ONNX Flags):** Run `edit apply` while providing global ONNX flags (`-m`, `-t`). (Change `hello` back).
    ```bash
    # Corrected arguments, no -y. Use correct paths for your models.
    ./target/release/vectordb-cli -m /path/to/model.onnx -t /path/to/tokenizer/dir edit apply --file <spoon_knife_path>/edit_test.py --element-query 'function_definition:hello' --edit-content $'def hello():\\n    print("Hello, world!")'
    ```
    *   Verify edit is applied (flags should be accepted but not affect edit logic).
    *   Verify file content using `cat`.

## Cleanup

1.  Remove the test files created in the workspace.
    ```bash
    rm ./simple_test.txt ./edit_test.py
    ```
2.  If you set temporary environment variables, unset them.
3.  Clean up any remaining `vectordb-cli` config/data if desired (check `~/.config/vectordb-cli/` and `~/.local/share/vectordb-cli/`). The repositories added during the test should have been removed by the `repo remove` steps.
4.  Optionally remove the test repositories from the CLI config if they weren't cleaned up by the test itself (e.g., if `repo remove` failed).

## Manual Rating

During the `repo query` and `simple query` steps, manually rate the relevance of the returned results on a scale of 1 (irrelevant) to 10 (perfect match). Record these ratings for feedback.