# Modern Compact Chat System

This is a completely redesigned chat interface that eliminates speech bubbles in favor of a compact, space-efficient design inspired by modern development tools and terminals.

## Key Features

### ✅ **Fixed Issues**
- **No more speech bubbles** - Saves significant screen space
- **Always-visible copy buttons** - No more flickering hover issues
- **Unified tool display** - Tool calls, results, and outputs are consolidated
- **Clickable tool results** - Easy access to detailed tool information
- **Real streaming support** - Proper text streaming with visual indicators
- **Working thinking mode** - Expandable thinking content display

### 🎨 **Design Principles**
- **Space Efficient**: Maximum content, minimal UI overhead
- **Information Dense**: More information visible at once
- **Accessible**: Always-visible controls, no hidden functionality
- **Modern**: Clean, terminal-inspired aesthetic
- **Responsive**: Adapts to different screen sizes

## Visual Layout

```
[Author] [Time] [Status] [Copy]
💭 Thinking... (expandable)
🔧 ✓ tool_name: preview result...
📊 Tool Result: summary (clickable)
Main message content with markdown support
💻 rust [Copy]
┌─────────────────────────────────────┐
│ fn main() {                         │
│     println!("Hello, world!");     │
│ }                                   │
└─────────────────────────────────────┐
▋ (streaming cursor)
────────────────────────────────────────
```

## Usage

### Basic Usage

```rust
use crate::gui::chat::view::{modern_chat_view_ui, StreamingMessage};

// In your UI code
modern_chat_view_ui(ui, &messages, app_theme);
```

### Streaming Integration

```rust
use crate::gui::chat::StreamingChatManager;

let chat_manager = StreamingChatManager::new();

// Add user message
let user_id = chat_manager.add_user_message("Hello!".to_string());

// Start agent response with streaming
let agent_id = chat_manager.start_agent_response();

// Set thinking mode
chat_manager.set_thinking(&agent_id, "Let me think about this...".to_string());

// Stream content chunks
chat_manager.append_content(&agent_id, "I think ".to_string());
chat_manager.append_content(&agent_id, "the answer is...".to_string());

// Add tool calls
let tool_call = ToolCall {
    name: "web_search".to_string(),
    arguments: r#"{"query": "rust programming"}"#.to_string(),
    result: Some("Found 10 results".to_string()),
    status: MessageStatus::Complete,
};
chat_manager.add_tool_call(&agent_id, tool_call);

// Finish streaming
chat_manager.finish_streaming(&agent_id);

// Get messages for display
let messages = chat_manager.get_all_messages();
modern_chat_view_ui(ui, &messages, app_theme);
```

## Message Types

### StreamingMessage
The core message type that supports real-time updates:

```rust
pub struct StreamingMessage {
    pub id: String,
    pub author: MessageAuthor,
    pub content: String,
    pub status: MessageStatus,
    pub thinking_content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
```

### MessageStatus
Tracks the current state of a message:

```rust
pub enum MessageStatus {
    Sending,     // ⏳ Message being prepared
    Thinking,    // 🤔 AI is thinking
    Streaming,   // ⟳ Content being streamed
    Complete,    // ✓ Message finished
    Error(String), // ✗ Error occurred
}
```

### MessageAuthor
Different types of message authors with color coding:

```rust
pub enum MessageAuthor {
    User,    // Blue - You
    Agent,   // Green - Sagitta Code
    System,  // Red - System
    Tool,    // Yellow - Tool
}
```

## Features

### 🧠 Thinking Mode
- Expandable thinking content
- Smooth transition from thinking to streaming
- Visual indicator with animated emoji

### 🔧 Tool Integration
- Compact tool execution display
- Status indicators (✓ ✗ ⟳ 🤔)
- Clickable tool names for details
- Result previews with truncation

### 📊 Tool Results
- Smart result detection
- Clickable summaries
- Automatic content extraction
- Preview in side panel (TODO)

### 💻 Code Blocks
- Syntax highlighting
- Language detection
- Copy functionality
- Collapsible for long code (>10 lines)
- Performance optimized (max 20 lines rendered)

### ⚡ Streaming
- Real-time content updates
- Animated cursor indicator
- Smooth status transitions
- Thread-safe message management

## Integration Guide

### 1. Replace Old Chat View
```rust
// Old
chat_view_ui(ui, &legacy_messages, app_theme);

// New
let streaming_messages: Vec<StreamingMessage> = legacy_messages
    .iter()
    .map(|msg| msg.clone().into())
    .collect();
modern_chat_view_ui(ui, &streaming_messages, app_theme);
```

### 2. Implement Streaming
```rust
// Create manager
let chat_manager = Arc::new(StreamingChatManager::new());

// In your AI response handler
let response_id = chat_manager.start_agent_response();

// For thinking mode (Gemini thinking)
if let Some(thinking) = response.thinking_content {
    chat_manager.set_thinking(&response_id, thinking);
}

// For streaming content
for chunk in response.content_stream {
    chat_manager.append_content(&response_id, chunk);
    // Trigger UI update here
}

// For tool calls
for tool_call in response.tool_calls {
    chat_manager.add_tool_call(&response_id, tool_call);
}

// When done
chat_manager.finish_streaming(&response_id);
```

### 3. Handle Errors
```rust
// On error
chat_manager.set_error(&response_id, "Network timeout".to_string());
```

## Performance Considerations

- **Code rendering**: Limited to 20 lines for performance
- **Long code blocks**: Automatically collapsed
- **Message batching**: Use `append_content` for chunks
- **Thread safety**: All operations are thread-safe
- **Memory efficient**: Minimal UI overhead

## Customization

### Themes
The system uses Catppuccin themes with author-specific colors:
- User: Blue accent
- Agent: Green accent  
- System: Red accent
- Tool: Yellow accent

### Spacing
Compact spacing optimized for information density:
- Message spacing: 8px
- Content padding: Minimal
- Separator lines: Subtle visual breaks

### Typography
- Author names: 12px, bold, colored
- Timestamps: 10px, subtle
- Content: Standard markdown rendering
- Code: 10px monospace with syntax highlighting

## Migration from Speech Bubbles

The new system eliminates several issues:

1. **Space waste**: No more bubble padding and margins
2. **Copy button issues**: Always visible, no hover required
3. **Tool fragmentation**: Unified tool display
4. **Streaming problems**: Proper real-time updates
5. **Thinking invisibility**: Expandable thinking content

## Testing

Run the comprehensive test suite:
```bash
cargo test --all --release --features ort/cuda chat::
```

Tests cover:
- Message creation and management
- Streaming functionality
- Status transitions
- Tool integration
- Error handling
- UI rendering logic

## Future Enhancements

- [ ] Side panel for detailed tool results
- [ ] Message search and filtering
- [ ] Export functionality
- [ ] Custom themes
- [ ] Keyboard shortcuts
- [ ] Message threading
- [ ] Collaborative features 