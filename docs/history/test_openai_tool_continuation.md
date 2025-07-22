# Testing OpenAI Tool Continuation

This document explains how to test the OpenAI-style tool continuation feature that has been implemented.

## Background

OpenAI-compatible APIs follow a specific pattern for tool execution:
1. User sends a message
2. LLM responds with tool calls and `finish_reason: "tool_calls"`
3. Stream ends (this is where the issue was - it used to just stop)
4. Client executes tools and sends results back
5. LLM provides final response

## What Was Fixed

1. **Streaming Processor** (`agent/streaming.rs`):
   - Now properly checks `finish_reason` in stream chunks
   - When `finish_reason` is "stop", it emits `ConversationCompleted` event
   - When `finish_reason` is "tool_calls", it sets state to "Waiting for tool execution"

2. **Tool Completion Handler** (`gui/app/events.rs`):
   - Tracks active tool calls
   - When all tools complete AND we're using an OpenAI-compatible provider
   - Automatically sends an empty message to trigger continuation
   - Creates a new streaming session for the final response

## How to Test

1. **Configure an OpenAI-compatible provider** (e.g., OpenRouter, local Mistral.rs server)

2. **Send a message that requires tool use**, for example:
   ```
   Search for the main function in this codebase and tell me what it does
   ```

3. **Expected behavior**:
   - LLM will request tool execution (e.g., semantic_code_search)
   - Tool will execute and show results in the UI
   - **Automatically** (without user input), the LLM will continue and provide a final response
   - The conversation will be marked as complete

4. **What to look for in logs**:
   ```
   Stream: Response complete with tool calls - conversation will continue
   Tool call semantic_code_search completed
   All tools complete for OpenAI-compatible provider, triggering continuation
   Starting continuation stream after tool completion
   Stream: Conversation completed (finish_reason: stop)
   ```

## Comparison with Previous Behavior

**Before**: After tool execution, the conversation would stop. User had to send another message (even empty) to get the final response.

**After**: Tool execution automatically triggers continuation, and the LLM provides its final response without user intervention.

## Notes

- This behavior only applies to OpenAI-compatible providers
- Claude's native API handles tool execution differently (all in one stream)
- The continuation uses an empty message to trigger the agent to process tool results