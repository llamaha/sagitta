# GUI Infinite Loop Diagnosis

## Problem
GUI still has infinite loops with "Do an ls -l and summarize what you see" while CLI works correctly.

## Event Flow Analysis

### Current Flow (PROBLEMATIC):

1. **User sends message** → Agent processes → Stream contains tool call
2. **StreamingProcessor emits** `AgentEvent::ToolCall` 
3. **GUI handles** `AgentEvent::ToolCall`:
   - Executes tool via MCP
   - Sends `AppEvent::ToolExecutionComplete` (line 270)
4. **GUI handles** `AppEvent::ToolExecutionComplete`:
   - Stores tool result in `completed_tool_results`
   - Adds tool results to agent history ✅ (FIXED)
   - Calls `process_message_stream("")` for continuation
5. **ALSO: StreamingProcessor might emit** `AgentEvent::ToolCallComplete`
6. **GUI handles** `AgentEvent::ToolCallComplete`:
   - ALSO stores tool result in `completed_tool_results`
   - ALSO adds tool results to agent history
   - ALSO calls `process_message_stream("")` for continuation

## Potential Issues

### 1. Double Continuation
- Both `AppEvent::ToolExecutionComplete` AND `AgentEvent::ToolCallComplete` trigger continuation
- This could cause two simultaneous empty message streams
- Model might get confused with multiple continuation requests

### 2. State Conflicts
- `completed_tool_results` is cleared by first handler (line 786)
- Second handler might not have results to add
- Race condition between handlers

### 3. Wrong Event Path
- GUI uses `AgentEvent::ToolCall` → `AppEvent::ToolExecutionComplete` path
- But StreamingProcessor might expect `AgentEvent::ToolCallComplete` path
- CLI doesn't use these event handlers at all - it directly adds results inline

## CLI vs GUI Difference

### CLI (Working):
```rust
// Direct execution in main loop
1. Detect tool call in stream
2. Execute tool via MCP
3. Add result to messages array as Function role
4. Continue loop (automatic continuation)
```

### GUI (Broken):
```rust
// Complex event-driven flow
1. Detect tool call in stream
2. Emit AgentEvent::ToolCall
3. Handle in GUI → execute → emit AppEvent::ToolExecutionComplete
4. Handle completion → add to history → call process_message_stream("")
5. Possibly ALSO handle AgentEvent::ToolCallComplete → double continuation?
```

## Hypothesis
The GUI's event-driven architecture is causing either:
1. Double continuation (both event handlers fire)
2. Missing tool results (race condition)
3. Incorrect state management between handlers

## Next Steps
1. Check if `AgentEvent::ToolCallComplete` is being emitted by StreamingProcessor
2. Determine which event path should be the canonical one
3. Disable one of the continuation paths
4. Test if single path fixes the issue