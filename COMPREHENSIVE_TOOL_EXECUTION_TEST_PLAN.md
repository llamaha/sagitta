# Comprehensive Tool Execution Test Plan

## Overview
This document provides a resilient testing framework for verifying tool execution works identically between GUI and CLI for OpenAI-compatible providers.

## Test Categories

### 1. Basic Tool Execution
**Objective**: Verify simple tool calls work without loops

#### Test 1.1: Server Connectivity
```bash
RUST_LOG=info ./target/release/test-openai-streaming -p "Please ping the server and tell me if it responds" -g
```
**Expected**: 
- ✅ Tool call: `ping`
- ✅ Tool executes successfully
- ✅ Model references result in response
- ✅ No infinite loops
- ✅ Total messages: 5 (System, User, Assistant+ToolCall, Function+Result, Assistant+Final)

#### Test 1.2: Shell Command Execution
```bash
RUST_LOG=info ./target/release/test-openai-streaming -p "Do an ls -l and summarize what you see" -g
```
**Expected**:
- ✅ Tool call: `shell_execute` with `{"command": "ls -l"}`
- ✅ Tool executes successfully 
- ✅ Model analyzes file listing and provides summary
- ✅ No infinite loops
- ✅ Total messages: 5

#### Test 1.3: Repository Listing
```bash
RUST_LOG=info ./target/release/test-openai-streaming -p "List all available repositories" -g
```
**Expected**:
- ✅ Tool call: `repository_list`
- ✅ Model lists repositories with details
- ✅ No loops

### 2. Tool Chaining
**Objective**: Verify tool A results can be used by tool B

#### Test 2.1: Repository → Search Chain
```bash
RUST_LOG=info ./target/release/test-openai-streaming -p "List all repositories, then search for 'save game' in the first repository if any exist" -g
```
**Expected**:
- ✅ Tool call 1: `repository_list`
- ✅ Tool call 2: `semantic_code_search` with repository from first result
- ✅ Model shows results from both tools
- ✅ Total messages: 7 (System, User, Assistant+Tool1, Function+Result1, Assistant+Tool2, Function+Result2, Assistant+Final)

#### Test 2.2: Directory → Analysis Chain
```bash
RUST_LOG=info ./target/release/test-openai-streaming -p "List files in current directory, then search for any Rust files mentioned" -g
```
**Expected**:
- ✅ Tool call 1: `shell_execute` with `ls` command
- ✅ Tool call 2: `semantic_code_search` for Rust files found
- ✅ Intelligent chaining of results

#### Test 2.3: Complex Multi-Tool Chain
```bash
RUST_LOG=info ./target/release/test-openai-streaming -p "Ping the server, list repositories, then search for 'config' in the sagitta repository" -g
```
**Expected**:
- ✅ Three sequential tool calls
- ✅ Each tool result influences next tool call
- ✅ Total messages: 9

### 3. Error Handling
**Objective**: Verify graceful handling of tool failures

#### Test 3.1: Invalid Command
```bash
RUST_LOG=info ./target/release/test-openai-streaming -p "Run the command 'nonexistent_command_xyz'" -g
```
**Expected**:
- ✅ Tool call: `shell_execute` with invalid command
- ✅ Tool returns error result
- ✅ Model acknowledges error and suggests alternatives
- ✅ No infinite loops despite error

#### Test 3.2: Invalid Repository Search
```bash
RUST_LOG=info ./target/release/test-openai-streaming -p "Search for 'test' in repository 'nonexistent_repo'" -g
```
**Expected**:
- ✅ Tool call: `semantic_code_search` with invalid repo
- ✅ Tool returns error
- ✅ Model handles error gracefully

### 4. Interactive Mode Testing
**Objective**: Verify tool execution works in multi-turn conversations

#### Test 4.1: Follow-up Questions
```bash
RUST_LOG=info ./target/release/test-openai-streaming -p "Do an ls -l" -i
# Then input: "What language do these files look like?"
# Then input: "quit"
```
**Expected**:
- ✅ First tool execution completes
- ✅ Follow-up question can reference tool results
- ✅ Model shows understanding of previous tool output

### 5. Edge Cases
**Objective**: Test unusual scenarios

#### Test 5.1: Empty Tool Result
```bash
RUST_LOG=info ./target/release/test-openai-streaming -p "List repositories and check if any are completely empty" -g
```

#### Test 5.2: Large Tool Output
```bash
RUST_LOG=info ./target/release/test-openai-streaming -p "Run 'find /usr -name '*.so' | head -100' and summarize the results" -g
```

#### Test 5.3: Multiple Simultaneous Tool Calls
*(If model attempts to call multiple tools at once)*

### 6. Performance Testing
**Objective**: Verify reasonable response times

#### Test 6.1: Simple Tool Performance
- Measure time from tool call to completion
- **Target**: < 5 seconds for simple tools like `ping`

#### Test 6.2: Complex Tool Performance  
- Measure time for semantic search operations
- **Target**: < 30 seconds for code search

