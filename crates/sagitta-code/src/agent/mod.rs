pub mod core;
pub mod message;
pub mod state;
pub mod conversation;
pub mod events;
pub mod recovery;
pub mod streaming;
pub mod prompts;

#[cfg(test)]
mod cancellation_tests;

// New reasoning engine modules
// pub mod reasoning_adapter;
// pub mod reasoning_config;
// pub mod new_reasoning;

// Re-export the Agent struct for easy access
pub use core::Agent;

