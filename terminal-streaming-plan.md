# Terminal Streaming Implementation Plan

## Overview
Implement real-time streaming of shell command output to provide users with live feedback during code execution, testing, and build processes. This will replace the current "black box" experience where users wait without knowing what's happening.

## Current State Analysis

### What We Have Now
- **Shell Execution Tool**: Executes commands in Docker containers but only returns final output
- **No Real-time Feedback**: Users see nothing until command completes (or times out)
- **Poor UX**: Long-running commands (like Rust compilation) appear to hang
- **Limited Debugging**: Can't see intermediate output or progress

### What We Need
- **Live Output Streaming**: Real-time stdout/stderr as commands execute
- **Visual Terminal Interface**: Terminal-like display component in GUI
- **Progress Indication**: Clear feedback that commands are running
- **Interruptible Operations**: Allow users to cancel long-running commands

---

## Architecture Design

### 1. Stream Processing Layer

#### Core Streaming Infrastructure
**File**: `crates/sagitta-code/src/streaming/mod.rs`
```rust
// New module structure:
pub mod terminal_stream;     // Core streaming logic
pub mod output_buffer;       // Buffered output management  
pub mod stream_processor;    // Process stdout/stderr streams
pub mod progress_tracker;    // Track command progress
```

#### Stream Event Types
**File**: `crates/sagitta-code/src/streaming/events.rs`
```rust
pub enum StreamEvent {
    StdOut(String),           // Standard output chunk
    StdErr(String),           // Standard error chunk  
    Progress(ProgressInfo),   // Progress indication
    CommandStart(CommandInfo), // Command execution started
    CommandEnd(ExitInfo),     // Command completed
    ContainerPull(PullInfo),  // Docker image pull progress
    Error(StreamError),       // Stream processing error
}
```

### 2. Enhanced Shell Execution Tool

#### Streaming Shell Tool
**File**: `crates/sagitta-code/src/tools/shell_execution.rs`

**Changes Needed**:
- Add streaming capabilities to existing `ShellExecutionTool`
- Implement async stream processing with `tokio::process::Command`
- Add real-time stdout/stderr capture using `AsyncBufReadExt`
- Maintain backward compatibility with existing non-streaming interface

**New Methods**:
```rust
impl ShellExecutionTool {
    // New streaming method
    pub async fn execute_streaming(
        &self, 
        params: ShellExecutionParams,
        stream_sender: mpsc::UnboundedSender<StreamEvent>
    ) -> Result<ShellExecutionResult>;
    
    // Enhanced Docker execution with streaming
    async fn execute_in_container_streaming(...);
}
```

### 3. GUI Terminal Component

#### Terminal Widget Implementation
**File**: `crates/sagitta-code/src/gui/terminal/mod.rs`
```rust
// New GUI component structure:
pub mod terminal_widget;     // Main terminal display widget
pub mod terminal_buffer;     // Text buffer management
pub mod terminal_theme;      // Colors, fonts, styling
pub mod scroll_manager;      // Auto-scroll and user scroll
```

#### Terminal Display Features
- **Syntax Highlighting**: Different colors for stdout vs stderr
- **Auto-scrolling**: Follow output automatically, pause when user scrolls up
- **Text Selection**: Allow copying output text
- **Search Functionality**: Find text in output history
- **Export Capability**: Save terminal output to file
- **Clear/Reset**: Clear terminal or reset to clean state

#### Integration Points
**Files to Modify**:
- `crates/sagitta-code/src/gui/tools/panel.rs` - Add terminal to tools panel
- `crates/sagitta-code/src/gui/app/layout.rs` - Terminal layout management
- `crates/sagitta-code/src/gui/state.rs` - Terminal state management

### 4. CLI Streaming Support

#### CLI Terminal Output
**File**: `crates/sagitta-code/src/bin/chat_cli.rs`

**Enhancements**:
- Stream output directly to console in real-time
- Preserve existing piped input/output functionality
- Add progress indicators for long operations
- Color-coded output (if terminal supports it)

---

## Implementation Phases

### Phase 1: Core Streaming Infrastructure ⭐ **Priority**
**Estimated Time**: 2-3 days

1. **Create Streaming Module**
   - Implement `StreamEvent` enum and core types
   - Create `TerminalStream` for managing output streams
   - Add `OutputBuffer` for efficient text storage

2. **Enhance Shell Execution Tool**
   - Add `execute_streaming()` method to `ShellExecutionTool`
   - Implement real-time stdout/stderr capture
   - Maintain compatibility with existing `execute()` method

3. **Basic CLI Streaming**
   - Update `chat_cli` to use streaming shell execution
   - Stream output directly to console
   - Test with simple commands

**Files to Create/Modify**:
- `crates/sagitta-code/src/streaming/` (new module)
- `crates/sagitta-code/src/tools/shell_execution.rs`
- `crates/sagitta-code/src/bin/chat_cli.rs`

### Phase 2: GUI Terminal Component ⭐ **Priority**
**Estimated Time**: 3-4 days

1. **Terminal Widget Development**
   - Create egui-based terminal display component
   - Implement text rendering with color support
   - Add auto-scroll and manual scroll capabilities

2. **Integration with Tools Panel**
   - Add terminal tab to tools panel
   - Connect streaming shell execution to terminal display
   - Implement terminal controls (clear, pause, export)

3. **State Management**
   - Terminal state persistence across sessions
   - Buffer size limits and cleanup
   - Integration with existing app state

