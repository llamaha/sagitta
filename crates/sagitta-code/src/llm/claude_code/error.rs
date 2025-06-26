use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClaudeCodeError {
    #[error("Claude binary not found at path: {0}")]
    BinaryNotFound(String),
    
    #[error("Authentication error: {0}")]
    AuthenticationError(String),
    
    #[error("Model access error: {0}")]
    ModelAccessError(String),
    
    #[error("Process spawn error: {0}")]
    ProcessError(#[from] io::Error),
    
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Stream error: {0}")]
    StreamError(String),
    
    #[error("Timeout error: process did not complete within {0} seconds")]
    TimeoutError(u64),
    
    #[error("Claude process exited with code {code}: {message}")]
    ProcessExitError { code: i32, message: String },
    
    #[error("Unexpected error: {0}")]
    UnexpectedError(String),
}

impl From<serde_json::Error> for ClaudeCodeError {
    fn from(err: serde_json::Error) -> Self {
        ClaudeCodeError::ParseError(err.to_string())
    }
}