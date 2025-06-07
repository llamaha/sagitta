# Sagitta Code Comprehensive Testing Plan

## Overview
This plan systematically tests coding scenarios across all supported languages to ensure robust functionality and identify issues before they impact users. Each test scenario will be marked as completed when successfully validated.

## Supported Languages
Based on `src/syntax/languages.rs`:
- **Rust** (`.rs`)
- **Markdown** (`.md`) 
- **Go** (`.go`)
- **JavaScript** (`.js`, `.jsx`)
- **TypeScript** (`.ts`, `.tsx`)
- **YAML** (`.yaml`, `.yml`)
- **Ruby** (`.rb`)
- **Python** (`.py`)
- **HTML** (`.html`)
- **Fallback** (unsupported extensions)

## Testing Categories

### 1. Basic Language Support Tests

#### 1.1 File Creation and Basic Syntax
- [ ] **Rust**: Create a simple "Hello World" program with proper syntax
- [ ] **Python**: Create a basic script with functions and classes
- [ ] **JavaScript**: Create a simple Node.js script with ES6 features
- [ ] **TypeScript**: Create a typed script with interfaces and classes
- [ ] **Go**: Create a basic Go program with packages and functions
- [ ] **Ruby**: Create a simple Ruby script with classes and methods
- [ ] **YAML**: Create configuration files with proper structure
- [ ] **Markdown**: Create documentation with various formatting
- [ ] **HTML**: Create a basic web page with common elements
- [ ] **Fallback**: Test unsupported file types (e.g., `.cpp`, `.java`)

#### 1.2 Language-Specific Tool Requirements
- [ ] **Rust**: Verify `cargo`, `rustc` available in execution environment
- [ ] **Python**: Verify `python3`, `pip` available
- [ ] **JavaScript/Node.js**: Verify `node`, `npm` available
- [ ] **TypeScript**: Verify `tsc`, `ts-node` available
- [ ] **Go**: Verify `go` compiler and tools available
- [ ] **Ruby**: Verify `ruby`, `gem` available
- [ ] **YAML**: Verify YAML validation tools available

### 2. Project Creation and Structure Tests

#### 2.1 New Project Creation
- [ ] **Rust**: Create new Cargo project with `Cargo.toml`
- [ ] **Python**: Create project with `requirements.txt` and proper structure
- [ ] **JavaScript**: Create Node.js project with `package.json`
- [ ] **TypeScript**: Create TypeScript project with `tsconfig.json`
- [ ] **Go**: Create Go module with `go.mod`
- [ ] **Ruby**: Create Ruby project with `Gemfile`

#### 2.2 Multi-File Projects
- [ ] **Rust**: Create multi-crate workspace
- [ ] **Python**: Create package with multiple modules
- [ ] **JavaScript**: Create project with multiple files and imports
- [ ] **TypeScript**: Create project with module system
- [ ] **Go**: Create multi-package project

### 3. Code Editing and Modification Tests

#### 3.1 Syntax-Aware Editing
- [ ] **Rust**: Add/modify functions, structs, implementations
- [ ] **Python**: Add/modify classes, methods, decorators
- [ ] **JavaScript**: Add/modify functions, objects, async/await
- [ ] **TypeScript**: Add/modify with type annotations
- [ ] **Go**: Add/modify with proper error handling
- [ ] **Ruby**: Add/modify classes, modules, mixins

#### 3.2 Complex Refactoring
- [ ] **Rust**: Refactor with ownership and borrowing considerations
- [ ] **Python**: Refactor with proper imports and dependencies
- [ ] **JavaScript**: Refactor with module dependencies
- [ ] **TypeScript**: Refactor maintaining type safety
- [ ] **Go**: Refactor with interface implementations

### 4. Build and Execution Tests

#### 4.1 Compilation/Interpretation
- [ ] **Rust**: `cargo build`, `cargo run`
- [ ] **Python**: Direct execution and module imports
- [ ] **JavaScript**: Node.js execution
- [ ] **TypeScript**: Compilation to JavaScript and execution
- [ ] **Go**: `go build`, `go run`
- [ ] **Ruby**: Direct execution

