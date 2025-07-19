# GUI-CLI Tool Execution Synchronization Plan

## Problem Statement
GUI has infinite loops with "Do an ls -l and summarize what you see" while CLI works correctly.
Root cause: GUI and CLI use different tool execution flows.

## Analysis Summary

### CLI (Working Correctly)
- File: `/crates/sagitta-code/src/bin/test_openai_streaming.rs`
- Tool execution flow:
  1. Tool call detected in stream
  2. Execute via MCP (`execute_mcp_tool`)
  3. **Add tool result to conversation history as Function role message**
  4. Continue conversation loop
- Result: Clean execution, no loops

### GUI (Broken - Infinite Loops)
- File: `/crates/sagitta-code/src/gui/app/events.rs`
- **Two separate tool execution paths**:

#### Path 1: AgentEvent::ToolCallComplete (lines 280-405)
- Executes tools via MCP
- Tries to add results to agent history
- May work but complex async handling

#### Path 2: AppEvent::ToolExecutionComplete (lines 737-849) ⚠️ **PROBLEM**
- Does **NOT** add tool results to conversation history
- Triggers continuation with empty message
- Agent doesn't see tool results → repeats same tool call → infinite loop

## Root Cause
The `AppEvent::ToolExecutionComplete` handler triggers continuation without adding tool results to agent's conversation history. Agent thinks it never received tool results, so repeats the same tool call infinitely.

## Fix Strategy

### Phase 1: Immediate Fix
1. **Update AppEvent::ToolExecutionComplete handler** to add tool results to agent history before continuation
2. **Use same execute_mcp_tool logic** as CLI for consistency
3. **Test both GUI and CLI** with same problematic command

### Phase 2: Code Unification  
1. **Extract shared tool execution logic** into common module
2. **Make GUI and CLI use identical code paths**
3. **Eliminate duplicate tool execution implementations**

### Phase 3: Comprehensive Testing
1. **Test matrix** covering multiple scenarios:
   - Simple tool calls (ping)
   - Shell commands (ls -l)
   - Tool chaining scenarios
   - Error handling
   - Edge cases
2. **Verify identical behavior** between GUI and CLI

## Implementation Plan

### Step 1: Fix GUI AppEvent::ToolExecutionComplete Handler
Location: `/crates/sagitta-code/src/gui/app/events.rs` lines ~737-849

**Current problematic code:**
```rust
AppEvent::ToolExecutionComplete { tool_call_id, result } => {
    // ... remove from tracking ...
    // ⚠️ MISSING: Add tool result to agent conversation history
    let agent_clone = app.agent.clone();
    let future = async move {
        agent_clone.process_message_stream("").await  // ⚠️ No tool results in history!
    };
    // ... spawn task ...
}
```

**Required fix:**
```rust
AppEvent::ToolExecutionComplete { tool_call_id, result } => {
    // ... remove from tracking ...
    
    // 🔧 FIX: Add tool result to agent conversation history BEFORE continuation
    if let Some((tool_name, tool_result)) = app.state.completed_tool_results.get(&tool_call_id) {
        let result_json = /* convert tool_result to JSON */;
        let agent_clone = app.agent.clone();
        let tool_call_id_clone = tool_call_id.clone();
        let tool_name_clone = tool_name.clone();
        
        let future = async move {
            // Add tool result to history FIRST
            if let Err(e) = agent_clone.add_tool_result_to_history(&tool_call_id_clone, &tool_name_clone, &result_json).await {
                log::error!("Failed to add tool result to history: {}", e);
            }
            
            // THEN continue conversation
            agent_clone.process_message_stream("").await
        };
        // ... spawn task ...
    }
}
```

### Step 2: Verify execute_mcp_tool Logic
- Ensure GUI uses same MCP execution path as CLI
- Check tool result format consistency
- Verify error handling matches

### Step 3: Testing Protocol

#### Test Commands
1. **Simple tool**: "Please ping the server"
2. **Shell command**: "Do an ls -l and summarize what you see"  
3. **Tool chaining**: "List repositories, then search for 'config' in the first one"
4. **Error case**: "Run a command that doesn't exist"
5. **Complex chain**: "List files, then search for main function in any .rs files"

#### Success Criteria
- ✅ No infinite loops in GUI
- ✅ Identical behavior between GUI and CLI
- ✅ Tool results visible in agent conversation history
- ✅ Model can reference tool results in responses
- ✅ Clean completion after tool execution

### Step 4: Code Extraction (Future)
Extract common tool execution logic into:
- `crates/sagitta-code/src/tools/execution_common.rs`
- Shared between GUI and CLI
- Single source of truth for tool execution flow

## Testing Matrix

| Test Case | CLI Status | GUI Status | Expected |
|-----------|------------|------------|----------|
| Simple ping | ✅ Works | ❌ Unknown | ✅ Should work |
| ls -l command | ✅ Works | ❌ Infinite loop | ✅ Should work |
| Tool chaining | ✅ Works | ❌ Unknown | ✅ Should work |
| Error handling | ❌ Untested | ❌ Unknown | ✅ Should work |
| Complex scenarios | ❌ Untested | ❌ Unknown | ✅ Should work |

## Risk Mitigation
1. **Test thoroughly** after each change
2. **Keep backup** of working CLI implementation
3. **Incremental fixes** - don't change everything at once
4. **Log extensively** during testing for debugging

## Success Metrics
1. **Zero infinite loops** in GUI tool execution
2. **Identical logs** between GUI and CLI for same commands
3. **100% test pass rate** for all tool execution scenarios
4. **Sub-5 second** response time for simple tool calls
5. **Proper error handling** for failed tool calls

---

*Last updated: 2025-07-19*
*Status: Investigation complete, ready for implementation*