//! # Terminal Stream
//! 
//! A real-time terminal streaming widget for egui applications.
//! 
//! This crate provides components for displaying streaming command output,
//! handling stdout/stderr differentiation, and providing terminal-like 
//! functionality within egui applications.

pub mod events;
pub mod buffer;
pub mod widget;
pub mod config;
pub mod error;
mod ui_tool_result;

// Re-export main types
pub use events::{StreamEvent, LineType, CommandInfo, ExitInfo};
pub use buffer::{TerminalBuffer, TerminalLine};
pub use widget::TerminalWidget;
pub use config::TerminalConfig;
pub use error::{TerminalError, Result};
pub use ui_tool_result::UiToolResultChunk; 