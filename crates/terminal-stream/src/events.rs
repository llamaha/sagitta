use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

/// Types of output lines in the terminal
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LineType {
    /// Standard output from a command
    StdOut,
    /// Standard error from a command  
    StdErr,
    /// Command that was executed
    Command,
    /// System message (e.g., command started/finished)
    System,
    /// Error message from the terminal system
    Error,
}

/// Information about a command execution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandInfo {
    pub id: Uuid,
    pub command: String,
    pub working_dir: Option<String>,
    pub started_at: DateTime<Utc>,
}

/// Information about command completion
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExitInfo {
    pub command_id: Uuid,
    pub exit_code: Option<i32>,
    pub duration: Duration,
    pub finished_at: DateTime<Utc>,
}

/// Events that can occur during terminal streaming
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StreamEvent {
    /// A line of output was received
    Output {
        command_id: Option<Uuid>,
        line_type: LineType,
        content: String,
        timestamp: DateTime<Utc>,
    },
    /// A command started executing
    CommandStarted(CommandInfo),
    /// A command finished executing
    CommandFinished(ExitInfo),
    /// The terminal was cleared
    Clear {
        timestamp: DateTime<Utc>,
    },
    /// An error occurred in the streaming system
    StreamError {
        message: String,
        timestamp: DateTime<Utc>,
    },
    /// Standard output from a command (simplified)
    Stdout {
        content: String,
    },
    /// Standard error from a command (simplified)
    Stderr {
        content: String,
    },
    /// Command exit with code
    Exit {
        code: i32,
    },
    /// Request for user approval of a command
    ApprovalRequest {
        id: String,
        command: String,
        reason: String,
    },
    /// A required tool is missing and installation advice is provided
    MissingTool {
        tool: String,
        advice: serde_json::Value,
    },
    /// Progress update for long-running operations
    Progress {
        message: String,
        percentage: Option<f32>,
    },
}

impl StreamEvent {
    /// Create a new stdout output event
    pub fn stdout(command_id: Option<Uuid>, content: String) -> Self {
        Self::Output {
            command_id,
            line_type: LineType::StdOut,
            content,
            timestamp: Utc::now(),
        }
    }

    /// Create a new stderr output event
    pub fn stderr(command_id: Option<Uuid>, content: String) -> Self {
        Self::Output {
            command_id,
            line_type: LineType::StdErr,
            content,
            timestamp: Utc::now(),
        }
    }

    /// Create a new command event
    pub fn command(content: String) -> Self {
        Self::Output {
            command_id: None,
            line_type: LineType::Command,
            content,
            timestamp: Utc::now(),
        }
    }

    /// Create a new system message event
    pub fn system(content: String) -> Self {
        Self::Output {
            command_id: None,
            line_type: LineType::System,
            content,
            timestamp: Utc::now(),
        }
    }

    /// Create a tool started event (convenience method)
    pub fn tool_started(tool: &str, run_id: Uuid) -> Self {
        Self::system(format!("🔧 {} started ({})", tool, run_id))
    }

    /// Create a tool completed event (convenience method)
    pub fn tool_completed(tool: &str, run_id: Uuid, success: bool) -> Self {
        let status = if success { "✅ completed" } else { "❌ failed" };
        Self::system(format!("{} {} ({})", tool, status, run_id))
    }

