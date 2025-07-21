# Tool Card Data Flow Documentation

## Overview

This document explains how tool cards flow through the Sagitta Code system, from execution to persistence and display. Understanding this flow is critical for fixing issues with tool card persistence and copying.

## Data Flow Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌────────────────┐
│   Tool Execute  │───▶│  StreamingChat   │───▶│   Display      │
│                 │    │    Manager       │    │   (UI)         │
└─────────────────┘    └──────────────────┘    └────────────────┘
                              │
                              ▼
                       ┌──────────────────┐    ┌────────────────┐
                       │  Conversation    │───▶│   Persistence  │
                       │    Service       │    │   (Disk/DB)    │
                       └──────────────────┘    └────────────────┘
```

## Core Data Structures

### 1. ChatItem Enum
Located in `crates/sagitta-code/src/gui/chat/mod.rs`

```rust
#[derive(Debug, Clone)]
pub enum ChatItem {
    Message(StreamingMessage),  // Regular chat messages
    ToolCard(ToolCard),        // Tool execution cards
}
```

### 2. ToolCard Struct
```rust
#[derive(Debug, Clone)]
pub struct ToolCard {
    pub run_id: ToolRunId,                    // Unique tool execution ID
    pub tool_name: String,                    // Name of the tool (e.g., "read_file")
    pub status: ToolCardStatus,               // Running, Completed, Failed, Cancelled
    pub progress: Option<f32>,                // Progress percentage (0.0-1.0)
    pub logs: Vec<String>,                    // Execution logs
    pub started_at: DateTime<Utc>,            // Start timestamp
    pub completed_at: Option<DateTime<Utc>>,  // Completion timestamp
    pub input_params: serde_json::Value,      // Tool input parameters
    pub result: Option<serde_json::Value>,    // Tool execution result
}
```

### 3. StreamingChatManager
The central manager that holds all chat items in memory:

```rust
pub struct StreamingChatManager {
    messages: Arc<Mutex<Vec<ChatItem>>>,           // Main messages list
    active_streams: Arc<Mutex<HashMap<String, StreamingMessage>>>, // Currently streaming
    tool_cards: Arc<Mutex<HashMap<ToolRunId, ToolCard>>>,         // Tool card lookup
}
```

## Tool Card Lifecycle

### 1. Tool Execution Start
1. User triggers tool execution
2. `handle_tool_run_started()` called in `events.rs`
3. Tool card created with `Running` status
4. Card added to `StreamingChatManager` via `insert_tool_card_after_last_user_msg()`

### 2. Tool Execution Progress
1. Tool reports progress via MCP/agent events
2. `update_tool_card_progress()` called
3. Progress updated in both `tool_cards` map and `messages` list

### 3. Tool Execution Completion
1. Tool completes with result
2. `complete_tool_card()` called
3. Status updated to `Completed { success: bool }`
4. Result stored in `result` field as `serde_json::Value`

### 4. Display in UI
1. `get_all_items()` returns all `ChatItem`s including tool cards
2. UI renders tool cards with results via `render_tool_card()`
3. Copy function uses same `get_all_items()` data

## Conversation Switching Flow

### The Problem: Tool Cards Disappear

```
Current User Action: Switch to New Conversation
    ↓
switch_to_conversation() called
    ↓
chat_manager.clear_all_messages() ← TOOL CARDS CLEARED FROM MEMORY
    ↓
Async: service.get_conversation(id) loads AgentMessages
    ↓
handle_conversation_messages() converts AgentMessages to StreamingMessages
    ↓ 
chat_manager.add_complete_message(streaming_message) ← ONLY REGULAR MESSAGES
    ↓
RESULT: Tool cards are gone from UI and copy function
```

### Key Issue: AgentMessage ≠ ToolCard

The conversation persistence system stores `AgentMessage` objects, but tool cards are stored as separate `ToolCard` objects. When switching conversations:

1. **Memory Cleared**: `clear_all_messages()` removes all `ChatItem`s including `ToolCard`s
2. **Persistence Loads**: Only `AgentMessage`s are loaded from disk
3. **Conversion**: `AgentMessage` → `StreamingMessage` (no tool cards created)
4. **Result**: Tool cards are lost

## Root Cause Analysis

### Issue 1: Tool Cards Not Persisted
- Tool cards exist only in `StreamingChatManager` memory
- Conversation persistence only saves `AgentMessage` objects
- No mechanism to save/load `ToolCard` objects separately

### Issue 2: Copy Function Uses Same Data
- Copy function calls `get_all_items()` which returns memory-only data
- Since tool cards aren't in memory after conversation switch, they don't get copied
- This explains why copy shows empty "Sagitta Code" entries (messages without tool cards)

## Data Flow Diagram

### Current (Broken) Flow
```
Tool Execution
    ↓
