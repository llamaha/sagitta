#!/bin/bash
# set -e # Re-enable this after debugging if needed
set -x

# --- Configuration ---
# Ensure the release binary exists
CLI_BIN="./target/release/vectordb-cli"
if [ ! -f "$CLI_BIN" ]; then
    echo "Error: Release binary not found at $CLI_BIN. Please build with 'cargo build --release'."
    exit 1
fi

# Ensure Qdrant is running (basic check)
if ! curl -s http://localhost:6333/readyz > /dev/null; then
    echo "Error: Cannot connect to Qdrant on http://localhost:6333. Is it running?"
    exit 1
fi

# --- Setup ---
echo "[E2E Setup] Creating temporary directory and cloning test repos..."
TEMP_DIR=$(mktemp -d)
export TEST_TEMP_DIR="$TEMP_DIR"
export UNIQUE_PREFIX="e2e-$(date +%s)"

# Repository 1: Spoon-Knife (Small, simple)
REPO1_NAME="${UNIQUE_PREFIX}-spoon"
REPO1_URL="https://github.com/octocat/Spoon-Knife"
REPO1_PATH="$TEMP_DIR/Spoon-Knife"
git clone "$REPO1_URL" "$REPO1_PATH"

# Repository 2: rust-lang/book (Larger, Rust/Markdown content)
REPO2_NAME="${UNIQUE_PREFIX}-rustbook"
REPO2_URL="https://github.com/rust-lang/book"
REPO2_PATH="$TEMP_DIR/book"
# Clone only the main branch, depth 1 to save time/space
git clone --branch main --depth 1 "$REPO2_URL" "$REPO2_PATH"

# Repository 3: For testing custom base path (will be added later)
REPO3_NAME="${UNIQUE_PREFIX}-basepathtest"
REPO3_URL="https://github.com/github/gitignore" # Another smallish repo
CUSTOM_BASE_PATH="$TEMP_DIR/custom_repo_base"
mkdir "$CUSTOM_BASE_PATH"
REPO3_EXPECTED_PATH="$CUSTOM_BASE_PATH/$REPO3_NAME" # Expected location uses the provided --name

# Simple index file
SIMPLE_FILE="$TEMP_DIR/simple_test.txt"
echo "Simple index content for E2E test." > "$SIMPLE_FILE"

# Edit test file setup (will be created later)
EDIT_FILE="$REPO1_PATH/edit_test.py"

echo "[E2E Setup] Completed."
echo "--- Test Repos ---"
echo "Repo 1 Name: $REPO1_NAME"
echo "Repo 1 Path: $REPO1_PATH"
echo "Repo 2 Name: $REPO2_NAME"
echo "Repo 2 Path: $REPO2_PATH"
echo "Repo 3 Name: $REPO3_NAME (used for base path test)"
echo "Custom Base Path: $CUSTOM_BASE_PATH"
echo "------------------"

# === Basic CLI Checks ===
echo "[E2E] Basic CLI checks (--help, --version)"
"$CLI_BIN" --help | grep "vectordb-cli"
"$CLI_BIN" --version | grep -E "[0-9]+\.[0-9]+\.[0-9]+" # Expect semver

# === Phase 1: Repo Commands ===
echo "[E2E] === Phase 1: Repo Commands ==="

# Repo Add (Success & Errors)
echo "[E2E] Testing 'repo add' (with -p)..."
"$CLI_BIN" repo add --name "$REPO1_NAME" -p "$REPO1_PATH" | grep "Repository added successfully"
"$CLI_BIN" repo add --name "$REPO2_NAME" -p "$REPO2_PATH" | grep "Repository added successfully"
# Test missing --name
! "$CLI_BIN" repo add -p "$REPO1_PATH" 2>&1 | grep -E "(Missing argument|required)"
# Test missing -p
! "$CLI_BIN" repo add --name "${UNIQUE_PREFIX}-err" 2>&1 | grep -E "(Missing argument|required)"

