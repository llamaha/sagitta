# Git Repository Management Robustness Implementation

## Summary of Changes

### 1. Edge Case Handling Module (`git_edge_cases.rs`)

Added comprehensive edge case handling:
- **HEAD Resolution**: Resolves "HEAD" to actual branch name or commit hash
- **Reference Validation**: Validates branch/ref names before use
- **Default Branch Detection**: Detects actual default branch from remote
- **Working Tree Checks**: Verifies clean working tree before operations
- **Current Branch Detection**: Safely gets current branch, handling detached HEAD

### 2. Repository Recovery Module (`recovery.rs`)

Added recovery mechanisms for common issues:
- **Abort In-Progress Operations**: Aborts merge/rebase/cherry-pick
- **Clean Lock Files**: Removes stale .git lock files
- **Partial Clone Detection**: Identifies incomplete clones
- **Failed Add Cleanup**: Removes partial clones and orphaned collections

### 3. Updated `prepare_repository` Function

Enhanced with:
- Special ref resolution (HEAD → actual branch)
- Branch name validation
- Better error messages
- Graceful handling of edge cases

### 4. Comprehensive Test Suite

#### Unit Tests (`git_edge_cases_tests.rs`)
- Reference name validation
- HEAD resolution on branches
- Working tree clean checks

#### Integration Tests (`git_edge_cases_integration_tests.rs`)
- Adding repo with HEAD as target
- Invalid branch names
- Detached HEAD states
- Uncommitted changes during sync
- Empty repositories
- Non-existent branches
- Special characters in names
- Force push scenarios
- Concurrent operations

### 5. Key Improvements

1. **Input Validation**
   - Rejects invalid patterns like "refs/heads/main" (should be just "main")
   - Validates special characters
   - Clear error messages

2. **State Management**
   - Detects repository states (merge conflicts, etc.)
   - Handles detached HEAD gracefully
   - Checks for uncommitted changes

3. **Error Recovery**
   - Cleans up partial operations
   - Removes orphaned data
   - Provides actionable error messages

4. **User Experience**
   - Clear, helpful error messages
   - Suggests fixes for common issues
   - Handles edge cases transparently

## Usage Examples

### For Users

The system now handles these scenarios gracefully:

```bash
# Adding with HEAD reference - now resolved to actual branch
sagitta-cli repo add --url https://github.com/org/repo --target-ref HEAD

# Invalid branch names - clear error
sagitta-cli repo add --url https://github.com/org/repo --branch refs/heads/main
# Error: Reference 'refs/heads/main' looks like a full ref path. Use just the branch name instead.

# Repository in bad state - can recover
sagitta-cli repo sync --name my-repo
# Automatically handles uncommitted changes, lock files, etc.
```

### For Developers

```rust
// Resolve special refs
let resolved = resolve_git_ref(&repo, "HEAD")?;

// Validate branch names
validate_ref_name("feature/new-ui")?;

// Check working tree
if !check_working_tree_clean(&repo_path).await? {
    // Handle uncommitted changes
}

// Recover repository
recover_repository(&repo_path).await?;
```

## Testing

Run all robustness tests:
```bash
./test_git_robustness.sh
```

Individual test suites:
```bash
# Edge case unit tests
cargo test -p sagitta-search git_edge_cases

# Integration tests
cargo test -p sagitta-search integration_tests

# Recovery tests
cargo test -p sagitta-search recovery
```

## Future Improvements

1. **Stash Management**: Auto-stash uncommitted changes
2. **Conflict Resolution**: Help users resolve merge conflicts
3. **Progress Recovery**: Resume interrupted operations
4. **Diagnostic Commands**: `repo check`, `repo fix`
5. **Better Logging**: Track all git operations for debugging

## Benefits

- **Fewer User Errors**: Invalid inputs caught early
- **Better Recovery**: Can recover from bad states
- **Clear Communication**: Helpful error messages
- **Reliable Operations**: Handles edge cases gracefully
- **Maintainable Code**: Well-tested edge cases