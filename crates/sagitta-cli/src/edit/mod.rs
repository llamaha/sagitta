// src/edit/mod.rs
//! Core module for handling code editing operations.

pub mod cli;    // Placeholder for CLI command implementation (later)

// Re-export CLI args and handler
pub use cli::{EditArgs, ApplyArgs, ValidateArgs, handle_edit_command};

// Potentially re-export key functions or structs if needed
// pub use engine::apply_edit; 