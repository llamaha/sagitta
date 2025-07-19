# Test Plan for OpenAI-Compatible Tool Continuation

## Problem Statement
When using OpenAI-compatible providers (like OpenRouter), after tools are executed, the conversation stops instead of automatically continuing with the LLM processing the tool results.

## Solution Implemented
Added logic in the `ToolCallComplete` event handler to:
1. Check if all tools are complete (`active_tool_calls.is_empty()`)
2. Check if we're using an OpenAI-compatible provider
3. If both conditions are met, automatically trigger a new LLM call to continue the conversation

## Testing Steps

1. **Configure OpenAI-Compatible Provider**
   - Set up OpenRouter or another OpenAI-compatible provider in settings
   - Ensure proper API key is configured

2. **Test Tool Execution Flow**
   - Send a message that requires tool use (e.g., "What files are in the current directory?")
   - Observe that:
     a. The LLM requests a tool call
     b. The tool executes and returns results
     c. **NEW**: The conversation automatically continues without user input
     d. The LLM processes the tool results and provides a final response

3. **Test Multiple Tools**
   - Send a message requiring multiple tools (e.g., "Read the README.md file and tell me what this project is about")
   - Verify that all tools complete before continuation is triggered

4. **Test Non-OpenAI Providers**
   - Switch to Claude or another non-OpenAI provider
   - Verify that the behavior doesn't change for these providers

## Expected Behavior

### Before Fix
1. User asks question requiring tools
2. LLM requests tool execution
3. Tools execute and return results
4. **Conversation stops - user must send another message**

### After Fix
1. User asks question requiring tools
2. LLM requests tool execution
3. Tools execute and return results
4. **Conversation automatically continues**
5. LLM processes tool results and provides final answer

## Code Changes
- Modified `crates/sagitta-code/src/gui/app/events.rs`
- Added check in `ToolCallComplete` event handler
- When all tools complete for OpenAI-compatible provider, triggers continuation stream