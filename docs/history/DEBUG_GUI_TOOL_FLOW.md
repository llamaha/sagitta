# Debug Plan for GUI Tool Flow

## Issue
GUI gets stuck in infinite loop when running "Do an ls -l and summarize what you see"

## Current Understanding
1. CLI works perfectly - adds tool results to history and continues
2. GUI has complex event flow that might be causing issues
3. Tool results ARE being added to history (we fixed that)
4. But something else is causing the loop

## Debugging Steps

### 1. Add Comprehensive Logging
Need to log at these critical points:
- When tool execution starts
- When tool results are added to history
- What messages are in history before continuation
- When continuation stream starts
- What the model receives in the continuation request

### 2. Check State Flags
- `is_waiting_for_response` state during tool execution
- `active_tool_calls` tracking
- `completed_tool_results` management

### 3. Potential Issues to Investigate

#### A. Empty Continuation Message
When GUI calls `process_message_stream("")`, it sends an empty message. This might cause the model to:
- Not see the conversation context properly
- Think it needs to start over
- Re-execute the same tool

#### B. Message History Corruption
- Are tool results actually in the agent's conversation history?
- Is the history being cleared or reset somewhere?
- Are messages in the correct order?

#### C. Provider Detection Issue
- Is the OpenAI-compatible provider check working correctly?
- Could it be using the wrong streaming logic?

### 4. Quick Test
Add a non-empty continuation message like "Please continue with the analysis" instead of empty string to see if that helps the model understand it should continue.

## Temporary Workaround Ideas
1. Add a delay before continuation to ensure state is settled
2. Send a more explicit continuation prompt
3. Check if tool results are actually visible to the model
4. Log the entire conversation history at continuation time