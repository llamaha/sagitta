# Terminal Stream

A Rust crate for real-time terminal streaming in egui applications. This crate provides a terminal widget that can display streaming command output, logs, and other terminal-like content with customizable appearance and behavior.

## Features

- **Real-time streaming**: Display live terminal output as it happens
- **egui integration**: Native egui widget that fits seamlessly into your applications
- **Configurable appearance**: Customizable colors, fonts, and display options
- **Search functionality**: Built-in search with highlighting
- **Command tracking**: Associate output with specific commands
- **Buffer management**: Automatic cleanup of old content
- **Event-driven**: Channel-based event system for streaming content
- **Comprehensive testing**: 73+ tests covering all functionality

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
terminal-stream = "0.1.0"
egui = "0.29"
crossbeam-channel = "0.5"
```

Basic usage:

```rust
use terminal_stream::{TerminalConfig, TerminalWidget, StreamEvent};
use crossbeam_channel;

// Create a channel for streaming events
let (sender, receiver) = crossbeam_channel::unbounded();

// Configure the terminal
let config = TerminalConfig::new()
    .with_max_lines(1000)
    .unwrap()
    .with_auto_scroll(true)
    .with_timestamps(true);

// Create the widget with event receiver
let mut terminal = TerminalWidget::new(config)
    .with_search_enabled(true)
    .with_event_receiver(Some(receiver));

// In your egui update loop:
ui.add(&mut terminal);

// Send events from another thread:
sender.send(StreamEvent::stdout(None, "Hello, terminal!".to_string()));
```

## Event Types

The crate supports various types of terminal events:

```rust
use terminal_stream::StreamEvent;

// Standard output
StreamEvent::stdout(None, "Regular output".to_string());

// Error output
StreamEvent::stderr(None, "Error message".to_string());

// Command execution
StreamEvent::command("ls -la".to_string());

// System messages
StreamEvent::system("Process completed".to_string());

// Error events
StreamEvent::error("Something went wrong".to_string());

// Clear terminal
StreamEvent::clear();
```

## Configuration

Extensive configuration options are available:

```rust
use terminal_stream::{TerminalConfig, TerminalColors, BufferConfig, StreamingConfig};
use egui::Color32;

let config = TerminalConfig::new()
    .with_max_lines(5000).unwrap()
    .with_auto_scroll(true)
    .with_timestamps(true)
    .with_font_size(14.0).unwrap()
    .with_colors(TerminalColors {
        stdout: Color32::WHITE,
        stderr: Color32::RED,
        command: Color32::GREEN,
        system: Color32::BLUE,
        error: Color32::YELLOW,
        background: Color32::BLACK,
        timestamp: Color32::GRAY,
    })
    .with_buffer_config(BufferConfig {
        max_lines: 5000,
        lines_to_keep: 2500,
        cleanup_interval_ms: 1000,
    }).unwrap()
    .with_streaming_config(StreamingConfig {
        update_interval_ms: 50,
        stream_buffer_size: 8192,
        process_ansi: true,
    }).unwrap();
```

## Examples

Run the basic usage example:

```bash
cargo run --example basic_usage
```

This will open a window with a terminal widget and buttons to simulate various terminal events.

## Architecture

The crate is organized into several modules:

- **`events`**: Event types and command tracking
- **`buffer`**: Terminal line storage and management  
- **`widget`**: egui widget implementation
- **`config`**: Configuration structures and validation
- **`error`**: Error types and handling

## Testing

The crate includes comprehensive tests covering all functionality:

```bash
cargo test
```

All 73 tests should pass, covering:
- Event creation and serialization
- Buffer management and cleanup
- Widget functionality and rendering
- Configuration validation
- Error handling

## License

This project is licensed under the MIT License.

## Contributing

Contributions are welcome! Please ensure all tests pass and add tests for new functionality. 