#### 4.2 Dependency Management
- [ ] **Rust**: Add external crates via `Cargo.toml`
- [ ] **Python**: Install packages via `pip` and `requirements.txt`
- [ ] **JavaScript**: Install packages via `npm`
- [ ] **TypeScript**: Type definitions and package installation
- [ ] **Go**: Add modules via `go get`
- [ ] **Ruby**: Add gems via `Gemfile`

### 5. Testing Framework Integration

#### 5.1 Unit Testing
- [ ] **Rust**: Create and run tests with `cargo test`
- [ ] **Python**: Create tests with `unittest` or `pytest`
- [ ] **JavaScript**: Create tests with Jest or Mocha
- [ ] **TypeScript**: Create typed tests
- [ ] **Go**: Create tests with `testing` package
- [ ] **Ruby**: Create tests with RSpec or Minitest

#### 5.2 Test-Driven Development (TDD)
- [ ] **Rust**: Write failing tests first, then implementation
- [ ] **Python**: TDD workflow with pytest
- [ ] **JavaScript**: TDD with Jest
- [ ] **TypeScript**: TDD with type safety
- [ ] **Go**: TDD with table-driven tests

### 6. Docker and Containerization Tests

#### 6.1 Container Environment
- [ ] **Rust**: Verify Rust toolchain in Docker containers
- [ ] **Python**: Verify Python environment and pip
- [ ] **JavaScript**: Verify Node.js and npm in containers
- [ ] **TypeScript**: Verify TypeScript tools in containers
- [ ] **Go**: Verify Go toolchain in containers
- [ ] **Ruby**: Verify Ruby and gem tools

#### 6.2 Missing Tools Detection
- [ ] Test scenarios where required tools are missing
- [ ] Verify graceful error handling and helpful messages
- [ ] Test tool installation suggestions

### 7. Reasoning Engine Integration Tests

#### 7.1 Intent Analysis Accuracy
- [ ] **Task Completion**: Verify correct detection of completion intent
- [ ] **Tool Selection**: Verify appropriate tool choice for language tasks
- [ ] **Clarification**: Test when engine asks for clarification
- [ ] **Planning**: Verify planning without immediate action detection

#### 7.2 Iteration Control
- [ ] **Normal Flow**: Verify completion within reasonable iterations
- [ ] **Infinite Loop Prevention**: Test max iteration limits
- [ ] **Early Termination**: Test appropriate early stopping
- [ ] **Error Recovery**: Test recovery from failed operations

### 8. Edge Cases and Error Handling

#### 8.1 File System Issues
- [ ] **Permissions**: Test file permission errors
- [ ] **Disk Space**: Test disk space limitations
- [ ] **Path Issues**: Test long paths, special characters
- [ ] **Concurrent Access**: Test file locking issues

#### 8.2 Syntax and Parsing Errors
- [ ] **Invalid Syntax**: Test handling of syntax errors in each language
- [ ] **Malformed Files**: Test corrupted or incomplete files
- [ ] **Encoding Issues**: Test non-UTF8 files
- [ ] **Large Files**: Test performance with large codebases

#### 8.3 Tool Execution Failures
- [ ] **Command Not Found**: Test missing language tools
- [ ] **Timeout**: Test long-running operations
- [ ] **Memory Limits**: Test memory-intensive operations
- [ ] **Network Issues**: Test package downloads failures

### 9. Stream Handling and UI Tests

#### 9.1 Real-time Output
- [ ] **Progress Indication**: Verify streaming progress updates
- [ ] **Tool Execution**: Verify real-time tool output
- [ ] **Error Streaming**: Verify error messages appear promptly
- [ ] **Backpressure**: Test handling of high-volume output

#### 9.2 Interruption Handling
- [ ] **User Cancellation**: Test Ctrl+C handling
- [ ] **Connection Loss**: Test network interruption
- [ ] **Process Termination**: Test graceful shutdown

### 10. Performance and Resource Tests

#### 10.1 Resource Usage
- [ ] **Memory**: Monitor memory usage during large operations
- [ ] **CPU**: Test CPU usage during intensive tasks
- [ ] **I/O**: Test file system performance
- [ ] **Network**: Test download/upload performance

