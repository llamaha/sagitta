# Double Continuation Fix Summary

## Problem
The GUI was experiencing infinite loops when executing "Do an ls -l and summarize what you see" with OpenAI-compatible providers.

## Root Cause
The GUI had two event handlers that both triggered continuation after tool execution:

1. **AgentEvent::ToolCallComplete** - Emitted by the streaming processor when it receives a `MessagePart::ToolResult`
2. **AppEvent::ToolExecutionComplete** - Emitted by the MCP tool executor after executing the tool

Both handlers were:
- Adding tool results to conversation history
- Triggering `process_message_stream()` to continue the conversation
- This caused the model to process the same tool results twice, leading to infinite loops

## Solution
Added tracking to prevent double continuation:

1. **Added tracking field** to `AppState`:
   ```rust
   pub tool_calls_continued: HashMap<String, bool>, // Maps tool_call_id to whether continuation has been triggered
   ```

2. **Modified both event handlers** to:
   - Check if continuation has already been triggered for a tool call
   - Mark tool calls as continued when triggering continuation
   - Prevent double handling of the same tool results

3. **Clear tracking on new messages** in `handle_chat_input_submission()`:
   ```rust
   app.state.tool_calls_continued.clear();
   app.state.completed_tool_results.clear();
   ```

## Key Changes

### In `/home/adam/repos/sagitta/crates/sagitta-code/src/gui/app/state.rs`:
- Added `tool_calls_continued` field to track which tool calls have triggered continuation

### In `/home/adam/repos/sagitta/crates/sagitta-code/src/gui/app/events.rs`:
- Both `AgentEvent::ToolCallComplete` and `AppEvent::ToolExecutionComplete` handlers now:
  - Check `already_continued` before triggering continuation
  - Mark all tool calls as continued when triggering

### In `/home/adam/repos/sagitta/crates/sagitta-code/src/gui/app/rendering.rs`:
- Clear tracking maps when starting a new message to prevent stale data

## Testing
The fix prevents the GUI from entering infinite loops by ensuring that continuation only happens once per set of tool executions, regardless of which event handler processes the results first.