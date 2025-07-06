use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum SagittaCodeError {
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("LLM client error: {0}")]
    LlmError(String),
    
    #[error("Tool error: {0}")]
    ToolError(String),
    
    #[error("Tool not found: {0}")]
    ToolNotFound(String),
    
    #[error("Sagitta core error: {0}")]
    SagittaDbError(String),
    
    #[error("IO error: {0}")]
    IoError(String),
    
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("Event error: {0}")]
    EventError(String),
    
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Unknown error: {0}")]
    Unknown(String),
}

/// Convert any error to a SagittaCodeError
pub fn to_agent_error<E: std::error::Error>(err: E) -> SagittaCodeError {
    SagittaCodeError::Unknown(err.to_string())
}

/// Log an error and return it (for use in ? operator chains)
pub fn log_error<T, E: std::error::Error>(result: Result<T, E>, context: &str) -> Result<T, E> {
    if let Err(ref e) = result {
        log::error!("{context}: {e}");
    }
    result
}

/// Implement From for broadcast::error::SendError
impl<T> From<tokio::sync::broadcast::error::SendError<T>> for SagittaCodeError {
    fn from(err: tokio::sync::broadcast::error::SendError<T>) -> Self {
        SagittaCodeError::EventError(format!("Failed to send event: {err}"))
    }
}

// Add manual From<std::io::Error> for SagittaCodeError
impl From<std::io::Error> for SagittaCodeError {
    fn from(err: std::io::Error) -> Self {
        SagittaCodeError::IoError(err.to_string())
    }
}