#### 10.2 Scalability
- [ ] **Large Projects**: Test with substantial codebases
- [ ] **Many Files**: Test with projects containing hundreds of files
- [ ] **Deep Nesting**: Test deeply nested directory structures
- [ ] **Concurrent Operations**: Test multiple simultaneous tasks

### 11. Configuration and Environment Tests

#### 11.1 Configuration Validation
- [ ] **Invalid Config**: Test handling of malformed configuration
- [ ] **Missing Config**: Test default configuration fallback
- [ ] **Environment Variables**: Test environment variable override
- [ ] **Path Configuration**: Test custom tool paths

#### 11.2 Multi-Platform Support
- [ ] **Linux**: Comprehensive testing on Linux systems
- [ ] **macOS**: Test macOS-specific behaviors
- [ ] **Windows**: Test Windows compatibility (if supported)
- [ ] **Docker**: Test various Docker base images

## Failure Scenarios to Anticipate

### Critical Failures
1. **Infinite Loops**: Engine never terminates reasoning cycles
2. **Data Loss**: Incorrect file modifications or deletions
3. **System Crashes**: Out of memory, segmentation faults
4. **Security Issues**: Arbitrary command execution, path traversal

### Common Issues
1. **Incorrect Changes**: Code modifications that break functionality
2. **Tool Execution Failures**: Missing dependencies, wrong commands
3. **Container Issues**: Wrong base images, missing tools
4. **Permission Problems**: Insufficient file/directory permissions
5. **Network Failures**: Package installation, git operations
6. **Encoding Problems**: Non-UTF8 files, special characters
7. **Path Issues**: Windows vs Unix paths, spaces in names
8. **Resource Exhaustion**: Memory, disk space, file handles

### Intent Analysis Issues
1. **False Positives**: Detecting completion when task isn't done
2. **False Negatives**: Missing obvious completion signals
3. **Wrong Tool Selection**: Choosing inappropriate tools
4. **Ambiguous Intent**: Unable to determine next action
5. **Context Loss**: Forgetting earlier conversation context

### Streaming and UI Issues
1. **Blocked Streams**: UI freezing during long operations
2. **Lost Output**: Missing tool execution results
3. **Garbled Display**: Encoding issues in output
4. **Delayed Updates**: Poor real-time experience

## Test Execution Protocol

### For Each Test Scenario:
1. **Setup**: Prepare clean environment and test data
2. **Execute**: Run the test using CLI chat binary
3. **Validate**: Verify expected behavior and outputs
4. **Document**: Record any issues, performance notes
5. **Mark Complete**: ✅ when test passes consistently

### Issue Tracking:
- Document all failures with reproduction steps
- Classify by severity (Critical, High, Medium, Low)
- Track fixes and retest accordingly
- Note any performance improvements needed

### Success Criteria:
- All critical scenarios pass without data loss or system issues
- 95% of test scenarios complete successfully
- No infinite loops or resource exhaustion
- Clear error messages for expected failure cases
- Reasonable performance for typical use cases

## Test Environment Requirements

### CLI Chat Binary
- Use `./target/debug/chat_cli` for all testing
- Ensure `GEMINI_API_KEY` is properly configured
- Monitor logs for debugging information

### Infrastructure
- Clean Docker environment for each language test
- Adequate system resources (CPU, memory, disk)
- Network access for package downloads
- Proper file permissions for test directories

### Monitoring
- Track system resource usage during tests
- Log all tool executions and outputs
- Monitor reasoning engine iteration counts
- Record timing information for performance analysis

---

**Status Legend:**
- [ ] Not Started
- ⏳ In Progress  
- ✅ Completed Successfully
- ❌ Failed (needs investigation)
- ⚠️ Partial Success (with notes)

**Next Steps:**
Start with basic language support tests (Section 1) and work through categories systematically, documenting all findings and fixes along the way.

# Coding Test Plan for Sagitta Code Reasoning Engine

This document outlines test scenarios to validate the reasoning engine's ability to handle various coding tasks across different languages and complexity levels.

## Test Status
- **Completed**: 1/30 tests ✅
- **Issues Found & Fixed**: 3 major infrastructure issues
- **Last Updated**: 2025-01-05

