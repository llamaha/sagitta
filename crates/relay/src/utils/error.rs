use thiserror::Error;

#[derive(Error, Debug)]
pub enum RelayError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("LLM API error: {0}")]
    LlmApiError(String),

    #[error("Chain execution error: {0}")]
    ChainError(String),

    #[error("Tool execution error: {0}")]
    ToolError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Vector database error: {0}")]
    VectorDBError(#[from] vectordb_core::error::VectorDBError),

    #[error(transparent)]
    Other(#[from] anyhow::Error), // source and Display delegate to anyhow::Error

    #[error("LLM interaction error: {0}")]
    LLMError(String), // Changed from #[from] crate::llm::LLMError
}

// Define a standard result type for the crate
pub type Result<T> = std::result::Result<T, RelayError>;
