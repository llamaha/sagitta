//! This module defines the command-line interface structure and handlers.

pub mod commands;
pub mod formatters;
pub mod stats;

// New module declarations for command handlers
pub mod clear;
pub mod repo_commands;
pub mod simple;
pub mod utils;
// Add mods for stats, clear, list, remove when created

// Re-export the main handler and the command enum
pub use commands::{handle_command, Commands};

// Re-export the Args structs for use in the main binary
pub use stats::StatsArgs;
pub use clear::ClearArgs;
pub use repo_commands::RepoArgs;
pub use commands::CliArgs;

// Potentially re-export other args structs here later...

// Re-export Command enum if needed by binary directly
// pub use commands::Command;
