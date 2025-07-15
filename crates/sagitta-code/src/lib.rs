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

/// Provider abstraction layer
pub mod providers;

// Tools removed - now provided via MCP from sagitta-mcp
// Minimal stub module for compilation compatibility
mod tools_stub;
pub mod tools {
    pub mod registry {
        pub use crate::tools_stub::ToolRegistry;
    }
    pub mod executor {
        pub use crate::tools_stub::{ToolExecutor, SagittaCodeToolExecutorInternal};
        pub use crate::agent::events::ToolExecutionEvent;
    }
    pub mod types {
        pub use crate::tools_stub::{ToolResult, ToolDefinition, ToolCategory};
    }
    pub use crate::tools_stub::WorkingDirectoryManager;
}

/// Utilities
pub mod utils;

/// GUI components
pub mod gui;

/// Tasks
pub mod tasks;

/// Services (file watching, auto-commit, etc.)
pub mod services;

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

// Re-export tool stub types for compatibility
pub use tools::{
    types::{ToolResult, ToolDefinition as ToolDefAlias, ToolCategory},
    registry::ToolRegistry,
}; 