**Files to Create/Modify**:
- `crates/sagitta-code/src/gui/terminal/` (new module)
- `crates/sagitta-code/src/gui/tools/panel.rs`
- `crates/sagitta-code/src/gui/state.rs`

### Phase 3: Advanced Features 🔮 **Future**
**Estimated Time**: 2-3 days

1. **Enhanced User Experience**
   - Command interruption/cancellation
   - Multiple terminal tabs
   - Command history and replay

2. **Progress Tracking**
   - Docker image pull progress
   - Compilation progress indicators
   - Estimated time remaining

3. **Advanced Terminal Features**
   - Terminal themes and customization
   - Export to different formats (text, HTML)
   - Integration with project creation workflow

---

## Technical Considerations

### Performance Requirements
- **Buffer Management**: Limit terminal buffer size (10,000-50,000 lines)
- **Efficient Rendering**: Only render visible text in GUI
- **Memory Usage**: Stream processing should not accumulate unbounded memory
- **Responsive UI**: Terminal updates shouldn't block GUI responsiveness

### Docker Integration Challenges
- **Container Lifecycle**: Proper cleanup when streaming is interrupted
- **Image Pull Progress**: Show Docker image download progress
- **Multiple Commands**: Handle sequences of commands in same container
- **Error Handling**: Graceful handling of Docker daemon issues

### Cross-Platform Considerations
- **ANSI Codes**: Handle terminal escape sequences appropriately
- **Character Encoding**: Proper UTF-8 handling across platforms
- **Color Support**: Detect and adapt to terminal color capabilities

---

## User Experience Goals

### GUI Experience
1. **Immediate Feedback**: Users see output within 100ms of command start
2. **Clear Progress**: Visual indication of what's happening
3. **Non-Blocking**: GUI remains responsive during command execution
4. **Intuitive Controls**: Easy-to-find pause, stop, clear buttons

### CLI Experience
1. **Console Integration**: Output feels native to terminal environment
2. **Preserved Functionality**: Existing piping and redirection still works
3. **Progress Indicators**: Clear feedback for long operations
4. **Graceful Interruption**: Ctrl+C properly stops operations

---

## Testing Strategy

### Unit Tests
**Files**: `crates/sagitta-code/src/streaming/tests.rs`
- Stream event processing
- Buffer management edge cases
- Docker output parsing
- Terminal widget rendering

### Integration Tests
**Files**: `crates/sagitta-code/src/tests/streaming_integration.rs`
- End-to-end streaming workflows
- GUI terminal component integration
- CLI streaming functionality
- Error recovery scenarios

### Manual Testing Scenarios
1. **Long-running Commands**: Rust compilation, large file operations
2. **Docker Operations**: Image pulls, container startup
3. **Error Conditions**: Network failures, permission errors, timeouts
4. **User Interactions**: Scrolling, selection, cancellation
5. **Multiple Languages**: Test streaming with all Alpine containers

---

## Migration Strategy

### Backward Compatibility
- Keep existing `execute()` method unchanged
- Add new `execute_streaming()` as optional enhancement
- Gradual migration of tool calls to streaming version
- CLI maintains existing behavior for scripts and automation

### Rollout Plan
1. **Internal Testing**: Use streaming for development and testing
2. **Opt-in Feature**: Add configuration option to enable streaming
3. **Default Enabled**: Make streaming default after stability proven
4. **Legacy Removal**: Eventually remove non-streaming methods

---

## Implementation Checklist

### Phase 1 - Core Streaming
- [ ] Create `crates/sagitta-code/src/streaming/` module
- [ ] Implement `StreamEvent` and related types
- [ ] Add streaming methods to `ShellExecutionTool`
- [ ] Update CLI to use streaming execution
- [ ] Test basic streaming with simple commands
- [ ] Test with Docker container operations

### Phase 2 - GUI Terminal
- [ ] Create terminal widget with egui
- [ ] Implement text buffer and rendering
- [ ] Add terminal to tools panel
- [ ] Connect streaming events to terminal display
- [ ] Implement scroll management
- [ ] Add terminal controls (clear, export)

### Phase 3 - Polish & Features
- [ ] Add command interruption capability
- [ ] Implement progress tracking for Docker operations
- [ ] Add terminal themes and customization
- [ ] Create comprehensive test suite
- [ ] Performance optimization and buffer management
- [ ] Documentation and user guide

---

## Dependencies Required

### New Cargo Dependencies
```toml
# For async stream processing
tokio-util = "0.7"
futures = "0.3"

# For terminal color support
ansi_term = "0.12"
crossterm = "0.27"

# For GUI terminal component (already have egui)
# egui_extras for additional widgets if needed
```

### Integration Points
- **Existing Tools**: All tools that use `ShellExecutionTool`
- **Project Creation**: Stream project scaffolding output
- **Testing Tools**: Stream test execution output
- **Build Tools**: Stream compilation and build output

---

## Success Metrics

### User Experience Metrics
- **Perceived Performance**: Users see output within 100ms
- **Completion Rates**: Fewer timeouts and cancellations
- **User Satisfaction**: Positive feedback on streaming experience

### Technical Metrics
- **Memory Usage**: Terminal buffer stays under 50MB
- **CPU Usage**: Streaming adds <5% CPU overhead
- **UI Responsiveness**: GUI remains responsive during streaming
- **Error Recovery**: Graceful handling of 99%+ error scenarios

---

**Next Steps**: Start with Phase 1 implementation focusing on core streaming infrastructure and CLI integration, then move to GUI terminal component in Phase 2. 