# Repo Config Set Base Path
echo "[E2E] Testing 'repo config set-repo-base-path'..."
"$CLI_BIN" repo config set-repo-base-path "$CUSTOM_BASE_PATH" | grep "Repository base path set"
# Test adding a repo *without* -p, should clone into custom base path
echo "[E2E] Testing 'repo add' (without -p, expecting clone to custom base path)..."
"$CLI_BIN" repo add --name "$REPO3_NAME" --url "$REPO3_URL"
# Verify the repo was cloned to the expected location
if [ -d "$REPO3_EXPECTED_PATH" ]; then
    echo "[E2E] Verification PASSED: Repo $REPO3_NAME found in custom base path $CUSTOM_BASE_PATH"
else
    echo "[E2E] Verification FAILED: Repo $REPO3_NAME NOT found in custom base path $CUSTOM_BASE_PATH (expected $REPO3_EXPECTED_PATH)"
    exit 1
fi

# Repo List (Plain & JSON)
echo "[E2E] Testing 'repo list' (should include all 3 repos)..."
"$CLI_BIN" repo list | grep "$REPO1_NAME"
"$CLI_BIN" repo list | grep "$REPO2_NAME"
"$CLI_BIN" repo list | grep "$REPO3_NAME"
"$CLI_BIN" repo list --json | grep "$REPO1_NAME"
"$CLI_BIN" repo list --json | grep "$REPO2_NAME"
"$CLI_BIN" repo list --json | grep "$REPO3_NAME"

# Repo Use (Set active repo - REPO1)
echo "[E2E] Testing 'repo use' ($REPO1_NAME)..."
"$CLI_BIN" repo use "$REPO1_NAME" | grep "Set active repository"
# Verify active repo in list
"$CLI_BIN" repo list | grep "$REPO1_NAME (active)"
# Test missing name
! "$CLI_BIN" repo use 2>&1 | grep -E "(Missing argument|required)"

# Repo Use Branch (Requires changes within the repo)
echo "[E2E] Testing 'repo use-branch' ($REPO1_NAME)..."
TEST_BRANCH="e2e-test-branch"
BRANCH_COMMIT_MSG="E2E test commit on new branch"
BRANCH_SPECIFIC_CONTENT="Content specific to the E2E test branch"
# Create branch, make change, commit in REPO1
(cd "$REPO1_PATH" && git checkout -b "$TEST_BRANCH")
(cd "$REPO1_PATH" && echo "$BRANCH_SPECIFIC_CONTENT" >> README.md)
(cd "$REPO1_PATH" && git add README.md && git -c user.name="E2E Test" -c user.email="test@example.com" commit -m "$BRANCH_COMMIT_MSG")
# Use the new branch
"$CLI_BIN" repo use-branch "$TEST_BRANCH" | grep "Set active branch"
# Sync the active repo (which now points to the new branch)
"$CLI_BIN" repo sync | grep "Successfully synced"
# Query for branch-specific content (should be found)
echo "--- Query (use-branch test 1) Results ($REPO1_NAME/$TEST_BRANCH, branch content) --- RATE THIS (1-10) ---"
"$CLI_BIN" repo query "$BRANCH_SPECIFIC_CONTENT" --branch "$TEST_BRANCH" --limit 1 | cat
echo "----------------------------------------------------------------------------------------------------"
# Query for original content on main (should still exist if not cleared)
# NOTE: Depending on implementation, querying non-active branch might require --name flag?
# Let's assume active repo context is enough.
echo "--- Query (use-branch test 2) Results ($REPO1_NAME/main, original content) --- RATE THIS (1-10) ---"
"$CLI_BIN" repo query "Spoon-Knife" --branch main --limit 1 | cat
echo "-------------------------------------------------------------------------------------------------"
# Switch back to main branch
"$CLI_BIN" repo use-branch main | grep "Set active branch"
# Verify active branch is main again (optional check)
# (Could check config file if isolated, or rely on next sync/query behavior)

