//! Sagitta Code AI agent built on sagitta-search
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

/// Project management
pub mod project;

/// Tasks
pub mod tasks;

// Re-export commonly used types
pub use config::SagittaCodeConfig;
pub use utils::init_logger;
pub use tasks::{Task, TaskManager, TaskScheduler};
pub use tasks::conversation::ConversationTaskIntegration; 