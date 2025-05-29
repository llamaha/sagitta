//! Fred AI agent built on sagitta-search
//! 
//! This crate provides a standalone agent for interacting with code repositories
//! using Gemini and sagitta-search. It eliminates network dependencies by
//! integrating LLM functionality directly.

/// Agent implementation and messaging
pub mod agent;

/// Configuration management
pub mod config;

/// LLM clients (Gemini)
pub mod llm;

/// Tools for agent interaction
pub mod tools;

/// Utilities
pub mod utils;

/// GUI components
pub mod gui;

/// Reasoning module
pub mod reasoning;

// Re-export commonly used items
pub use agent::Agent;
pub use config::FredAgentConfig;
pub use utils::init_logger; 