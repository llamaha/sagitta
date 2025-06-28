pub mod client;
pub mod error;
pub mod models;
pub mod streaming;
pub mod process;
pub mod message_converter;
pub mod incremental_parser;
pub mod claude_interface;
pub mod mcp_integration;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod integration_tests;

pub use client::ClaudeCodeClient;
pub use error::ClaudeCodeError;
pub use models::{CLAUDE_CODE_MODELS, ClaudeCodeModel};
pub use claude_interface::{ClaudeInterface, ClaudeModelInfo, ClaudeConfigInfo};