ToolCard created in StreamingChatManager (MEMORY ONLY)
    ↓
User switches conversation
    ↓
StreamingChatManager.clear_all_messages() (TOOL CARDS LOST)
    ↓
AgentMessages loaded from persistence (NO TOOL CARDS)
    ↓
UI shows messages without tool cards
    ↓
Copy function sees no tool cards
```

### Required (Fixed) Flow
```
Tool Execution
    ↓
ToolCard created in StreamingChatManager
    ↓
ToolCard persisted alongside AgentMessage
    ↓
User switches conversation
    ↓
StreamingChatManager.clear_all_messages()
    ↓
BOTH AgentMessages AND ToolCards loaded from persistence
    ↓
ToolCards restored to StreamingChatManager
    ↓
UI shows complete conversation with tool cards
    ↓
Copy function includes tool cards
```

## Fix Implementation Plan

### Phase 1: Add Tool Card Persistence

1. **Extend AgentMessage or Create ToolCardMessage**
   - Add tool card data to conversation persistence
   - Options:
     - A) Add `tool_cards: Vec<ToolCard>` to `AgentMessage`
     - B) Create separate `ToolCardMessage` type in persistence layer

2. **Update Conversation Save Logic**
   - Save tool cards when conversation is updated
   - Location: Conversation service save methods

3. **Update Conversation Load Logic**
   - Load tool cards alongside messages
   - Location: `handle_conversation_messages()` in `events.rs`

### Phase 2: Fix Message Loading

1. **Update handle_conversation_messages()**
   ```rust
   pub fn handle_conversation_messages(&mut self, conversation_id: uuid::Uuid, messages: Vec<AgentMessage>) {
       // Current: Only converts AgentMessages to StreamingMessages
       // Fix: Also extract and restore ToolCards to StreamingChatManager
       for agent_message in messages {
           // Convert to StreamingMessage (existing code)
           let streaming_message: StreamingMessage = chat_message.into();
           self.chat_manager.add_complete_message(streaming_message);
           
           // NEW: Extract and restore tool cards
           if let Some(tool_cards) = agent_message.tool_cards {
               for tool_card in tool_cards {
                   // Restore tool card to manager
                   self.chat_manager.restore_tool_card(tool_card);
               }
           }
       }
   }
   ```

2. **Add StreamingChatManager::restore_tool_card()**
   ```rust
   pub fn restore_tool_card(&self, tool_card: ToolCard) {
       // Add to both tool_cards map and messages list
       let run_id = tool_card.run_id;
       
       {
           let mut tool_cards = self.tool_cards.lock().unwrap();
           tool_cards.insert(run_id, tool_card.clone());
       }
       
       {
           let mut messages = self.messages.lock().unwrap();
           messages.push(ChatItem::ToolCard(tool_card));
       }
       
       // Sort by timestamp to maintain order
       self.sort_messages_by_timestamp();
   }
   ```

### Phase 3: Testing & Validation

1. **Test Conversation Switching**
   - Execute tools in conversation A
   - Switch to conversation B, then back to A  
   - Verify tool cards are restored

2. **Test Copy Function**
   - Execute tools
   - Switch conversations and back
   - Verify copy includes tool cards with parameters and results

## Implementation Files to Modify

1. **`crates/sagitta-code/src/agent/message/types.rs`**
   - Add tool card persistence to AgentMessage

2. **`crates/sagitta-code/src/gui/app/events.rs`**
   - Update `handle_conversation_messages()` to restore tool cards

3. **`crates/sagitta-code/src/gui/chat/mod.rs`**
   - Add `restore_tool_card()` method to StreamingChatManager

4. **`crates/sagitta-code/src/agent/conversation/persistence/`**
   - Update persistence layer to save/load tool cards

## Success Criteria

After implementation, these should work:

1. **Tool Card Persistence**: Tool cards survive conversation switching
2. **Copy Function**: "Copy entire conversation" includes tool cards with parameters and results
3. **UI Consistency**: Tool cards display correctly after conversation switch
4. **Data Integrity**: No tool card data loss during normal usage

## Notes

- This is a fundamental architecture issue, not a simple bug
- Tool cards were designed as UI-only entities, but need persistence
- The fix requires changes across multiple layers: UI, persistence, and data conversion