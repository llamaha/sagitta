# VectorDB CLI E2E Testing Plan

This document outlines the steps for end-to-end testing of the `vectordb-cli` binary.

## Prerequisites

1.  **Compiled Binary:** The `vectordb-cli` binary compiled in release mode (e.g., `./target/release/vectordb-cli`).
2.  **ONNX Models:** Default embedding model (`all-minilm-l6-v2.onnx` or similar) and tokenizer (`tokenizer.json`) available. The CLI will look for them via config file (`~/.config/vectordb-cli/config.toml`), environment variables (`VECTORDB_ONNX_MODEL`, `VECTORDB_ONNX_TOKENIZER_DIR`), or command-line flags (`-m`/`--onnx-model`, `-t`/`--onnx-tokenizer-dir`). Ensure they are accessible.
3.  **Git:** Git command-line tool installed.
4.  **Qdrant:** A running Qdrant instance accessible (default GRPC on port 6334). Use `curl http://localhost:6334/readyz` to check.
5.  **Test Repositories:** Internet access to clone `https://github.com/octocat/Spoon-Knife` and `https://github.com/rust-lang/book`.

## Test Script

A script `e2e_test.sh` automates most of these steps. Run it from the project root:

    ```bash
./e2e_test.sh
```

The script includes basic checks for prerequisites and performs the tests described below. It uses `set -x` to show commands being executed and includes basic `grep` checks for expected output.

## Phase 1: Repo Commands Testing

**Goal:** Test repository management (`add`, `list`, `use`, `sync`, `stats`, `clear`, `remove`) and querying (`query`).

**Setup (Automated by `e2e_test.sh`):**
1.  Create a temporary directory.
2.  Clone test repositories (`octocat/Spoon-Knife`, `rust-lang/book`) into the temp directory.
3.  Generate unique names for the test repositories based on the current timestamp.

**Test Steps (Automated by `e2e_test.sh`):**
1.  **Basic CLI:** Check `--help` and `--version` flags.
2.  **Add Repositories:** Add both cloned repositories using unique names.
    *   Verify success messages.
    *   Test error handling for missing `--name` or `-p` arguments.
3.  **List Repositories:** Verify both added repositories are listed.
    *   Test plain text and `--json` output.
4.  **Use Repository:** Set one repository (`Spoon-Knife`) as active.
    *   Verify success message.
    *   Verify `(active)` marker in `repo list` output.
    *   Test error handling for missing repository name.
5.  **Sync Repositories:** Sync content for both repositories.
    *   Sync the non-active repository (`rust-lang/book`) by name.
    *   Sync the active repository (`Spoon-Knife`).
    *   Sync the active repository again using `--force` and `--extensions` flags.
6.  **Repository Stats:** Get statistics for the active repository.
    *   Verify command runs and outputs stats-related text.
7.  **Query Repositories:** Query both repositories.
    *   Query the active repository (`Spoon-Knife`) for relevant content.
    *   Query the non-active repository (`rust-lang/book`) by name using `--name`, filter flags (`--lang`), and `--json` output.
    *   **Manual Step:** The script will output the results for each query, clearly marked with `--- Query X Results --- RATE THIS (1-10) ---`. Manually review these results and assess their relevance on a scale of 1-10.
    *   Test error handling for missing query text.
8.  **Clear Repositories:** Clear the indexed content for both repositories.
    *   Clear the active repository using `-y` flag.
    *   Clear the non-active repository using `--name` and `-y` flags.
9.  **Remove Repositories:** Remove the configuration for both repositories.
    *   Remove both repositories by name using `-y` flag.
    *   Verify success messages.
    *   Test error handling for missing repository name.

## Phase 2: Simple Commands Testing

**Goal:** Test the non-repository index commands (`simple index`, `simple query`, `simple clear`).

**Setup (Automated by `e2e_test.sh`):**
1.  Create a simple text file (`simple_test.txt`) with test content in the temp directory.

**Test Steps (Automated by `e2e_test.sh`):**
1.  **Index File:** Index the test file using `simple index`.
    *   Index with an `--extension` flag.
    *   Index again without the flag (should be idempotent or update).
    *   Test error handling for missing file path.
2.  **Query Index:** Query the simple index for content from the test file.
    *   Query using plain text output.
    *   Query using `--json` and filter flags (`--lang`, `--type`, `--limit`).
    *   **Manual Step:** The script will output the results, marked for manual rating (1-10).
    *   Test error handling for missing query text.
3.  **Clear Index:** Clear the simple index using `simple clear`.
    *   Verify success message.
    *   *Note:* The script assumes `simple clear` doesn't require interactive confirmation. If it does, the script might need adjustment (e.g., `echo 'y' | ...`).

## Phase 3: Edit Command Testing

**Goal:** Test the basic functionality of the `edit validate` and `edit apply` commands.

**Setup (Automated by `e2e_test.sh`):**
1.  Create a Python file (`edit_test.py`) with simple content within the `Spoon-Knife` repository clone.
2.  Add and commit this file to the `Spoon-Knife` git repository.
3.  Run `repo sync` for the `Spoon-Knife` repository to ensure the new file is indexed.

**Test Steps (Automated by `e2e_test.sh`):**
1.  **Validate Edit (Lines):** Use `edit validate` with `--start-line`/`--end-line` to check a line-based modification.
    *   Verify validation passes.
2.  **Apply Edit (Lines):** Use `edit apply` with `--start-line`/`--end-line` to perform the modification.
    *   Verify edit is applied.
    *   Verify file content using `grep`.
3.  **Validate Edit (Semantic):** Use `edit validate` with `--element-query` using the `type:name` format (e.g., `function_definition:hello`).
    *   Verify validation passes.
4.  **Apply Edit (Semantic):** Use `edit apply` with `--element-query`.
    *   Verify edit is applied.
    *   Verify file content using `grep`.
5.  **Apply Edit (ONNX Flags):** Run `edit apply` while providing global ONNX flags (`-m`, `-t`).
    *   Verify edit is applied (flags should be accepted but not affect edit logic).
    *   Verify file content using `grep`.

## Cleanup (Automated by `e2e_test.sh`)

1.  Remove the temporary directory containing cloned repos and test files.
2.  Unset temporary environment variables.

## Manual Rating

During the `repo query` and `simple query` steps, the script will print the search results and prompt for a manual rating (1-10). Review the output at these points:

*   **Repo Query 1:** Results for "Spoon-Knife" in the Spoon-Knife repo.
*   **Repo Query 2:** Results for "borrow checker" in the rust-lang/book repo.
*   **Simple Query 1 & 2:** Results for "E2E test" in the simple index.

Assess the quality and relevance of the returned chunks based on the query. This provides a qualitative measure of search performance.

## Obsolete Phases

*   **Phase 4: Agent Command Testing:** Removed as the `agent` subcommand is obsolete.
*   **Phase 5: Config Command Testing:** Removed as the `config` subcommand is obsolete (config handled by file/flags/env vars).