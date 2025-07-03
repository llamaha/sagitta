# Git Edge Cases Test Plan for Sagitta

## Issue: Repository Added with Branch "HEAD"

### Root Cause
When adding a repository with `target_ref = "HEAD"`, the system treats "HEAD" as a branch name rather than resolving it to the actual branch or commit. This causes sync failures because `refs/heads/HEAD` doesn't exist.

### Edge Cases to Test

## 1. Detached HEAD States

### Test Cases:
- **Adding repo at specific commit**: Clone and checkout a specific commit hash
- **Adding repo at tag**: Clone and checkout a specific tag
- **Adding repo with HEAD as target_ref**: Current issue
- **Adding repo in detached state**: Local repo already in detached HEAD

### Expected Behavior:
- Should resolve HEAD to actual commit hash
- Should determine the actual branch name if on a branch
- Should handle detached HEAD gracefully (use commit hash as reference)

## 2. Branch Name Edge Cases

### Test Cases:
- **Non-existent default branch**: Repo has "develop" as default, not "main" or "master"
- **Special characters in branch names**: `feature/user@domain`, `bugfix/issue#123`
- **Unicode branch names**: Branch names with emojis or non-ASCII characters
- **Very long branch names**: Names exceeding typical limits
- **Branch names that look like refs**: `heads/main`, `refs/heads/feature`

### Expected Behavior:
- Should detect actual default branch from remote
- Should handle special characters in collection names (already uses hash)
- Should not fail on unusual branch names

## 3. Repository State Issues

### Test Cases:
- **Uncommitted changes**: Trying to switch branches with uncommitted changes
- **Merge conflicts**: Repository in middle of merge
- **Rebase in progress**: Repository in middle of rebase
- **Empty repository**: No commits yet
- **Corrupted repository**: Missing git objects

### Expected Behavior:
- Should provide clear error messages
- Should not leave system in inconsistent state
- Should suggest recovery actions

## 4. Remote Repository Issues

### Test Cases:
- **No remote configured**: Local-only repository
- **Multiple remotes**: Which remote to use?
- **Remote branch deleted**: Local branch exists but remote doesn't
- **Force-pushed history**: Remote history diverged
- **Network failures during clone**: Partial clone state

### Expected Behavior:
- Should handle local-only repos (already does with `added_as_local_path`)
- Should use specified remote or default to "origin"
- Should handle missing remotes gracefully

## 5. Authentication and Access Issues

### Test Cases:
- **SSH key failures**: Wrong key, no key, passphrase issues
- **HTTPS credential failures**: Private repos without credentials
- **Expired credentials**: Tokens that worked before but expired
- **Rate limiting**: GitHub/GitLab rate limits

### Expected Behavior:
- Clear error messages about auth failures
- Should not store credentials insecurely
- Should handle rate limits with retries

## Implementation Fixes Needed

### 1. Fix HEAD Resolution
```rust
// In prepare_repository, after cloning/opening repo
let resolved_ref = if target_ref == Some("HEAD") {
    // Resolve HEAD to actual branch or commit
    let head = repo.head()?;
    if head.is_branch() {
        head.shorthand().unwrap_or("main").to_string()
    } else {
        // Detached HEAD - use commit hash
        head.target().map(|oid| oid.to_string()).unwrap_or("main".to_string())
    }
} else {
    target_ref.unwrap_or(final_branch).to_string()
};
```

### 2. Better Default Branch Detection
```rust
// Instead of defaulting to "main", detect from remote
async fn detect_default_branch(repo_path: &Path) -> Result<String> {
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(&["symbolic-ref", "refs/remotes/origin/HEAD"])
        .output()
        .await?;
    
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Extract branch name from refs/remotes/origin/HEAD -> refs/remotes/origin/main
        if let Some(branch) = stdout.split('/').last() {
            return Ok(branch.trim().to_string());
        }
    }
    
    // Fallback: try common defaults
    for branch in &["main", "master", "develop", "trunk"] {
        let check = Command::new("git")
            .current_dir(repo_path)
            .args(&["show-ref", "--verify", &format!("refs/remotes/origin/{}", branch)])
            .output()
            .await?;
        
        if check.status.success() {
            return Ok(branch.to_string());
        }
    }
    
    Ok("main".to_string()) // Ultimate fallback
}
```

### 3. Validate Branch Names
```rust
fn validate_branch_name(branch: &str) -> Result<(), String> {
    // Check for special refs that shouldn't be used as branch names
    if branch == "HEAD" || branch.starts_with("refs/") {
        return Err(format!("'{}' is not a valid branch name", branch));
    }
    
    // Check for other invalid patterns
    if branch.is_empty() || branch.contains("..") || branch.ends_with('.') {
        return Err(format!("Invalid branch name: '{}'", branch));
    }
    
    Ok(())
}
```

### 4. Handle Uncommitted Changes
```rust
async fn check_working_tree_clean(repo_path: &Path) -> Result<bool> {
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(&["status", "--porcelain"])
        .output()
        .await?;
    
    Ok(output.stdout.is_empty())
}

// Before switching branches
if !check_working_tree_clean(&repo_path).await? {
    return Err(anyhow!("Cannot switch branches: uncommitted changes present. Please commit or stash changes first."));
}
```

### 5. Robust Error Recovery
```rust
// Wrap operations in transactions where possible
async fn add_repository_with_recovery(args: AddRepoArgs) -> Result<RepositoryConfig> {
    let result = add_repository_impl(args.clone()).await;
    
    if let Err(e) = &result {
        // Clean up partial state
        if let Some(path) = determine_repo_path(&args) {
            if path.exists() && is_partial_clone(&path) {
                warn!("Cleaning up partial clone at {:?}", path);
                fs::remove_dir_all(path).ok();
            }
        }
        
        // Clean up Qdrant collection if created
        if let Some(collection) = determine_collection_name(&args) {
            client.delete_collection(collection).await.ok();
        }
    }
    
    result
}
```

## Test Implementation Strategy

1. **Unit Tests**: Test individual functions with mocked git operations
2. **Integration Tests**: Use real git repositories in temp directories
3. **Edge Case Tests**: Specific tests for each edge case identified
4. **Property-Based Tests**: Generate random valid/invalid inputs
5. **Regression Tests**: Ensure fixed issues don't reoccur

## Monitoring and Diagnostics

1. **Better Logging**: Log git command outputs for debugging
2. **State Validation**: Validate repository state before operations
3. **Health Checks**: Periodic checks for repository consistency
4. **Error Context**: Include repository state in error messages