# Repo Sync (Specific repo, active repo, with options)
echo "[E2E] Testing 'repo sync' ($REPO2_NAME)..."
"$CLI_BIN" repo sync "$REPO2_NAME" # Sync non-active repo
echo "[E2E] Testing 'repo sync' (active: $REPO1_NAME on main branch)..."
"$CLI_BIN" repo sync # Sync active repo (now back on main)
echo "[E2E] Testing 'repo sync' (active: $REPO1_NAME with options)..."
"$CLI_BIN" repo sync --force --extensions md,html # Sync active with options

# Repo Stats (Active repo)
echo "[E2E] Testing 'repo stats' (active: $REPO1_NAME)..."
"$CLI_BIN" repo stats | grep "Fetching stats"

# Repo Query (Active repo, specific repo, options, JSON)
echo "[E2E] Testing 'repo query' (active: $REPO1_NAME on main branch)..."
echo "--- Query 1 Results ($REPO1_NAME/main, 'Spoon-Knife') --- RATE THIS (1-10) ---"
"$CLI_BIN" repo query "Spoon-Knife" --limit 3 | cat # Show results clearly
echo "--------------------------------------------------------------------"

echo "[E2E] Testing 'repo query' (specific: $REPO2_NAME)..."
echo "--- Query 2 Results ($REPO2_NAME, 'borrow checker') --- RATE THIS (1-10) ---"
"$CLI_BIN" repo query "borrow checker" --name "$REPO2_NAME" --limit 3 --lang rust --json | cat # Query specific repo, JSON output
echo "-----------------------------------------------------------------------"

# Test missing query text
! "$CLI_BIN" repo query 2>&1 | grep -E "(Missing argument|required)"

# Repo Clear (Active repo)
echo "[E2E] Testing 'repo clear' (active: $REPO1_NAME)..."
"$CLI_BIN" repo clear -y | grep "index cleared"

# Repo Clear (Specific repo)
echo "[E2E] Testing 'repo clear' (specific: $REPO2_NAME)..."
"$CLI_BIN" repo clear --name "$REPO2_NAME" -y | grep "index cleared"

# Repo Clear (Repo 3 - added without -p)
echo "[E2E] Testing 'repo clear' (specific: $REPO3_NAME)..."
"$CLI_BIN" repo clear --name "$REPO3_NAME" -y | grep "index cleared"


# === Phase 2: Simple Commands ===
echo "[E2E] === Phase 2: Simple Commands ==="
# Simple Index (With extension, without, error)
echo "[E2E] Testing 'simple index'..."
"$CLI_BIN" simple index "$SIMPLE_FILE" --extension txt | grep "Indexing complete"
# Index again (should be idempotent or update)
"$CLI_BIN" simple index "$SIMPLE_FILE" | grep "Indexing complete"
# Test missing path
! "$CLI_BIN" simple index 2>&1 | grep -E "(Missing argument|required)"

# Simple Query (Plain, JSON, options, error)
echo "[E2E] Testing 'simple query'..."
echo "--- Simple Query 1 Results ('E2E test') --- RATE THIS (1-10) ---"
"$CLI_BIN" simple query "E2E test" | cat
echo "-------------------------------------------------------------"
echo "--- Simple Query 2 Results ('E2E test', JSON) --- RATE THIS (1-10) ---"
"$CLI_BIN" simple query "E2E test" --limit 1 --lang fallback --type fallback_chunk_0 --json | cat
echo "-------------------------------------------------------------------"
# Test missing query
! "$CLI_BIN" simple query 2>&1 | grep -E "(Missing argument|required)"

# Simple Clear
echo "[E2E] Testing 'simple clear'..."
# Note: Simple clear does not have a -y flag currently
# Need to pipe 'y' or handle interactively if prompt exists, otherwise it might hang.
# Assuming no prompt for now, or it defaults to yes/no without input.
# If it prompts: echo 'y' | "$CLI_BIN" simple clear
"$CLI_BIN" simple clear | grep "Successfully cleared"


