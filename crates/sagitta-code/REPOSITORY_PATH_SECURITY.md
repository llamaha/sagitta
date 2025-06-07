# Repository Path Security Documentation

## Overview

This document describes the security measures implemented in the `ProjectCreationTool` to ensure that projects are ONLY created within the designated `repositories_base_path`, preventing accidental project creation in system directories, current working directory, or other unintended locations.

## Security Requirements

**CRITICAL**: All projects MUST be created within the `repositories_base_path`. This path is either:
1. Explicitly configured via `with_repositories_base_path()`
2. Determined by the default XDG repository base path from `get_repo_base_path()`

## Implementation Details

### Path Resolution Logic

The `create_project` method enforces strict path validation:

1. **System Directory Rejection**: Paths starting with `/usr`, `/var`, `/etc`, `/sys`, `/proc`, or `/` are rejected outright
2. **Container Path Handling**: Paths like `/workspace` and `/app` are detected and converted to safe relative paths within `repositories_base_path`
3. **Temp Directory Redirection**: In production, temp directory paths are redirected to `repositories_base_path`
4. **Absolute Path Conversion**: Any absolute path is converted to use only the project name within `repositories_base_path`
5. **Final Security Check**: A final verification ensures the resolved path starts with `repositories_base_path`

### Test Mode Exception

For testing purposes only, a `test_mode` flag can be enabled via `with_test_mode(true)`:

- **Purpose**: Allows tests to create projects in temp directories (`/tmp/*`)
- **Scope**: ONLY applies to paths starting with `/tmp`
- **Production**: `test_mode` defaults to `false` and should NEVER be enabled in production

## Test Coverage

### Integration Tests (`tests/repository_path_validation.rs`)

1. **`test_prevent_system_directory_creation`**: Verifies system directories are rejected or redirected
2. **`test_container_path_handling`**: Tests container path detection and redirection  
3. **`test_relative_path_handling`**: Ensures relative paths work correctly
4. **`test_no_repository_base_path_configured`**: Tests fallback to default XDG path
5. **`test_temp_directory_paths`**: Tests both production (redirect) and test mode (allow) behaviors
6. **`test_current_working_directory_protection`**: Prevents accidental CWD project creation
7. **`test_path_validation_edge_cases`**: Handles empty, `.`, `..`, and other edge cases
8. **`test_repositories_base_path_enforcement`**: Verifies all projects go to configured base path

### Unit Tests (`src/tools/project_creation.rs`)

1. **`test_system_directory_rejection`**: Unit test for system directory rejection
2. **`test_container_path_redirection`**: Unit test for container path handling
3. **`test_repositories_base_path_enforcement`**: Unit test for base path enforcement  
4. **`test_current_working_directory_protection`**: Unit test for CWD protection
5. **`test_edge_case_path_handling`**: Unit test for edge case handling
6. **`test_temp_directory_handling`**: Test mode behavior for temp directories
7. **`test_temp_directory_redirection_in_production`**: Production behavior for temp directories
8. **`test_parameter_validation`**: Basic parameter validation

## Security Guarantees

With these measures in place:

✅ **No system directory pollution**: Projects cannot be created in `/usr`, `/var`, `/etc`, etc.
✅ **No CWD accidents**: Projects cannot accidentally pollute the current working directory
✅ **Consistent location**: All projects are created within the designated repository area
✅ **Container-aware**: Handles container paths like `/workspace` safely
✅ **Test-friendly**: Allows temp directory usage in test mode only
✅ **Default fallback**: Uses XDG-compliant default if no base path configured

## Usage Examples

### Production Usage (Safe)
```rust
let tool = ProjectCreationTool::new(working_dir)
    .with_repositories_base_path(Some(PathBuf::from("/home/user/repos")));
// test_mode defaults to false - all projects go to /home/user/repos/*
```

### Test Usage (Controlled)
```rust
let tool = ProjectCreationTool::new(working_dir)
    .with_repositories_base_path(Some(temp_dir.path().to_path_buf()))
    .with_test_mode(true); // Allows /tmp/* paths for testing
```

## What This Prevents

❌ **Accidental system pollution**: `sagitta-code create --path /usr/local/my-project`
❌ **CWD contamination**: `sagitta-code create --path ./my-project` (in sensitive directory)
❌ **Container path confusion**: `sagitta-code create --path /workspace/project`
❌ **Temp directory persistence**: Projects accidentally created in `/tmp`

## Error Messages

When dangerous paths are detected, users see helpful error messages:

```
Cannot create project in system directory '/usr/local/my-project'. 
All projects must be created within the repository base path. 
Please use a relative path like 'my-project' or 'projects/my-project'.
```

This guides users toward safe usage patterns while preventing security issues. 