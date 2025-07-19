# Streaming Tool Tracking Fix Summary

## Problem
The GUI was experiencing infinite loops because the model would re-execute the same tool (`ls -l`) instead of analyzing the results.

## Root Causes

### 1. Double Continuation (Fixed Earlier)
Both `AgentEvent::ToolCallComplete` and `AppEvent::ToolExecutionComplete` were triggering continuation, causing duplicate processing.

### 2. Missing Tool Call Tracking in Assistant Messages (Fixed Now)
When the streaming processor received a `MessagePart::ToolCall`, it would:
- Emit an event to trigger tool execution
- But **NOT** add the tool call to the assistant message in history

This caused:
- `add_tool_result_to_history` couldn't find the tool call to update
- The conversation history showed Function messages with empty content
- The model couldn't see the tool results properly and would re-run the same tool

## Solution

### 1. Fixed Double Continuation
- Added `tool_calls_continued` tracking to prevent double handling
- Both event handlers now check if continuation was already triggered

### 2. Fixed Tool Call Tracking
In `streaming.rs`, when receiving a `MessagePart::ToolCall`:
```rust
// CRITICAL: Add tool call to the assistant message in history
if let Some(msg) = history_manager.get_message(message_id).await {
    let mut updated = msg.clone();
    updated.tool_calls.push(tool_call.clone());
    
    let _ = history_manager.remove_message(message_id).await;
    let _ = history_manager.add_message(updated).await;
    info!("Stream: Added tool call to assistant message for {name}");
}
```

### 3. Fixed Tool Result Updates
Modified `add_tool_result_to_history` to properly find and update tool calls in assistant messages rather than creating new Function messages with empty content.

## Why Streaming is Hard

1. **Asynchronous Nature**: Events arrive out of order, tool execution happens separately from streaming
2. **State Management**: Must track which messages have which tool calls across async boundaries
3. **Multiple Protocols**: Different providers (Claude Code vs OpenAI-compatible) handle tools differently
4. **Event-Driven Architecture**: GUI's complex event system can trigger duplicate handling
5. **Message History**: Must maintain proper conversation history for the model to understand context

The fix ensures that:
- Tool calls are properly tracked in assistant messages
- Tool results update the existing tool calls
- The model sees a complete conversation history with proper tool results
- No duplicate continuation happens