    /// Get the timestamp of this event
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            StreamEvent::Output { timestamp, .. } => *timestamp,
            StreamEvent::CommandStarted(info) => info.started_at,
            StreamEvent::CommandFinished(info) => info.finished_at,
            StreamEvent::Clear { timestamp } => *timestamp,
            StreamEvent::StreamError { timestamp, .. } => *timestamp,
            StreamEvent::Stdout { .. } => Utc::now(),
            StreamEvent::Stderr { .. } => Utc::now(),
            StreamEvent::Exit { .. } => Utc::now(),
            StreamEvent::ApprovalRequest { .. } => Utc::now(),
            StreamEvent::MissingTool { .. } => Utc::now(),
            StreamEvent::Progress { .. } => Utc::now(),
        }
    }

    /// Check if this is an output event
    pub fn is_output(&self) -> bool {
        matches!(self, StreamEvent::Output { .. } | StreamEvent::Stdout { .. } | StreamEvent::Stderr { .. } | StreamEvent::Exit { .. } | StreamEvent::ApprovalRequest { .. } | StreamEvent::MissingTool { .. } | StreamEvent::Progress { .. })
    }

    /// Check if this is an error event (either stderr output or stream error)
    pub fn is_error(&self) -> bool {
        match self {
            StreamEvent::Output { line_type: LineType::StdErr, .. } => true,
            StreamEvent::Output { line_type: LineType::Error, .. } => true,
            StreamEvent::StreamError { .. } => true,
            StreamEvent::Stderr { .. } => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_line_type_serialization() {
        // Test that LineType can be serialized and deserialized
        let line_types = vec![
            LineType::StdOut,
            LineType::StdErr,
            LineType::Command,
            LineType::System,
            LineType::Error,
        ];

        for line_type in line_types {
            let json = serde_json::to_string(&line_type).unwrap();
            let deserialized: LineType = serde_json::from_str(&json).unwrap();
            assert_eq!(line_type, deserialized);
        }
    }

    #[test]
    fn test_command_info_creation() {
        let id = Uuid::new_v4();
        let started_at = Utc::now();
        
        let cmd_info = CommandInfo {
            id,
            command: "ls -la".to_string(),
            working_dir: Some("/home/user".to_string()),
            started_at,
        };

        assert_eq!(cmd_info.id, id);
        assert_eq!(cmd_info.command, "ls -la");
        assert_eq!(cmd_info.working_dir, Some("/home/user".to_string()));
        assert_eq!(cmd_info.started_at, started_at);
    }

    #[test]
    fn test_exit_info_creation() {
        let command_id = Uuid::new_v4();
        let finished_at = Utc::now();
        let duration = Duration::from_secs(5);

        let exit_info = ExitInfo {
            command_id,
            exit_code: Some(0),
            duration,
            finished_at,
        };

        assert_eq!(exit_info.command_id, command_id);
        assert_eq!(exit_info.exit_code, Some(0));
        assert_eq!(exit_info.duration, duration);
        assert_eq!(exit_info.finished_at, finished_at);
    }

    #[test]
    fn test_stream_event_stdout_creation() {
        let command_id = Uuid::new_v4();
        let content = "Hello, world!".to_string();
        let event = StreamEvent::stdout(Some(command_id), content.clone());

        if let StreamEvent::Output { command_id: cmd_id, line_type, content: event_content, .. } = event {
            assert_eq!(cmd_id, Some(command_id));
            assert_eq!(line_type, LineType::StdOut);
            assert_eq!(event_content, content);
        } else {
            panic!("Expected Output event");
        }
    }

    #[test]
    fn test_stream_event_stderr_creation() {
        let command_id = Uuid::new_v4();
        let content = "Error occurred".to_string();
        let event = StreamEvent::stderr(Some(command_id), content.clone());

        if let StreamEvent::Output { command_id: cmd_id, line_type, content: event_content, .. } = event {
            assert_eq!(cmd_id, Some(command_id));
            assert_eq!(line_type, LineType::StdErr);
            assert_eq!(event_content, content);
        } else {
            panic!("Expected Output event");
        }
    }

    #[test]
    fn test_stream_event_command_creation() {
        let content = "cargo build".to_string();
        let event = StreamEvent::command(content.clone());

        if let StreamEvent::Output { command_id, line_type, content: event_content, .. } = event {
            assert_eq!(command_id, None);
            assert_eq!(line_type, LineType::Command);
            assert_eq!(event_content, content);
        } else {
            panic!("Expected Output event");
        }
    }

    #[test]
    fn test_stream_event_system_creation() {
        let content = "Command started".to_string();
        let event = StreamEvent::system(content.clone());

        if let StreamEvent::Output { command_id, line_type, content: event_content, .. } = event {
            assert_eq!(command_id, None);
            assert_eq!(line_type, LineType::System);
            assert_eq!(event_content, content);
        } else {
            panic!("Expected Output event");
        }
    }

    #[test]
    fn test_stream_event_timestamp() {
        let now = Utc::now();
        let event = StreamEvent::stdout(None, "test".to_string());
        let event_timestamp = event.timestamp();
        
        // Should be within 1 second of creation
        let diff = (event_timestamp - now).num_milliseconds().abs();
        assert!(diff < 1000, "Timestamp should be close to creation time");
    }

    #[test]
    fn test_stream_event_is_output() {
        let output_event = StreamEvent::stdout(None, "test".to_string());
        let command_started = StreamEvent::CommandStarted(CommandInfo {
            id: Uuid::new_v4(),
            command: "test".to_string(),
            working_dir: None,
            started_at: Utc::now(),
        });

        assert!(output_event.is_output());
        assert!(!command_started.is_output());
    }

    #[test]
    fn test_stream_event_is_error() {
        let stdout_event = StreamEvent::stdout(None, "normal output".to_string());
        let stderr_event = StreamEvent::stderr(None, "error output".to_string());
        let stream_error = StreamEvent::StreamError {
            message: "Stream failed".to_string(),
            timestamp: Utc::now(),
        };
        let error_event = StreamEvent::Output {
            command_id: None,
            line_type: LineType::Error,
            content: "error".to_string(),
            timestamp: Utc::now(),
        };

        assert!(!stdout_event.is_error());
        assert!(stderr_event.is_error());
        assert!(stream_error.is_error());
        assert!(error_event.is_error());
    }

    #[test]
    fn test_stream_event_clear() {
        let clear_event = StreamEvent::Clear {
            timestamp: Utc::now(),
        };

        assert!(!clear_event.is_output());
        assert!(!clear_event.is_error());
    }

    #[test]
    fn test_stream_event_serialization() {
        let events = vec![
            StreamEvent::stdout(Some(Uuid::new_v4()), "test output".to_string()),
            StreamEvent::stderr(None, "error".to_string()),
            StreamEvent::CommandStarted(CommandInfo {
                id: Uuid::new_v4(),
                command: "ls".to_string(),
                working_dir: Some("/tmp".to_string()),
                started_at: Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(),
            }),
            StreamEvent::Clear { timestamp: Utc::now() },
        ];

        for event in events {
            let json = serde_json::to_string(&event).unwrap();
            let deserialized: StreamEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(event, deserialized);
        }
    }

    #[test]
    fn test_command_finished_duration() {
        let command_id = Uuid::new_v4();
        let duration = Duration::from_millis(1500);
        
        let exit_info = ExitInfo {
            command_id,
            exit_code: Some(0),
            duration,
            finished_at: Utc::now(),
        };

        assert_eq!(exit_info.duration.as_millis(), 1500);
    }
} 