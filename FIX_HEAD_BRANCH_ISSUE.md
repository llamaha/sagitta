# Fix for HEAD Branch Issue

## Quick Fix for Existing Repository

To fix the repository that was added with branch "HEAD":

```bash
# 1. Remove the problematic repository
sagitta-cli repo remove helix

# 2. Re-add it properly
sagitta-cli repo add --url https://github.com/helix-editor/helix --name helix

# Or if specifying a branch:
sagitta-cli repo add --url https://github.com/helix-editor/helix --name helix --branch master
```

## Code Changes Needed

### 1. Update `prepare_repository` in `repo_helpers/repo_indexing.rs`

Add after line 366 (before "Handle target_ref"):

```rust
// Resolve HEAD and other special refs
let resolved_target_ref = if let Some(ref target) = target_ref_opt {
    if target == "HEAD" {
        // Open the repository to resolve HEAD
        let repo = git2::Repository::open(&final_local_path)
            .context("Failed to open repository to resolve HEAD")?;
        
        match resolve_git_ref(&repo, target) {
            Ok(resolved) => {
                info!("Resolved '{}' to '{}'", target, resolved);
                Some(resolved)
            }
            Err(e) => {
                warn!("Failed to resolve '{}': {}, using as-is", target, e);
                Some(target.to_string())
            }
        }
    } else {
        Some(target.to_string())
    }
} else {
    None
};

// Update the rest of the code to use resolved_target_ref instead of target_ref_opt
```

### 2. Add import at the top of the file:

```rust
use super::git_edge_cases::resolve_git_ref;
```

### 3. Update `mod.rs` to include the new module:

```rust
pub mod git_edge_cases;
```

## Alternative: Patch Database Directly (Not Recommended)

If you need to fix it without removing/re-adding:

1. Edit the config file directly:
```bash
# Find your config file location
# Usually ~/.config/sagitta/config.toml or similar

# Edit the repository entry for helix
# Change default_branch from "HEAD" to "master" (or appropriate branch)
# Change active_branch from "HEAD" to "master"
```

2. Clear and re-sync:
```bash
sagitta-cli repo clear --repo-name helix
sagitta-cli repo sync --name helix
```

## Prevention

The code changes above will prevent this from happening in the future by:

1. Detecting when someone specifies "HEAD" as a target
2. Resolving it to the actual branch name or commit
3. Validating branch names before using them
4. Providing better error messages for edge cases