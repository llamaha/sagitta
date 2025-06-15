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



/// Tasks
pub mod tasks;

/// Conversation functionality 
pub mod conversation {
    pub use crate::agent::conversation::*;
}

// Re-export commonly used types
pub use config::SagittaCodeConfig;
pub use utils::init_logger;
pub use tasks::{Task, TaskManager, TaskScheduler};
pub use tasks::conversation::ConversationTaskIntegration;

// Re-export agent types for tests
pub use agent::events::AgentEvent;
pub use agent::state::types::{AgentMode, AgentState, ConversationStatus};

// Re-export LLM types for tests
pub use llm::client::{
    LlmClient, Message, MessagePart, Role, ToolDefinition,
    LlmResponse, StreamChunk, ThinkingConfig, GroundingConfig, TokenUsage,
};

// Re-export tool types for tests
pub use tools::{Tool, ToolDefinition as ToolDefAlias, ToolResult, ToolCategory, ToolRegistry};

// Re-export reasoning types for tests  
pub use reasoning::{AgentToolExecutor, AgentEventEmitter}; 