# OpenAI Compatible Search and Path Issues Fix Plan

## Issues Identified

### 1. Search Files Tool Not Searching Recursively
- **Issue**: The `search_files` tool with pattern `*.rs` didn't find files in the `src/` subdirectory
- **Evidence**: After creating `src/main.rs`, searching for `*.rs` returned empty results
- **Expected**: Should find files in subdirectories when using glob patterns

### 2. Stream Finishing Abruptly
- **Issue**: The stream finished with `finish_reason: stop` while the AI was still working
- **Evidence**: Logs show stream completed right after tool execution started
- **Impact**: AI responses may be cut off mid-task

### 3. Working Directory Path Resolution Issues
- **Issue**: Shell commands with `working_directory: fibonacci-calculator/` fail
- **Evidence**: `ls -la src` failed with "No such file or directory" even with working_directory set
- **Questions**:
  - Is working_directory relative to current directory or repositories_base_dir?
  - How is the path resolution happening?
  - Why does the trailing slash matter?

### 4. Repository Context Confusion
- **Issue**: The AI seems confused about which directory it's operating in
- **Evidence**: Uses both absolute and relative paths inconsistently

## Root Cause Analysis

### Search Files Issue - RESOLVED ✅
- The glob pattern matching works as designed
- `*.rs` only matches files in the root directory
- `**/*.rs` is required to match files recursively in subdirectories
- This is the expected behavior per the tests in fs_utils_test.rs

### Working Directory Issue - IDENTIFIED
- The `working_directory` parameter in shell_execute is used directly without resolution
- When passed `"fibonacci-calculator/"`, it's treated as a relative path from the current process working directory
- It should be resolved relative to:
  1. The `repositories_base_path` from AppConfig
  2. OR the current repository path (if one is active)
- The handler doesn't know that "fibonacci-calculator" is a repository name that should be resolved

### Stream Termination Issue
- Could be related to:
  1. Token limits in the OpenAI compatible API
  2. Timeout settings
  3. Model-specific behavior (Devstral might have different completion patterns)

## Fix Plan

### Phase 1: Document Search Pattern Behavior ✅
- The search works correctly - users need to use `**/*.rs` for recursive search
- Update documentation and AI prompts to clarify this

### Phase 2: Fix Working Directory Resolution
1. Modify `handle_shell_execute` to resolve relative working directories
2. Check if the working_directory looks like a repository name
3. If so, resolve it relative to repositories_base_path
4. Otherwise, check if it's a path relative to the current repository
5. Add proper path validation and error messages

### Proposed Fix for shell_execute.rs:
```rust
// In handle_shell_execute function, replace lines 123-129 with:
if let Some(ref dir) = params.working_directory {
    let path = PathBuf::from(dir);
    
    // If it's already absolute, use as-is
    if path.is_absolute() {
        cmd.current_dir(&path);
    } else {
        // Try to resolve relative to repositories base path
        let config_guard = _config.read().await;
        if let Some(base_path) = &config_guard.repositories_base_path {
            let repo_path = PathBuf::from(base_path).join(&path);
            if repo_path.exists() && repo_path.is_dir() {
                cmd.current_dir(&repo_path);
                log::info!("Resolved working directory to repository: {}", repo_path.display());
            } else {
                // Try relative to current repository
                if let Some(current_repo) = get_current_repository_path().await {
                    let relative_path = current_repo.join(&path);
                    if relative_path.exists() && relative_path.is_dir() {
                        cmd.current_dir(&relative_path);
                        log::info!("Resolved working directory relative to current repo: {}", relative_path.display());
                    } else {
                        // Fall back to using as-is (will likely fail)
                        cmd.current_dir(&path);
                        log::warn!("Could not resolve working directory: {}", dir);
                    }
                } else {
                    cmd.current_dir(&path);
                }
            }
        } else {
            // No base path configured, use as-is
            cmd.current_dir(&path);
        }
    }
} else if let Some(repo_path) = get_current_repository_path().await {
    cmd.current_dir(&repo_path);
    log::info!("Using current repository as working directory: {}", repo_path.display());
}
```

### Phase 3: Debug Stream Termination
1. Add logging to track why streams are finishing
2. Check if there are token limits being hit
3. Investigate Devstral-specific behavior
4. Consider implementing continuation logic for cut-off responses

### Phase 4: Improve Repository Context Handling
1. Make working directory handling more explicit
2. Add better error messages when paths don't exist
3. Provide clearer feedback about current context

## Implementation Steps

### Step 1: Search Tool Investigation
```bash
# Test various glob patterns
pattern: "*.rs"          # Current - not recursive
pattern: "**/*.rs"       # Should be recursive
pattern: "src/*.rs"      # Explicit directory
```

### Step 2: Working Directory Debugging
- Add debug logging to shell command execution
- Log the resolved absolute path
- Check how repositories_base_dir is used

### Step 3: Path Resolution Fix
- Ensure working_directory is properly joined with base path
- Normalize paths to handle trailing slashes
- Validate directory exists before execution

### Step 4: Stream Handling Improvements
- Log token usage if available
- Track stream termination reasons
- Implement retry logic for premature stops

## Testing Strategy

1. **Search Recursion Test**:
   - Create nested directory structure
   - Test various glob patterns
   - Verify recursive search works

2. **Working Directory Test**:
   - Test with various path formats
   - Test with/without trailing slashes
   - Test relative vs absolute paths

3. **Stream Completion Test**:
   - Monitor long-running tasks
   - Check token usage patterns
   - Test with different models

## Success Criteria

- [ ] Search files finds files in subdirectories with appropriate patterns
- [ ] Shell commands work correctly with working_directory parameter
- [ ] Working directory is resolved consistently and correctly
- [ ] Streams complete tasks without premature termination
- [ ] Clear error messages when paths don't exist
- [ ] Documentation updated with correct usage patterns