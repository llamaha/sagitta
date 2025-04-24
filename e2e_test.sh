#!/bin/bash
# set -e # Temporarily disabled during final debugging, re-enable later
set -x

# Setup
TEMP_DIR=$(mktemp -d)
export TEST_TEMP_DIR="$TEMP_DIR"
export UNIQUE_NAME="e2e-test-$(date +%s)"
REPO_PATH="$TEMP_DIR/Spoon-Knife"
SIMPLE_FILE="$TEMP_DIR/simple_test.txt"
EDIT_FILE="$REPO_PATH/edit_test.py"

echo "[E2E] Setup"
git clone https://github.com/octocat/Spoon-Knife "$REPO_PATH"
echo "[E2E] Repo cloned"

# Test --help and --version
echo "[E2E] CLI --help and --version"
./target/release/vectordb-cli --help | grep "vectordb-cli"
./target/release/vectordb-cli --version | grep -E "[0-9]+\.[0-9]+"

# === Phase 1: Repo Commands ===
echo "[E2E] Phase 1: Repo Commands"
# Repo add (success & errors)
./target/release/vectordb-cli repo add --name "$UNIQUE_NAME" -p "$REPO_PATH" | grep "Repository added successfully"
! ./target/release/vectordb-cli repo add -p "$REPO_PATH" 2>&1 | grep "required"
! ./target/release/vectordb-cli repo add --name "$UNIQUE_NAME-err" 2>&1 | grep "required"

# Repo list (plain & JSON)
./target/release/vectordb-cli repo list | grep "$UNIQUE_NAME"
./target/release/vectordb-cli repo list --json | grep "$UNIQUE_NAME"

# Repo use (success & error)
./target/release/vectordb-cli repo use "$UNIQUE_NAME" | grep "Set active repository"
! ./target/release/vectordb-cli repo use 2>&1 | grep "required"

# Repo sync (with options, specific repo, active repo)
./target/release/vectordb-cli repo sync --force --extensions md,html "$UNIQUE_NAME"
./target/release/vectordb-cli repo sync "$UNIQUE_NAME"
./target/release/vectordb-cli repo sync

# Repo stats
./target/release/vectordb-cli repo stats | grep "Fetching stats"

# Repo query (with options, JSON, plain text, error)
./target/release/vectordb-cli repo query "Spoon-Knife" --branch main --limit 1 --lang markdown --type paragraph --json --name "$UNIQUE_NAME" | grep "\"payload\":"
./target/release/vectordb-cli repo query "Spoon-Knife" --limit 1 | grep "Score:"
! ./target/release/vectordb-cli repo query 2>&1 | grep "required"

# Repo clear
./target/release/vectordb-cli repo clear -y | grep "index cleared"

# === Phase 2: Simple Commands ===
echo "[E2E] Phase 2: Simple Commands"
# Simple index (with extension, without, error)
echo "Simple index content for E2E test." > "$SIMPLE_FILE"
./target/release/vectordb-cli simple index "$SIMPLE_FILE" --extension txt | grep "Indexing complete"
./target/release/vectordb-cli simple index "$SIMPLE_FILE" | grep "Indexing complete"
! ./target/release/vectordb-cli simple index 2>&1 | grep "required"

# Simple query (with options, JSON, plain text, error)
./target/release/vectordb-cli simple query "E2E test" --limit 1 --lang fallback --type fallback_chunk_0 --json | grep "\"payload\":"
./target/release/vectordb-cli simple query "E2E test" | grep "Score:"
! ./target/release/vectordb-cli simple query 2>&1 | grep "required"

# Simple clear
./target/release/vectordb-cli simple clear | grep "Successfully cleared"

# === Phase 3: Edit Commands ===
echo "[E2E] Phase 3: Edit Commands"
# Setup: Create and commit edit target file, then sync repo
echo -e "def hello():\n    print(\"Hello World\")" > "$EDIT_FILE"
(cd "$REPO_PATH" && git add edit_test.py && git -c user.name="Test User" -c user.email="test@example.com" commit -m "Add edit_test.py")
echo "[E2E] edit_test.py committed"
./target/release/vectordb-cli repo sync "$UNIQUE_NAME"
echo "[E2E] repo synced for edit tests"

# Edit validate (lines)
REPLACEMENT_CONTENT1="    print(\"Hello E2E Test By Line\")"
./target/release/vectordb-cli edit validate --file "$EDIT_FILE" --edit-content "$REPLACEMENT_CONTENT1" --start-line 2 --end-line 2 | grep "Validation passed"

# Edit apply (lines) & verify
./target/release/vectordb-cli edit apply --file "$EDIT_FILE" --edit-content "$REPLACEMENT_CONTENT1" --start-line 2 --end-line 2 | grep "Edit applied"
grep "Hello E2E Test By Line" "$EDIT_FILE"

# Edit validate (semantic)
REPLACEMENT_CONTENT2="    # Semantic edit target test"
./target/release/vectordb-cli edit validate --file "$EDIT_FILE" --element-query "function hello" --edit-content "$REPLACEMENT_CONTENT2" | grep "Validation passed"

# Edit apply (semantic) & verify
REPLACEMENT_CONTENT3="    print(\"Semantic Edit Applied Test\")"
./target/release/vectordb-cli edit apply --file "$EDIT_FILE" --element-query "function hello" --edit-content "$REPLACEMENT_CONTENT3" | grep "Edit applied"
grep "Semantic Edit Applied Test" "$EDIT_FILE"

# Edit apply (flags) & verify
REPLACEMENT_CONTENT4="# Flag Test Line"
./target/release/vectordb-cli edit apply --file "$EDIT_FILE" --start-line 1 --end-line 1 --edit-content "$REPLACEMENT_CONTENT4" --no-format --update-references --no-preserve-docs | grep "Edit applied"
grep "Flag Test Line" "$EDIT_FILE"

# Edit apply (onnx flags - should have no effect)
REPLACEMENT_CONTENT5="# ONNX Flag Test Line"
./target/release/vectordb-cli -m ./onnx/all-minilm-l6-v2.onnx -t ./onnx edit apply --file "$EDIT_FILE" --start-line 1 --end-line 1 --edit-content "$REPLACEMENT_CONTENT5" | grep "Edit applied"
grep "ONNX Flag Test Line" "$EDIT_FILE"

# === Cleanup ===
echo "[E2E] Cleanup repo"
# Repo remove (success & error) - Moved to end
./target/release/vectordb-cli repo remove "$UNIQUE_NAME" -y | grep "Repository configuration removed"
! ./target/release/vectordb-cli repo remove -y 2>&1 | grep "required"

echo "[E2E] Cleanup temp files"
rm -rf "$TEMP_DIR"
unset TEST_TEMP_DIR
unset UNIQUE_NAME
echo "E2E test completed." 