---

## Issues Discovered and Resolved

### 🐛 **Issue #1: Project Creation Path Resolution**
- **Problem**: ProjectCreationTool was using shell tool's current working directory instead of proper repository base path
- **Impact**: Projects were created in `/home/adam/repos/sagitta-search` instead of XDG data directory
- **Root Cause**: Tool fell back to `default_working_dir` when `repositories_base_path` was None
- **Fix**: Updated tool to use `get_repo_base_path()` function for proper XDG path resolution
- **Files Modified**: `crates/sagitta-code/src/tools/project_creation.rs`
- **Status**: ✅ **RESOLVED** - Projects now correctly create in `~/.local/share/sagitta/repositories/`

### 🐛 **Issue #2: Docker Container Hanging and Privileged Mode Errors**
- **Problem**: System hung indefinitely during shell execution, requiring multiple Ctrl+C to exit
- **Symptoms**: 
  - Docker errors: `mount: /sys/kernel/security: permission denied`
  - `mkdir: cannot create directory '/sys/fs/cgroup/init': Read-only file system`
  - Docker-in-Docker failures with `megabytelabs/devcontainer:latest`
- **Root Cause**: Container tried to run privileged operations that failed in containerized environment
- **Impact**: Made the tool unusable - tests never completed
- **Fix**: Replaced with lightweight Alpine Linux containers for all supported languages
- **Files Modified**: `crates/sagitta-code/src/tools/shell_execution.rs`
- **Status**: ✅ **RESOLVED** - No more hanging, fast execution, secure isolation

### 🐛 **Issue #3: Intent Analyzer Infinite Loop Detection**
- **Problem**: Intent analyzer failed to detect critical failures and request human intervention
- **Symptoms**: System kept retrying failed operations instead of stopping
- **Impact**: Wasted resources and poor user experience
- **Fix**: Enhanced critical failure detection patterns for permission/filesystem errors
- **Files Modified**: `crates/sagitta-code/src/reasoning/intent_analyzer.rs`
- **Status**: ✅ **IMPROVED** - Better failure detection (needs further testing)

---

## Infrastructure Improvements Implemented

### 🏗️ **Alpine Container Implementation**
Successfully implemented language-specific Alpine containers:
- **Rust**: `rust:1.75-alpine` (1GB RAM, 2 CPU, 10min timeout for compilation)
- **Python**: `python:3.12-alpine` (512MB RAM, 1 CPU, 5min timeout)
- **JavaScript/TypeScript**: `node:20-alpine` (512MB RAM, 1 CPU, 5min timeout)  
- **Go**: `golang:1.21-alpine` (512MB RAM, 1 CPU, 5min timeout)
- **Ruby**: `ruby:3.2-alpine` (512MB RAM, 1 CPU, 5min timeout)
- **HTML/YAML/Markdown**: `alpine:3.19` (128MB RAM, 0.5 CPU, 1min timeout)
- **Default**: `alpine:3.19` (256MB RAM, 1 CPU, 5min timeout)

**Benefits**:
- ✅ No privileged mode required
- ✅ Network isolation (`--network none`) for security
- ✅ Proper resource limits
- ✅ Fast startup times
- ✅ Consistent execution environment

---

## 1. Basic Language Support Tests

### ✅ **COMPLETED** - Rust
- **Test**: Create a simple "Hello World" program with proper syntax
- **Expected**: Valid Rust code with `main()` function, proper imports, CLI structure
- **Status**: ✅ **PASSED** - Created proper Rust CLI project with clap integration
- **Location**: `~/.local/share/sagitta/repositories/hello_world_rust/`
- **Result**: 
  - ✅ Generated syntactically correct code with proper project structure
  - ✅ Created complete Cargo.toml with dependencies (clap, anyhow)
  - ✅ Added .gitignore file
  - ✅ Compiled successfully with `cargo build`
  - ✅ Executed successfully: outputs "Hello from hello_world!" 
- **Infrastructure**: ✅ Alpine container working perfectly - no hanging or Docker errors
- **Performance**: ✅ Fast execution (~800ms tool calls instead of timeouts)

