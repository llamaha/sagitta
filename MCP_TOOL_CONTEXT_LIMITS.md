# MCP Tool Context Limits Implementation

## Overview

This document describes the implementation of mandatory context limits for MCP tools to prevent excessive output that could overwhelm AI context windows.

## Changes Made

### 1. Read File Tool

**Problem**: AI assistants could accidentally read entire large files, consuming excessive context.

**Solution**: Made `start_line` and `end_line` parameters mandatory with a maximum range of 400 lines.

#### Changes:
- Modified `ReadFileParams` struct to make `start_line` and `end_line` required (non-Option) fields
- Added validation in `handle_read_file_inner` to enforce maximum 400 line range
- Updated tool definition to reflect mandatory parameters
- Updated all tests to use the new parameter structure

#### Example Usage:
```json
{
  "file_path": "/path/to/file.rs",
  "start_line": 100,
  "end_line": 200
}
```

#### Error Example:
```json
{
  "code": -32603,
  "message": "Line range too large: 401 lines requested. Maximum allowed is 400 lines. 
             Please adjust start_line (1) and end_line (401) to request fewer lines."
}
```

### 2. Shell Execute Tool

**Problem**: Shell commands could produce unlimited output, overwhelming the AI context.

**Solution**: Made output filtering mandatory - at least one of `grep_pattern`, `head_lines`, or `tail_lines` must be specified.

#### Changes:
- Added validation in `handle_shell_execute` to require at least one filter
- Updated tool definition with `oneOf` schema constraint
- Modified all tests to include appropriate filters

#### Example Usage:
```json
{
  "command": "ls -la",
  "head_lines": 20
}
```

Or with grep:
```json
{
  "command": "cat large_log_file.log",
  "grep_pattern": "ERROR"
}
```

#### Error Example:
```json
{
  "code": -32603,
  "message": "At least one output filter must be specified (grep_pattern, head_lines, or tail_lines) to prevent excessive output. 
             Use head_lines to limit output to first N lines, tail_lines for last N lines, or grep_pattern to filter specific content."
}
```

## Benefits

1. **Predictable Context Usage**: AI assistants can't accidentally consume their entire context with a single tool call
2. **Better Performance**: Prevents unnecessary processing of large files or command outputs
3. **Improved Reliability**: Reduces risk of timeouts or memory issues from processing excessive data
4. **Clear Expectations**: Tool definitions clearly communicate the requirements upfront

## Migration Guide

For existing code using these tools:

### Read File
```rust
// Before
let params = ReadFileParams {
    file_path: "file.txt".to_string(),
    start_line: None,  // Read entire file
    end_line: None,
};

// After
let params = ReadFileParams {
    file_path: "file.txt".to_string(),
    start_line: 1,     // Must specify range
    end_line: 100,     // Maximum 400 lines
};
```

### Shell Execute
```rust
// Before
let params = ShellExecuteParams {
    command: "ls -la".to_string(),
    working_directory: None,
    timeout_ms: 5000,
    env: None,
    grep_pattern: None,     // No filters
    head_lines: None,
    tail_lines: None,
};

// After - must have at least one filter
let params = ShellExecuteParams {
    command: "ls -la".to_string(),
    working_directory: None,
    timeout_ms: 5000,
    env: None,
    grep_pattern: None,
    head_lines: Some(50),   // Show first 50 lines
    tail_lines: None,
};
```

## Testing

All existing tests have been updated to comply with the new requirements. Additional tests added:
- `test_read_file_400_line_limit`: Validates the 400 line maximum
- `test_shell_execute_requires_filter`: Validates filter requirement

Run tests with:
```bash
cargo test --release --features cuda -p sagitta-mcp
```

## Tool Description Updates

To ensure AI assistants understand the new requirements, tool descriptions have been updated with:

1. **read_file**: Clear examples showing start_line and end_line usage, explicit warning against using old parameters
2. **shell_execute**: Examples for each filter type with common use cases
3. **search_file**: Clarification about recursive search behavior by default

These descriptions help prevent confusion when AI models try to use outdated parameter names or don't understand the requirements.