// Add manual From<serde_json::Error> for SagittaCodeError
impl From<serde_json::Error> for SagittaCodeError {
    fn from(err: serde_json::Error) -> Self {
        SagittaCodeError::Unknown(format!("JSON serialization error: {err}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    use std::error::Error;
    use tokio::sync::broadcast;

    #[test]
    fn test_config_error() {
        let error = SagittaCodeError::ConfigError("Invalid configuration".to_string());
        assert_eq!(error.to_string(), "Configuration error: Invalid configuration");
    }

    #[test]
    fn test_llm_error() {
        let error = SagittaCodeError::LlmError("API request failed".to_string());
        assert_eq!(error.to_string(), "LLM client error: API request failed");
    }

    #[test]
    fn test_tool_error() {
        let error = SagittaCodeError::ToolError("Tool execution failed".to_string());
        assert_eq!(error.to_string(), "Tool error: Tool execution failed");
    }

    #[test]
    fn test_tool_not_found() {
        let error = SagittaCodeError::ToolNotFound("search_tool".to_string());
        assert_eq!(error.to_string(), "Tool not found: search_tool");
    }

    #[test]
    fn test_sagitta_error() {
        let error = SagittaCodeError::SagittaDbError("Database connection failed".to_string());
        assert_eq!(error.to_string(), "Sagitta core error: Database connection failed");
    }

    #[test]
    fn test_io_error_conversion() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found"); // Fully qualify
        let agent_error = SagittaCodeError::from(io_error);
        
        match agent_error {
            SagittaCodeError::IoError(ref msg) => { // Updated to match new IoError(String)
                assert!(msg.contains("File not found"));
                assert!(agent_error.to_string().contains("File not found"));
            },
            _ => panic!("Expected IoError variant"),
        }
    }

    #[test]
    fn test_network_error() {
        let error = SagittaCodeError::NetworkError("Connection timeout".to_string());
        assert_eq!(error.to_string(), "Network error: Connection timeout");
    }

    #[test]
    fn test_event_error() {
        let error = SagittaCodeError::EventError("Event broadcast failed".to_string());
        assert_eq!(error.to_string(), "Event error: Event broadcast failed");
    }

    #[test]
    fn test_unknown_error() {
        let error = SagittaCodeError::Unknown("Unexpected error occurred".to_string());
        assert_eq!(error.to_string(), "Unknown error: Unexpected error occurred");
    }

    #[test]
    fn test_to_agent_error() {
        let original_error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Access denied"); // Fully qualify
        let agent_error = to_agent_error(original_error);
        
        match agent_error {
            SagittaCodeError::Unknown(msg) => {
                assert!(msg.contains("Access denied"));
            },
            _ => panic!("Expected Unknown variant"),
        }
    }

    #[test]
    fn test_log_error_success() {
        let result: Result<i32, std::io::Error> = Ok(42); // Fully qualify
        let logged_result = log_error(result, "test context");
        
        assert!(logged_result.is_ok());
        assert_eq!(logged_result.unwrap(), 42);
    }

    #[test]
    fn test_log_error_failure() {
        let result: Result<i32, std::io::Error> = Err(std::io::Error::other("test error")); // Fully qualify
        let logged_result = log_error(result, "test context");
        
        assert!(logged_result.is_err());
        assert!(logged_result.unwrap_err().to_string().contains("test error"));
    }

    #[test]
    fn test_broadcast_send_error_conversion() {
        let (tx, _rx) = broadcast::channel::<String>(1);
        drop(_rx); // Close the receiver to cause a send error
        
        let send_result = tx.send("test message".to_string());
        assert!(send_result.is_err());
        
        let agent_error = SagittaCodeError::from(send_result.unwrap_err());
        match agent_error {
            SagittaCodeError::EventError(msg) => {
                assert!(msg.contains("Failed to send event"));
            },
            _ => panic!("Expected EventError variant"),
        }
    }

    #[test]
    fn test_error_debug_format() {
        let error = SagittaCodeError::ConfigError("test".to_string());
        let debug_str = format!("{error:?}");
        assert!(debug_str.contains("ConfigError"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_error_chain() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "Original error"); // Fully qualify
        let agent_error = SagittaCodeError::from(io_error);
        
        // Test that the error chain is preserved, now it won't be for IoError(String)
        // as it doesn't store the original error object.
        // assert!(agent_error.source().is_some()); // This will now fail for IoError(String)
        // Instead, check if the source() is None for our new IoError(String)
        if let SagittaCodeError::IoError(_) = agent_error {
            assert!(agent_error.source().is_none(), "IoError(String) should not have a source().");
        } else {
            // For other errors that might have a source, this could still be true
            // For simplicity, this test now focuses on the IoError case
        }
    }

    #[test]
    fn test_error_variants_exhaustive() {
        // Test that we can create all error variants
        let errors = vec![
            SagittaCodeError::ConfigError("config".to_string()),
            SagittaCodeError::LlmError("llm".to_string()),
            SagittaCodeError::ToolError("tool".to_string()),
            SagittaCodeError::ToolNotFound("not_found".to_string()),
            SagittaCodeError::SagittaDbError("sagitta".to_string()),
            SagittaCodeError::IoError("io error string".to_string()), // Updated
            SagittaCodeError::NetworkError("network".to_string()),
            SagittaCodeError::EventError("event".to_string()),
            SagittaCodeError::ParseError("parse error".to_string()),
            SagittaCodeError::Unknown("unknown".to_string()),
        ];
        
        assert_eq!(errors.len(), 10); // Updated count after removing ReasoningError
        
        for error in errors {
            // Each error should have a meaningful string representation
            let error_str = error.to_string();
            assert!(!error_str.is_empty());
            assert!(error_str.len() > 5); // Should be more than just the variant name
        }
    }

    #[test]
    fn test_error_equality_and_comparison() {
        let error1 = SagittaCodeError::ConfigError("same message".to_string());
        let error2 = SagittaCodeError::ConfigError("same message".to_string());
        let error3 = SagittaCodeError::ConfigError("different message".to_string());
        
        // Test based on Clone + Debug (if PartialEq is not derived)
        // SagittaCodeError derives Clone, so we can compare cloned instances
        assert_eq!(format!("{error1:?}"), format!("{:?}", error2));
        assert_ne!(format!("{error1:?}"), format!("{:?}", error3));

        let io_error1 = SagittaCodeError::IoError("io same".to_string());
        let io_error2 = SagittaCodeError::IoError("io same".to_string());
        assert_eq!(format!("{io_error1:?}"), format!("{:?}", io_error2));
    }

    #[test]
    fn test_error_with_empty_message() {
        let error = SagittaCodeError::Unknown("".to_string());
        assert_eq!(error.to_string(), "Unknown error: ");
    }

    #[test]
    fn test_error_with_special_characters() {
        let special_msg = "Error with special chars: !@#$%^&*(){}[]|\\:;\"'<>,.?/~`";
        let error = SagittaCodeError::ToolError(special_msg.to_string());
        assert!(error.to_string().contains(special_msg));
    }

    #[test]
    fn test_error_with_unicode() {
        let unicode_msg = "Error with unicode: 🚨 错误 エラー";
        let error = SagittaCodeError::NetworkError(unicode_msg.to_string());
        assert!(error.to_string().contains(unicode_msg));
    }
}
