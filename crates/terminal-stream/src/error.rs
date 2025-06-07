use thiserror::Error;

/// Result type for terminal streaming operations
pub type Result<T> = std::result::Result<T, TerminalError>;

/// Errors that can occur in terminal streaming operations
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum TerminalError {
    /// Buffer capacity exceeded
    #[error("Terminal buffer capacity exceeded: {current} >= {max}")]
    BufferCapacityExceeded { current: usize, max: usize },

    /// Invalid configuration
    #[error("Invalid terminal configuration: {reason}")]
    InvalidConfig { reason: String },

    /// Stream processing error
    #[error("Stream processing failed: {details}")]
    StreamProcessing { details: String },

    /// Command execution error
    #[error("Command execution failed: {command} - {reason}")]
    CommandExecution { command: String, reason: String },

    /// IO error during streaming
    #[error("IO error: {message}")]
    Io { message: String },

    /// Channel communication error
    #[error("Channel communication error: {details}")]
    Channel { details: String },

    /// Widget rendering error
    #[error("Widget rendering error: {context}")]
    Rendering { context: String },
}

impl TerminalError {
    /// Create a new buffer capacity error
    pub fn buffer_capacity_exceeded(current: usize, max: usize) -> Self {
        Self::BufferCapacityExceeded { current, max }
    }

    /// Create a new invalid config error
    pub fn invalid_config(reason: impl Into<String>) -> Self {
        Self::InvalidConfig {
            reason: reason.into(),
        }
    }

    /// Create a new stream processing error
    pub fn stream_processing(details: impl Into<String>) -> Self {
        Self::StreamProcessing {
            details: details.into(),
        }
    }

    /// Create a new command execution error
    pub fn command_execution(command: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::CommandExecution {
            command: command.into(),
            reason: reason.into(),
        }
    }

    /// Create a new IO error
    pub fn io(message: impl Into<String>) -> Self {
        Self::Io {
            message: message.into(),
        }
    }

    /// Create a new channel error
    pub fn channel(details: impl Into<String>) -> Self {
        Self::Channel {
            details: details.into(),
        }
    }

    /// Create a new rendering error
    pub fn rendering(context: impl Into<String>) -> Self {
        Self::Rendering {
            context: context.into(),
        }
    }

    /// Check if this is a recoverable error
    pub fn is_recoverable(&self) -> bool {
        match self {
            TerminalError::BufferCapacityExceeded { .. } => true,
            TerminalError::InvalidConfig { .. } => false,
            TerminalError::StreamProcessing { .. } => true,
            TerminalError::CommandExecution { .. } => true,
            TerminalError::Io { .. } => true,
            TerminalError::Channel { .. } => false,
            TerminalError::Rendering { .. } => true,
        }
    }

    /// Get the error category
    pub fn category(&self) -> ErrorCategory {
        match self {
            TerminalError::BufferCapacityExceeded { .. } => ErrorCategory::Buffer,
            TerminalError::InvalidConfig { .. } => ErrorCategory::Configuration,
            TerminalError::StreamProcessing { .. } => ErrorCategory::Stream,
            TerminalError::CommandExecution { .. } => ErrorCategory::Command,
            TerminalError::Io { .. } => ErrorCategory::Io,
            TerminalError::Channel { .. } => ErrorCategory::Communication,
            TerminalError::Rendering { .. } => ErrorCategory::Ui,
        }
    }
}

/// Categories of terminal errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    Buffer,
    Configuration,
    Stream,
    Command,
    Io,
    Communication,
    Ui,
}

impl From<std::io::Error> for TerminalError {
    fn from(err: std::io::Error) -> Self {
        TerminalError::io(err.to_string())
    }
}

impl<T> From<crossbeam_channel::SendError<T>> for TerminalError {
    fn from(err: crossbeam_channel::SendError<T>) -> Self {
        TerminalError::channel(format!("Send error: {}", err))
    }
}

