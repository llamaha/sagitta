# Repository Management Robustness Improvements

## Summary of Issues and Solutions

### 1. Immediate Fix for "HEAD" Branch Issue

**Problem**: Repository was added with `branch = "HEAD"` instead of resolving to actual branch name.

**Quick Fix**:
```bash
sagitta-cli repo remove helix
sagitta-cli repo add --url https://github.com/helix-editor/helix --name helix
```

**Code Fix**: Add HEAD resolution in `prepare_repository`:
```rust
// After line 366 in repo_helpers/repo_indexing.rs
if target_ref_opt == Some("HEAD") {
    let repo = Repository::open(&final_local_path)?;
    let head = repo.head()?;
    if head.is_branch() {
        target_ref_opt = Some(head.shorthand().unwrap_or("main"));
    }
}
```

### 2. Key Robustness Improvements

#### A. Better Branch Detection
- Detect actual default branch from remote instead of assuming "main"
- Handle non-standard default branches (develop, trunk, etc.)
- Resolve special refs like HEAD to actual values

#### B. Input Validation
- Validate branch names before using them
- Reject invalid patterns (empty, "refs/heads/", "..", etc.)
- Provide clear error messages for invalid inputs

#### C. State Validation
- Check for uncommitted changes before branch operations
- Detect repository states (merge conflicts, rebase in progress)
- Handle empty repositories gracefully

#### D. Error Recovery
- Clean up partial clones on failure
- Remove orphaned Qdrant collections
- Provide actionable error messages

### 3. Testing Strategy

1. **Unit Tests**: Test individual validation functions
2. **Integration Tests**: Test full add/sync workflows
3. **Edge Case Tests**: Specific scenarios (detached HEAD, empty repos)
4. **Regression Tests**: Ensure fixed bugs don't reoccur

### 4. Recommended Implementation Order

1. **Phase 1 - Critical Fixes** (Do Now)
   - Fix HEAD resolution issue
   - Add basic input validation
   - Improve error messages

2. **Phase 2 - Robustness** (Next Sprint)
   - Add working tree clean checks
   - Implement default branch detection
   - Add comprehensive tests

3. **Phase 3 - Polish** (Future)
   - Add recovery mechanisms
   - Improve progress reporting
   - Add diagnostics commands

### 5. New Commands to Consider

```bash
# Diagnostic commands
sagitta-cli repo check --name <repo>     # Validate repo state
sagitta-cli repo fix --name <repo>       # Try to fix common issues

# Better sync control
sagitta-cli repo sync --stash            # Stash changes before sync
sagitta-cli repo sync --force-clean      # Discard local changes

# Branch management
sagitta-cli repo switch-branch --name <repo> --branch <branch>
sagitta-cli repo list-branches --name <repo>
```

### 6. Configuration Improvements

Add to repository config:
```toml
[repositories.my-repo]
name = "my-repo"
url = "https://github.com/..."
default_branch = "main"        # Detected, not assumed
active_branch = "feature/x"    # Current branch
sync_strategy = "incremental"  # or "full"
auto_stash = true             # Stash before operations
strict_mode = true            # Fail on any issues
```

### 7. Monitoring and Alerting

- Log all git commands and their outputs
- Track sync failures and patterns
- Alert on repeated failures
- Collect metrics on sync performance

## Benefits

1. **Reliability**: Fewer mysterious failures
2. **User Experience**: Clear errors, suggested fixes
3. **Maintainability**: Better test coverage
4. **Performance**: Avoid unnecessary re-indexing
5. **Debuggability**: Better logs and diagnostics

## Next Steps

1. Fix the immediate HEAD issue
2. Add the git_edge_cases module
3. Update prepare_repository with validations
4. Add tests for edge cases
5. Document known issues and workarounds