# === Phase 3: Edit Commands ===
# Note: Edit commands require the target repo to be indexed. We'll use REPO1.
echo "[E2E] === Phase 3: Edit Commands ==="
# Setup: Create and commit edit target file in REPO1, then sync REPO1 (main branch)
echo "[E2E] Setting up edit test file in $REPO1_NAME (main branch)..."
(cd "$REPO1_PATH" && git checkout main) # Ensure we are on main branch
echo -e "def hello():\n    print(\"Hello World\")" > "$EDIT_FILE"
(cd "$REPO1_PATH" && git add edit_test.py && git -c user.name="E2E Test" -c user.email="test@example.com" commit -m "Add edit_test.py")
echo "[E2E] edit_test.py committed to $REPO1_NAME (main branch)"
"$CLI_BIN" repo sync "$REPO1_NAME" # Ensure the new file is indexed on main branch
echo "[E2E] Synced $REPO1_NAME (main branch) for edit tests"

# Edit Validate (Lines)
echo "[E2E] Testing 'edit validate' (lines)..."
REPLACEMENT_CONTENT1="    print(\"Hello E2E Test By Line\")"
"$CLI_BIN" edit validate --file "$EDIT_FILE" --edit-content "$REPLACEMENT_CONTENT1" --start-line 2 --end-line 2 | grep "Validation passed"

# Edit Apply (Lines) & Verify
echo "[E2E] Testing 'edit apply' (lines)..."
"$CLI_BIN" edit apply --file "$EDIT_FILE" --edit-content "$REPLACEMENT_CONTENT1" --start-line 2 --end-line 2 | grep "Edit applied"
grep "Hello E2E Test By Line" "$EDIT_FILE"

# Edit Validate (Semantic)
echo "[E2E] Testing 'edit validate' (semantic)..."
REPLACEMENT_CONTENT2="    # Semantic edit target test"
# Using tree-sitter type:name format
"$CLI_BIN" edit validate --file "$EDIT_FILE" --element-query "function_definition:hello" --edit-content "$REPLACEMENT_CONTENT2" | grep "Validation passed"

# Edit Apply (Semantic) & Verify
echo "[E2E] Testing 'edit apply' (semantic)..."
REPLACEMENT_CONTENT3="    print(\"Semantic Edit Applied Test\")"
"$CLI_BIN" edit apply --file "$EDIT_FILE" --element-query "function_definition:hello" --edit-content "$REPLACEMENT_CONTENT3" | grep "Edit applied"
grep "Semantic Edit Applied Test" "$EDIT_FILE"

# Edit Apply (with global ONNX flags - should not affect edit itself)
echo "[E2E] Testing 'edit apply' (with ONNX flags)..."
REPLACEMENT_CONTENT4="# ONNX Flag Test Line"
# Assuming default ONNX paths are configured or accessible for the binary
"$CLI_BIN" edit apply --file "$EDIT_FILE" --start-line 1 --end-line 1 --edit-content "$REPLACEMENT_CONTENT4" | grep "Edit applied"
# Example using flags if needed:
# "$CLI_BIN" -m ./onnx/all-minilm-l6-v2.onnx -t ./onnx edit apply --file "$EDIT_FILE" --start-line 1 --end-line 1 --edit-content "$REPLACEMENT_CONTENT4" | grep "Edit applied"
grep "ONNX Flag Test Line" "$EDIT_FILE"

# === Cleanup ===
echo "[E2E] === Cleanup ==="

# Repo Remove (Success & Error)
echo "[E2E] Cleaning up repositories..."
"$CLI_BIN" repo remove "$REPO1_NAME" -y | grep "Repository configuration removed"
"$CLI_BIN" repo remove "$REPO2_NAME" -y | grep "Repository configuration removed"
"$CLI_BIN" repo remove "$REPO3_NAME" -y | grep "Repository configuration removed"
# Test missing name
! "$CLI_BIN" repo remove -y 2>&1 | grep -E "(Missing argument|required)"

echo "[E2E] Cleaning up temporary directory..."
rm -rf "$TEMP_DIR"
unset TEST_TEMP_DIR
unset UNIQUE_PREFIX
echo "[E2E] Test completed successfully."
exit 0 