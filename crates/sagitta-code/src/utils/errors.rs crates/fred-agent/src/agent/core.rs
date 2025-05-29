use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum SagittaCodeError {
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("LLM client error: {0}")]
    LlmError(String),
    
    #[error("Tool execution error: {0}")]
    ToolError(String),
    
    #[error("Tool not found: {0}")]
    ToolNotFound(String),
    
    #[error("Sagitta core error: {0}")]
    SagittaDbError(String),
    
    #[error("IO error: {0}")]
    IoError(String),
    
    #[error("Network error: {0}")]
    NetworkError(String),
}

/// Implement From for broadcast::error::SendError
impl<T> From<tokio::sync::broadcast::error::SendError<T>> for SagittaCodeError {
    fn from(err: tokio::sync::broadcast::error::SendError<T>) -> Self {
        SagittaCodeError::EventError(format!("Failed to send event: {}", err))
    }
}

// Add manual From<std::io::Error> for SagittaCodeError
impl From<std::io::Error> for SagittaCodeError {
    fn from(err: std::io::Error) -> Self {
        SagittaCodeError::IoError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;
    use mockall::predicate;
    use crate::tools::types::ToolCategory;
    use std::collections::HashMap;
} 