### ⏳ **TODO** - Python
- **Test**: Create a basic script with functions and classes
- **Expected**: Valid Python code with proper structure and imports
- **Status**: **READY TO TEST** - Alpine container configured

### ⏳ **TODO** - JavaScript  
- **Test**: Create a simple Node.js script with ES6 features
- **Expected**: Valid JS code with modern syntax
- **Status**: **READY TO TEST** - Alpine container configured

### ⏳ **TODO** - TypeScript
- **Test**: Create a typed script with interfaces and classes  
- **Expected**: Valid TS code with proper type annotations
- **Status**: **READY TO TEST** - Alpine container configured

### ⏳ **TODO** - Go
- **Test**: Create a basic Go program with packages and functions
- **Expected**: Valid Go code with proper module structure
- **Status**: **READY TO TEST** - Alpine container configured

### ⏳ **TODO** - Ruby
- **Test**: Create a simple Ruby script with classes and methods
- **Expected**: Valid Ruby code with proper structure
- **Status**: **READY TO TEST** - Alpine container configured

### ⏳ **TODO** - YAML
- **Test**: Create configuration files with proper structure
- **Expected**: Valid YAML with nested structures
- **Status**: **READY TO TEST** - Alpine container configured

### ⏳ **TODO** - Markdown
- **Test**: Create documentation with various formatting
- **Expected**: Valid Markdown with headers, lists, code blocks
- **Status**: **READY TO TEST** - Alpine container configured

### ⏳ **TODO** - HTML
- **Test**: Create a basic web page with common elements
- **Expected**: Valid HTML5 with proper structure
- **Status**: **READY TO TEST** - Alpine container configured

---

## Next Priority Issues to Address

### 🔍 **Issue #4: POTENTIAL - Tool Definition Clarity**
- **Observation**: LLM sometimes uses incorrect paths in tool calls
- **Need to investigate**: Whether tool descriptions need improvement
- **Files to check**: Tool definition descriptions in all tool files

### 🔍 **Issue #5: POTENTIAL - Chat Session Management**  
- **Observation**: Chat session doesn't clearly indicate completion
- **Need to investigate**: Whether conversation termination is properly handled
- **Files to check**: Chat CLI main loop and conversation management

### 🔍 **Issue #6: POTENTIAL - Error Message Quality**
- **Observation**: Need to verify error messages are helpful across all scenarios
- **Need to investigate**: Error handling in all tools
- **Files to check**: Tool execution error paths

---

## Testing Protocol

### Current Testing Command:
```bash
echo "Create a simple [LANGUAGE] program..." | timeout 120 ./target/debug/chat_cli
```

### Validation Steps:
1. ✅ Verify project created in correct location (`~/.local/share/sagitta/repositories/`)
2. ✅ Check project structure (proper files, dependencies, configs)
3. ✅ Validate syntax correctness 
4. ✅ Test compilation/execution
5. ✅ Verify no hanging or Docker errors
6. **NEW** ⭐ Commit successful changes to git
7. **NEW** ⭐ Sync repository using repo sync tool (if applicable)

### Post-Test Actions (REQUIRED):
After each successful test completion:

1. **Git Commit**: 
   ```bash
   git add .
   git commit -m "Complete [LANGUAGE] coding test - [brief description of what was created]"
   ```

2. **Repository Sync**: 
   Use the vectordb-mcp tool to sync any created repositories:
   ```bash
   # If working with external repositories, sync them to keep indexes updated
   # This ensures the reasoning engine has access to the latest created projects
   ```

### Success Criteria:
- Project creates in under 2 minutes
- No Docker permission errors
- Syntactically correct code
- Proper project structure
- Code compiles and runs successfully
- **Changes committed to git successfully**
- **Repository indexes updated (if applicable)**

---

**Status Legend:**
- ✅ **COMPLETED** - Test passed successfully
- ⏳ **TODO** - Ready to test with fixed infrastructure  
- 🔍 **INVESTIGATING** - Potential issue needs analysis
- 🐛 **ISSUE** - Problem identified, needs fix
- ❌ **FAILED** - Test failed, requires attention

**Infrastructure Status**: ✅ **STABLE** - All major blocking issues resolved 