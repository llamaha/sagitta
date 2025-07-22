# Tool Result Fix Plan for OpenAI-Compatible Providers

## Problem Analysis
The model is stuck in infinite loops because tool results are not being sent back to the LLM. From the chat log, the model:
1. Thinks about executing `ls -l` 
2. Never actually calls the tool (or the result doesn't make it back)
3. When asked about files, says it hasn't run the command yet
4. Repeats the same thinking pattern

## Current Architecture Issues
1. **Test CLI vs GUI Disconnect**: The test CLI uses a different code path than the GUI
2. **Tool Result Flow Missing**: Tool results aren't being added to conversation history properly
3. **Event System Not Used**: Test CLI doesn't use the same agent event system as GUI

## Plan: Make Test CLI Use Same Backend as GUI

### Phase 1: Understand Current vs Desired Flow

**Current Test CLI Flow:**
```
User Input → Agent::process_message_stream → LLM Stream → Tool calls detected → ???
```

**Current GUI Flow:**
```
User Input → Agent::process_message_stream → LLM Stream → Tool calls → AgentEvent → GUI event handler → MCP execution → Tool result in UI → Continuation with history
```

**Desired Test CLI Flow:**
```
User Input → Agent::process_message_stream → LLM Stream → Tool calls → AgentEvent → CLI event handler → MCP execution → Tool result added to history → Continuation
```

### Phase 2: Implementation Steps

#### Step 1: Add Agent Event Handling to Test CLI
- Create an event handler similar to GUI's `handle_agent_event`
- Listen for `AgentEvent::ToolCall` and `AgentEvent::ToolCallComplete`
- Execute tools via MCP like the GUI does

#### Step 2: Implement Tool Execution Flow
- When `ToolCall` event received, execute via MCP
- Add tool result to agent's conversation history using `Agent::add_tool_result_to_history`
- Trigger continuation with `agent.process_message_stream("")`

#### Step 3: Add Comprehensive Logging
- Log every step of tool execution and result handling
- Log conversation history before/after tool execution
- Log what gets sent to LLM in continuation

#### Step 4: Test and Iterate
- Test with simple tools like `ping` and `shell_execute`
- Verify tool results appear in conversation history
- Verify model can see and use tool results in responses

### Phase 3: Technical Implementation Details

#### New Test CLI Architecture
```rust
struct TestCLI {
    agent: Arc<Agent>,
    event_receiver: broadcast::Receiver<AgentEvent>,
    active_tool_calls: HashMap<String, String>, // tool_call_id -> message_id
}

impl TestCLI {
    async fn handle_conversation() {
        // Start agent processing
        let stream = agent.process_message_stream(input).await?;
        
        // Handle events in parallel
        tokio::select! {
            // Process stream chunks
            chunk = stream.next() => { /* handle chunk */ }
            
            // Handle agent events
            event = event_receiver.recv() => {
                match event {
                    AgentEvent::ToolCall { tool_call, message_id } => {
                        self.execute_tool(tool_call, message_id).await;
                    }
                    AgentEvent::ToolCallComplete { .. } => {
                        self.check_and_continue().await;
                    }
                }
            }
        }
    }
    
    async fn execute_tool(&mut self, tool_call: ToolCall, message_id: String) {
        // Store active tool call
        self.active_tool_calls.insert(tool_call.id.clone(), message_id);
        
        // Execute via MCP (same as GUI)
        let result = execute_mcp_tool(&tool_call.name, tool_call.arguments).await;
        
        // Add result to agent history
        let result_json = /* convert result */;
        self.agent.add_tool_result_to_history(&tool_call.id, &tool_call.name, &result_json).await;
        
        // Mark tool complete
        self.active_tool_calls.remove(&tool_call.id);
        
        // Continue if all tools done
        if self.active_tool_calls.is_empty() {
            self.continue_conversation().await;
        }
    }
    
    async fn continue_conversation(&self) {
        println!("🔄 Continuing conversation with tool results...");
        let stream = self.agent.process_message_stream("").await?;
        // Process continuation stream...
    }
}
```

#### Key Functions to Reuse from GUI
- `execute_mcp_tool()` function from GUI events.rs
- Tool result conversion logic
- History management approach

### Phase 4: Testing Strategy

#### Test Cases
1. **Simple Tool Call**: `ping` tool
2. **Shell Command**: `shell_execute` with `ls -l`
3. **Multi-tool Sequence**: List repos, then search in first repo
4. **Error Handling**: Tool that fails

#### Success Criteria
- Model executes tool calls without loops
- Model can reference tool results in follow-up responses
- Tool results visible in conversation history
- No infinite repetition of same tool calls

### Phase 5: Debugging Tools

#### Logging Points
- Before/after adding tool results to history
- Conversation history contents before continuation
- LLM messages sent during continuation
- Tool execution success/failure

#### Debug Commands
- Print conversation history at any point
- Show active tool calls
- Dump agent state

## Implementation Priority
1. **HIGH**: Get basic tool execution working in test CLI
2. **HIGH**: Verify tool results added to conversation history  
3. **HIGH**: Verify continuation includes tool results
4. **MEDIUM**: Add comprehensive logging
5. **LOW**: Error handling and edge cases

## Compile Flags
Remember to use: `cargo build --release --features cuda,openai-stream-cli`

## Exit Criteria
✅ Model can execute tools and reference their results
✅ No infinite loops or repeated tool calls  
✅ Test CLI behavior matches GUI behavior
✅ Comprehensive logging for debugging