impl From<crossbeam_channel::RecvError> for TerminalError {
    fn from(err: crossbeam_channel::RecvError) -> Self {
        TerminalError::channel(format!("Receive error: {}", err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_capacity_exceeded_error() {
        let error = TerminalError::buffer_capacity_exceeded(1000, 500);
        assert_eq!(
            error.to_string(),
            "Terminal buffer capacity exceeded: 1000 >= 500"
        );
        assert!(error.is_recoverable());
        assert_eq!(error.category(), ErrorCategory::Buffer);
    }

    #[test]
    fn test_invalid_config_error() {
        let error = TerminalError::invalid_config("Buffer size must be positive");
        assert_eq!(
            error.to_string(),
            "Invalid terminal configuration: Buffer size must be positive"
        );
        assert!(!error.is_recoverable());
        assert_eq!(error.category(), ErrorCategory::Configuration);
    }

    #[test]
    fn test_stream_processing_error() {
        let error = TerminalError::stream_processing("Failed to parse output");
        assert_eq!(
            error.to_string(),
            "Stream processing failed: Failed to parse output"
        );
        assert!(error.is_recoverable());
        assert_eq!(error.category(), ErrorCategory::Stream);
    }

    #[test]
    fn test_command_execution_error() {
        let error = TerminalError::command_execution("cargo build", "Permission denied");
        assert_eq!(
            error.to_string(),
            "Command execution failed: cargo build - Permission denied"
        );
        assert!(error.is_recoverable());
        assert_eq!(error.category(), ErrorCategory::Command);
    }

    #[test]
    fn test_io_error() {
        let error = TerminalError::io("File not found");
        assert_eq!(error.to_string(), "IO error: File not found");
        assert!(error.is_recoverable());
        assert_eq!(error.category(), ErrorCategory::Io);
    }

    #[test]
    fn test_channel_error() {
        let error = TerminalError::channel("Channel closed");
        assert_eq!(
            error.to_string(),
            "Channel communication error: Channel closed"
        );
        assert!(!error.is_recoverable());
        assert_eq!(error.category(), ErrorCategory::Communication);
    }

    #[test]
    fn test_rendering_error() {
        let error = TerminalError::rendering("Failed to render text");
        assert_eq!(error.to_string(), "Widget rendering error: Failed to render text");
        assert!(error.is_recoverable());
        assert_eq!(error.category(), ErrorCategory::Ui);
    }

    #[test]
    fn test_error_equality() {
        let error1 = TerminalError::invalid_config("test");
        let error2 = TerminalError::invalid_config("test");
        let error3 = TerminalError::invalid_config("different");

        assert_eq!(error1, error2);
        assert_ne!(error1, error3);
    }

    #[test]
    fn test_error_categories() {
        let categories = vec![
            ErrorCategory::Buffer,
            ErrorCategory::Configuration,
            ErrorCategory::Stream,
            ErrorCategory::Command,
            ErrorCategory::Io,
            ErrorCategory::Communication,
            ErrorCategory::Ui,
        ];

        // Ensure all categories are distinct
        for (i, cat1) in categories.iter().enumerate() {
            for (j, cat2) in categories.iter().enumerate() {
                if i == j {
                    assert_eq!(cat1, cat2);
                } else {
                    assert_ne!(cat1, cat2);
                }
            }
        }
    }

    #[test]
    fn test_from_io_error() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let terminal_error: TerminalError = io_error.into();
        
        assert!(matches!(terminal_error, TerminalError::Io { .. }));
        assert!(terminal_error.to_string().contains("File not found"));
    }

    #[test]
    fn test_from_send_error() {
        let (sender, _receiver) = crossbeam_channel::unbounded();
        drop(_receiver); // Close the receiver to cause a send error
        
        let send_result = sender.send("test");
        if let Err(send_error) = send_result {
            let terminal_error: TerminalError = send_error.into();
            assert!(matches!(terminal_error, TerminalError::Channel { .. }));
            assert!(terminal_error.to_string().contains("Send error"));
        }
    }

    #[test]
    fn test_from_recv_error() {
        let (_sender, receiver) = crossbeam_channel::unbounded::<String>();
        drop(_sender); // Close the sender to cause a recv error
        
        let recv_result = receiver.recv();
        if let Err(recv_error) = recv_result {
            let terminal_error: TerminalError = recv_error.into();
            assert!(matches!(terminal_error, TerminalError::Channel { .. }));
            assert!(terminal_error.to_string().contains("Receive error"));
        }
    }

    #[test]
    fn test_recoverable_classification() {
        let recoverable_errors = vec![
            TerminalError::buffer_capacity_exceeded(100, 50),
            TerminalError::stream_processing("test"),
            TerminalError::command_execution("test", "test"),
            TerminalError::io("test"),
            TerminalError::rendering("test"),
        ];

        let non_recoverable_errors = vec![
            TerminalError::invalid_config("test"),
            TerminalError::channel("test"),
        ];

        for error in recoverable_errors {
            assert!(error.is_recoverable(), "Error should be recoverable: {:?}", error);
        }

        for error in non_recoverable_errors {
            assert!(!error.is_recoverable(), "Error should not be recoverable: {:?}", error);
        }
    }
} 