### 7. Logging Verification
**Objective**: Ensure consistent logging between GUI and CLI

#### Critical Log Messages to Verify:
1. `🔧 Tool call detected: [tool_name]`
2. `🚀 Executing tool: [tool_name] with args: [args]`
3. `✅ Tool [tool_name] executed successfully`
4. `📝 Added tool result to conversation history`
5. `=== All tools executed, continuing conversation ===`

#### GUI-Specific Logs to Look For:
1. `Received ToolExecutionComplete event for tool [name]`
2. `Adding tool result to agent history: [id] -> [result]`
3. `Successfully added tool result to conversation history: [id]`
4. `All tools complete for OpenAI-compatible provider, triggering continuation`

### 8. Comparison Testing
**Objective**: Verify GUI and CLI produce identical results

#### Test 8.1: Side-by-Side Comparison
1. Run same command in CLI: `./target/release/test-openai-streaming -p "ping server and list repos" -g`
2. Run same command in GUI with OpenAI-compatible provider
3. Compare:
   - Tool execution sequence
   - Tool results
   - Final model responses
   - Total conversation messages

#### Test 8.2: Log Analysis
1. Save CLI logs: `RUST_LOG=info ./target/release/test-openai-streaming -p "test command" -g > cli_logs.txt 2>&1`
2. Compare with GUI logs for same command
3. Verify identical tool execution flow

## Automated Test Script

### Quick Test Suite
```bash
#!/bin/bash
# Save as test_tool_execution.sh

echo "=== Tool Execution Test Suite ==="

echo "Test 1: Basic ping"
RUST_LOG=info ./target/release/test-openai-streaming -p "Please ping the server" -g
echo ""

echo "Test 2: Shell command"
RUST_LOG=info ./target/release/test-openai-streaming -p "Do an ls -l and summarize" -g
echo ""

echo "Test 3: Tool chaining"
RUST_LOG=info ./target/release/test-openai-streaming -p "List repos, then search first one for config" -g
echo ""

echo "Test 4: Error handling"
RUST_LOG=info ./target/release/test-openai-streaming -p "Run command 'badcommand123'" -g
echo ""

echo "=== All tests complete ==="
```

### Extended Test Suite
```bash
#!/bin/bash
# Save as extended_test_tool_execution.sh

TEST_COMMANDS=(
    "Please ping the server and tell me if it responds"
    "Do an ls -l and summarize what you see"
    "List all repositories, then search for 'config' in the first one"
    "Run 'ps aux | head -5' and explain what processes you see"
    "Find all repositories and ping the server"
    "Search for 'function main' in the sagitta repository"
    "Execute command 'echo hello world' and explain the output"
    "List repositories and count how many there are"
)

for i in "${!TEST_COMMANDS[@]}"; do
    echo "=== Test $((i+1)): ${TEST_COMMANDS[$i]} ==="
    RUST_LOG=info ./target/release/test-openai-streaming -p "${TEST_COMMANDS[$i]}" -g
    echo ""
    echo "Press Enter to continue to next test..."
    read
done
```

## Success Criteria

### Must Pass (Critical)
- ✅ **Zero infinite loops** in both GUI and CLI
- ✅ **Tool results visible** in agent conversation history
- ✅ **Model can reference** tool results in responses
- ✅ **Identical behavior** between GUI and CLI
- ✅ **Clean completion** after tool execution

### Should Pass (Important)
- ✅ **Sub-5 second response** for simple tools
- ✅ **Comprehensive logging** for debugging
- ✅ **Graceful error handling** for failed tools
- ✅ **Multi-tool chaining** works reliably

### Nice to Have (Optional)
- ✅ **Performance metrics** in logs
- ✅ **Tool result formatting** is consistent
- ✅ **Memory usage** remains reasonable

## Debugging Checklist

When tests fail, check:

1. **Tool Result Addition**: 
   - Look for "📝 Added tool result to conversation history" in logs
   - Verify `agent.add_tool_result_to_history()` is called

2. **Conversation History**:
   - Check total message count matches expected
   - Verify Function role messages are present

3. **Event Flow**:
   - Tool call detected → Tool executed → Result added → Continuation triggered

4. **Provider Detection**:
   - Verify using OpenAI-compatible provider, not Claude Code

5. **State Management**:
   - Check `active_tool_calls` and `completed_tool_results` state

## Recovery Procedures

### If Tests Start Failing:
1. **Revert to last known good commit**
2. **Run single test** to isolate issue
3. **Check logs** for missing tool result addition
4. **Verify build** includes latest changes
5. **Test CLI first**, then GUI

### If GUI Still Has Loops:
1. Check `AppEvent::ToolExecutionComplete` handler
2. Verify `add_tool_result_to_history()` is called before continuation
3. Ensure `completed_tool_results` are cleared after use
4. Check provider type detection logic

---

*Last updated: 2025-07-19*
*Status: Ready for comprehensive testing*