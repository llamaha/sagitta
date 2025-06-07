use crate::{
    config::TerminalConfig,
    error::{Result, TerminalError},
    events::{LineType, StreamEvent},
};
use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use uuid::Uuid;

/// A single line in the terminal buffer
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLine {
    /// Unique identifier for this line
    pub id: Uuid,
    
    /// Content of the line
    pub content: String,
    
    /// Type of line (stdout, stderr, command, etc.)
    pub line_type: LineType,
    
    /// When this line was created
    pub timestamp: DateTime<Utc>,
    
    /// Optional command ID this line belongs to
    pub command_id: Option<Uuid>,
}

/// Buffer that manages terminal lines with automatic cleanup
#[derive(Debug, Clone)]
pub struct TerminalBuffer {
    /// Lines stored in the buffer
    lines: VecDeque<TerminalLine>,
    
    /// Configuration for the buffer
    config: TerminalConfig,
    
    /// Whether auto-scroll is currently enabled
    auto_scroll: bool,
    
    /// Last cleanup time
    last_cleanup: DateTime<Utc>,
}

impl TerminalLine {
    /// Create a new terminal line
    pub fn new(
        content: String,
        line_type: LineType,
        command_id: Option<Uuid>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            content,
            line_type,
            timestamp: Utc::now(),
            command_id,
        }
    }

    /// Create a terminal line from a stream event
    pub fn from_stream_event(event: &StreamEvent) -> Option<Self> {
        match event {
            StreamEvent::Output { command_id, line_type, content, timestamp } => {
                Some(Self {
                    id: Uuid::new_v4(),
                    content: content.clone(),
                    line_type: line_type.clone(),
                    timestamp: *timestamp,
                    command_id: *command_id,
                })
            },
            StreamEvent::CommandStarted(info) => {
                Some(Self {
                    id: Uuid::new_v4(),
                    content: format!("$ {}", info.command),
                    line_type: LineType::Command,
                    timestamp: info.started_at,
                    command_id: Some(info.id),
                })
            },
            StreamEvent::CommandFinished(info) => {
                let status = match info.exit_code {
                    Some(0) => "completed successfully".to_string(),
                    Some(code) => format!("exited with code {}", code),
                    None => "terminated".to_string(),
                };
                Some(Self {
                    id: Uuid::new_v4(),
                    content: format!("Command {} ({}ms)", status, info.duration.as_millis()),
                    line_type: LineType::System,
                    timestamp: info.finished_at,
                    command_id: Some(info.command_id),
                })
            },
            StreamEvent::Clear { .. } => None, // Clear events don't create lines
            StreamEvent::StreamError { message, timestamp } => {
                Some(Self {
                    id: Uuid::new_v4(),
                    content: format!("Error: {}", message),
                    line_type: LineType::Error,
                    timestamp: *timestamp,
                    command_id: None,
                })
            },
            StreamEvent::Stdout { content } => {
                Some(Self {
                    id: Uuid::new_v4(),
                    content: content.clone(),
                    line_type: LineType::StdOut,
                    timestamp: Utc::now(),
                    command_id: None,
                })
            },
            StreamEvent::Stderr { content } => {
                Some(Self {
                    id: Uuid::new_v4(),
                    content: content.clone(),
                    line_type: LineType::StdErr,
                    timestamp: Utc::now(),
                    command_id: None,
                })
            },
            StreamEvent::Exit { code } => {
                let status = if *code == 0 {
                    "completed successfully".to_string()
                } else {
                    format!("exited with code {}", code)
                };
                Some(Self {
                    id: Uuid::new_v4(),
                    content: format!("Command {}", status),
                    line_type: LineType::System,
                    timestamp: Utc::now(),
                    command_id: None,
                })
            },
            StreamEvent::ApprovalRequest { id, command, reason } => {
                Some(Self {
                    id: Uuid::new_v4(),
                    content: format!("⚠️  Approval required for '{}': {} (ID: {})", command, reason, id),
                    line_type: LineType::System,
                    timestamp: Utc::now(),
                    command_id: None,
                })
            },
            StreamEvent::MissingTool { tool, advice: _ } => {
                Some(Self {
                    id: Uuid::new_v4(),
                    content: format!("❌ Required tool '{}' is missing - installation required", tool),
                    line_type: LineType::Error,
                    timestamp: Utc::now(),
                    command_id: None,
                })
            },
        }
    }

    /// Check if this line matches a search query
    pub fn matches_search(&self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        
        self.content.to_lowercase().contains(&query.to_lowercase())
    }

    /// Get a display string for the line with optional timestamp
    pub fn display_string(&self, show_timestamp: bool) -> String {
        if show_timestamp {
            format!(
                "[{}] {}",
                self.timestamp.format("%H:%M:%S"),
                self.content
            )
        } else {
            self.content.clone()
        }
    }
}

impl TerminalBuffer {
    /// Create a new terminal buffer with the given configuration
    pub fn new(config: TerminalConfig) -> Result<Self> {
        config.validate()?;
        
        Ok(Self {
            lines: VecDeque::with_capacity(config.max_lines + 1000), // Extra capacity to avoid frequent reallocations
            auto_scroll: config.auto_scroll,
            config,
            last_cleanup: Utc::now(),
        })
    }

    /// Add a new line to the buffer
    pub fn add_line(&mut self, line: TerminalLine) -> Result<()> {
        // Check if we need to clean up first
        if self.lines.len() >= self.config.max_lines {
            self.cleanup_old_lines()?;
        }

        self.lines.push_back(line);
        self.maybe_cleanup()?;
        
        Ok(())
    }

    /// Add a stream event to the buffer
    pub fn add_event(&mut self, event: &StreamEvent) -> Result<()> {
        match event {
            StreamEvent::Clear { .. } => {
                self.clear();
                Ok(())
            },
            _ => {
                if let Some(line) = TerminalLine::from_stream_event(event) {
                    self.add_line(line)
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Add multiple events to the buffer
    pub fn add_events(&mut self, events: &[StreamEvent]) -> Result<()> {
        for event in events {
            self.add_event(event)?;
        }
        Ok(())
    }

    /// Get all lines in the buffer
    pub fn lines(&self) -> &VecDeque<TerminalLine> {
        &self.lines
    }

    /// Get lines that match a search query
    pub fn search_lines(&self, query: &str) -> Vec<&TerminalLine> {
        self.lines
            .iter()
            .filter(|line| line.matches_search(query))
            .collect()
    }

    /// Get lines for a specific command
    pub fn lines_for_command(&self, command_id: Uuid) -> Vec<&TerminalLine> {
        self.lines
            .iter()
            .filter(|line| line.command_id == Some(command_id))
            .collect()
    }

    /// Get the most recent lines (up to count)
    pub fn recent_lines(&self, count: usize) -> Vec<&TerminalLine> {
        self.lines
            .iter()
            .rev()
            .take(count)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    /// Clear all lines from the buffer
    pub fn clear(&mut self) {
        self.lines.clear();
    }

    /// Get the number of lines in the buffer
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Check if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Get/set auto-scroll setting
    pub fn auto_scroll(&self) -> bool {
        self.auto_scroll
    }

    pub fn set_auto_scroll(&mut self, auto_scroll: bool) {
        self.auto_scroll = auto_scroll;
    }

    /// Update the configuration
    pub fn update_config(&mut self, config: TerminalConfig) -> Result<()> {
        config.validate()?;
        
        // If max_lines decreased, we may need to cleanup
        if config.max_lines < self.config.max_lines && self.lines.len() > config.max_lines {
            self.config = config;
            self.cleanup_old_lines()?;
        } else {
            self.config = config;
        }
        
        Ok(())
    }

    /// Get the current configuration
    pub fn config(&self) -> &TerminalConfig {
        &self.config
    }

    /// Force cleanup of old lines
    pub fn cleanup_old_lines(&mut self) -> Result<()> {
        if self.lines.len() <= self.config.buffer.lines_to_keep {
            return Ok(());
        }

        let to_remove = self.lines.len() - self.config.buffer.lines_to_keep;
        
        if to_remove >= self.lines.len() {
            return Err(TerminalError::buffer_capacity_exceeded(
                to_remove,
                self.lines.len(),
            ));
        }

        for _ in 0..to_remove {
            self.lines.pop_front();
        }

        self.last_cleanup = Utc::now();
        Ok(())
    }

    /// Check if cleanup is needed based on time interval
    fn maybe_cleanup(&mut self) -> Result<()> {
        let now = Utc::now();
        let elapsed = (now - self.last_cleanup).num_milliseconds() as u64;
        
        if elapsed >= self.config.buffer.cleanup_interval_ms && self.lines.len() > self.config.buffer.lines_to_keep {
            self.cleanup_old_lines()?;
        }
        
        Ok(())
    }

    /// Get buffer statistics
    pub fn stats(&self) -> BufferStats {
        let mut stats = BufferStats::default();
        
        for line in &self.lines {
            match line.line_type {
                LineType::StdOut => stats.stdout_lines += 1,
                LineType::StdErr => stats.stderr_lines += 1,
                LineType::Command => stats.command_lines += 1,
                LineType::System => stats.system_lines += 1,
                LineType::Error => stats.error_lines += 1,
            }
        }
        
        stats.total_lines = self.lines.len();
        stats
    }
}

/// Statistics about the terminal buffer contents
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BufferStats {
    pub total_lines: usize,
    pub stdout_lines: usize,
    pub stderr_lines: usize,
    pub command_lines: usize,
    pub system_lines: usize,
    pub error_lines: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{CommandInfo, ExitInfo};
    use std::time::Duration;

    fn create_test_config() -> TerminalConfig {
        TerminalConfig::new()
            .with_max_lines(10)
            .unwrap()
            .with_auto_scroll(true)
            .with_buffer_config(crate::config::BufferConfig {
                max_lines: 10,
                lines_to_keep: 5, // Must be less than max_lines
                cleanup_interval_ms: 1000,
            })
            .unwrap()
    }

    #[test]
    fn test_terminal_line_new() {
        let line = TerminalLine::new(
            "test content".to_string(),
            LineType::StdOut,
            None,
        );

        assert_eq!(line.content, "test content");
        assert_eq!(line.line_type, LineType::StdOut);
        assert_eq!(line.command_id, None);
        
        // Timestamp should be recent
        let now = Utc::now();
        let diff = (now - line.timestamp).num_milliseconds().abs();
        assert!(diff < 1000);
    }

    #[test]
    fn test_terminal_line_from_stream_event() {
        // Test stdout event
        let event = StreamEvent::stdout(Some(Uuid::new_v4()), "output".to_string());
        let line = TerminalLine::from_stream_event(&event).unwrap();
        assert_eq!(line.content, "output");
        assert_eq!(line.line_type, LineType::StdOut);

        // Test command started event
        let cmd_info = CommandInfo {
            id: Uuid::new_v4(),
            command: "ls -la".to_string(),
            working_dir: None,
            started_at: Utc::now(),
        };
        let event = StreamEvent::CommandStarted(cmd_info.clone());
        let line = TerminalLine::from_stream_event(&event).unwrap();
        assert_eq!(line.content, "$ ls -la");
        assert_eq!(line.line_type, LineType::Command);
        assert_eq!(line.command_id, Some(cmd_info.id));

        // Test command finished event
        let exit_info = ExitInfo {
            command_id: Uuid::new_v4(),
            exit_code: Some(0),
            duration: Duration::from_millis(1500),
            finished_at: Utc::now(),
        };
        let event = StreamEvent::CommandFinished(exit_info.clone());
        let line = TerminalLine::from_stream_event(&event).unwrap();
        assert!(line.content.contains("completed successfully"));
        assert!(line.content.contains("1500ms"));
        assert_eq!(line.line_type, LineType::System);

        // Test clear event (should return None)
        let event = StreamEvent::Clear { timestamp: Utc::now() };
        let line = TerminalLine::from_stream_event(&event);
        assert!(line.is_none());
    }

    #[test]
    fn test_terminal_line_matches_search() {
        let line = TerminalLine::new(
            "Hello World Test".to_string(),
            LineType::StdOut,
            None,
        );

        assert!(line.matches_search(""));
        assert!(line.matches_search("hello"));
        assert!(line.matches_search("HELLO"));
        assert!(line.matches_search("world"));
        assert!(line.matches_search("test"));
        assert!(line.matches_search("Hello World"));
        assert!(!line.matches_search("notfound"));
    }

    #[test]
    fn test_terminal_line_display_string() {
        let line = TerminalLine::new(
            "test content".to_string(),
            LineType::StdOut,
            None,
        );

        // Without timestamp
        assert_eq!(line.display_string(false), "test content");

        // With timestamp
        let with_timestamp = line.display_string(true);
        assert!(with_timestamp.contains("test content"));
        assert!(with_timestamp.contains("["));
        assert!(with_timestamp.contains("]"));
    }

    #[test]
    fn test_terminal_buffer_new() {
        let config = create_test_config();
        let buffer = TerminalBuffer::new(config.clone()).unwrap();
        
        assert_eq!(buffer.line_count(), 0);
        assert!(buffer.is_empty());
        assert_eq!(buffer.config().max_lines, 10);
        assert!(buffer.auto_scroll());
    }

    #[test]
    fn test_terminal_buffer_add_line() {
        let config = create_test_config();
        let mut buffer = TerminalBuffer::new(config).unwrap();

        let line = TerminalLine::new(
            "test line".to_string(),
            LineType::StdOut,
            None,
        );

        buffer.add_line(line.clone()).unwrap();
        assert_eq!(buffer.line_count(), 1);
        assert!(!buffer.is_empty());
        assert_eq!(buffer.lines().back().unwrap().content, "test line");
    }

    #[test]
    fn test_terminal_buffer_add_event() {
        let config = create_test_config();
        let mut buffer = TerminalBuffer::new(config).unwrap();

        // Add stdout event
        let event = StreamEvent::stdout(None, "stdout content".to_string());
        buffer.add_event(&event).unwrap();
        assert_eq!(buffer.line_count(), 1);

        // Add clear event
        let event = StreamEvent::Clear { timestamp: Utc::now() };
        buffer.add_event(&event).unwrap();
        assert_eq!(buffer.line_count(), 0);
    }

    #[test]
    fn test_terminal_buffer_add_events() {
        let config = create_test_config();
        let mut buffer = TerminalBuffer::new(config).unwrap();

        let events = vec![
            StreamEvent::stdout(None, "line 1".to_string()),
            StreamEvent::stderr(None, "line 2".to_string()),
            StreamEvent::system("line 3".to_string()),
        ];

        buffer.add_events(&events).unwrap();
        assert_eq!(buffer.line_count(), 3);
    }

    #[test]
    fn test_terminal_buffer_search_lines() {
        let config = create_test_config();
        let mut buffer = TerminalBuffer::new(config).unwrap();

        let events = vec![
            StreamEvent::stdout(None, "hello world".to_string()),
            StreamEvent::stderr(None, "error occurred".to_string()),
            StreamEvent::stdout(None, "hello again".to_string()),
        ];

        buffer.add_events(&events).unwrap();

        let results = buffer.search_lines("hello");
        assert_eq!(results.len(), 2);

        let results = buffer.search_lines("error");
        assert_eq!(results.len(), 1);

        let results = buffer.search_lines("notfound");
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_terminal_buffer_lines_for_command() {
        let config = create_test_config();
        let mut buffer = TerminalBuffer::new(config).unwrap();

        let command_id = Uuid::new_v4();
        let events = vec![
            StreamEvent::stdout(Some(command_id), "output 1".to_string()),
            StreamEvent::stdout(None, "other output".to_string()),
            StreamEvent::stderr(Some(command_id), "error 1".to_string()),
        ];

        buffer.add_events(&events).unwrap();

        let results = buffer.lines_for_command(command_id);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_terminal_buffer_recent_lines() {
        let config = create_test_config();
        let mut buffer = TerminalBuffer::new(config).unwrap();

        let events = vec![
            StreamEvent::stdout(None, "line 1".to_string()),
            StreamEvent::stdout(None, "line 2".to_string()),
            StreamEvent::stdout(None, "line 3".to_string()),
            StreamEvent::stdout(None, "line 4".to_string()),
        ];

        buffer.add_events(&events).unwrap();

        let recent = buffer.recent_lines(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].content, "line 3");
        assert_eq!(recent[1].content, "line 4");

        let all_recent = buffer.recent_lines(10);
        assert_eq!(all_recent.len(), 4);
    }

    #[test]
    fn test_terminal_buffer_clear() {
        let config = create_test_config();
        let mut buffer = TerminalBuffer::new(config).unwrap();

        buffer.add_event(&StreamEvent::stdout(None, "test".to_string())).unwrap();
        assert_eq!(buffer.line_count(), 1);

        buffer.clear();
        assert_eq!(buffer.line_count(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_terminal_buffer_auto_scroll() {
        let config = create_test_config();
        let mut buffer = TerminalBuffer::new(config).unwrap();

        assert!(buffer.auto_scroll());
        
        buffer.set_auto_scroll(false);
        assert!(!buffer.auto_scroll());
    }

    #[test]
    fn test_terminal_buffer_cleanup() {
        let config = TerminalConfig::new()
            .with_max_lines(5)
            .unwrap()
            .with_buffer_config(crate::config::BufferConfig {
                max_lines: 5,
                lines_to_keep: 3,
                cleanup_interval_ms: 1,
            })
            .unwrap();

        let mut buffer = TerminalBuffer::new(config).unwrap();

        // Add more lines than max_lines
        for i in 0..7 {
            let event = StreamEvent::stdout(None, format!("line {}", i));
            buffer.add_event(&event).unwrap();
        }

        // Should have triggered cleanup
        assert!(buffer.line_count() <= 5);
    }

    #[test]
    fn test_terminal_buffer_update_config() {
        let config = create_test_config();
        let mut buffer = TerminalBuffer::new(config).unwrap();

        // Add some lines
        for i in 0..5 {
            let event = StreamEvent::stdout(None, format!("line {}", i));
            buffer.add_event(&event).unwrap();
        }

        // Update with smaller max_lines
        let new_config = TerminalConfig::new().with_max_lines(3).unwrap();
        buffer.update_config(new_config).unwrap();

        assert!(buffer.line_count() <= 3);
    }

    #[test]
    fn test_terminal_buffer_stats() {
        let config = create_test_config();
        let mut buffer = TerminalBuffer::new(config).unwrap();

        let events = vec![
            StreamEvent::stdout(None, "stdout 1".to_string()),
            StreamEvent::stdout(None, "stdout 2".to_string()),
            StreamEvent::stderr(None, "stderr 1".to_string()),
            StreamEvent::command("command 1".to_string()),
            StreamEvent::system("system 1".to_string()),
        ];

        buffer.add_events(&events).unwrap();

        let stats = buffer.stats();
        assert_eq!(stats.total_lines, 5);
        assert_eq!(stats.stdout_lines, 2);
        assert_eq!(stats.stderr_lines, 1);
        assert_eq!(stats.command_lines, 1);
        assert_eq!(stats.system_lines, 1);
        assert_eq!(stats.error_lines, 0);
    }

    #[test]
    fn test_buffer_stats_default() {
        let stats = BufferStats::default();
        assert_eq!(stats.total_lines, 0);
        assert_eq!(stats.stdout_lines, 0);
        assert_eq!(stats.stderr_lines, 0);
        assert_eq!(stats.command_lines, 0);
        assert_eq!(stats.system_lines, 0);
        assert_eq!(stats.error_lines, 0);
    }

    #[test]
    fn test_terminal_buffer_capacity_exceeded_error() {
        // Test that creating a buffer with invalid configuration fails
        let invalid_config = TerminalConfig::new()
            .with_max_lines(2)
            .unwrap()
            .with_buffer_config(crate::config::BufferConfig {
                max_lines: 2,
                lines_to_keep: 10, // Invalid: keep more than max
                cleanup_interval_ms: 1,
            });

        // This should fail due to validation
        assert!(invalid_config.is_err());
        
        // Test that creating a buffer with valid config succeeds
        let valid_config = TerminalConfig::new()
            .with_max_lines(10)
            .unwrap()
            .with_buffer_config(crate::config::BufferConfig {
                max_lines: 10,
                lines_to_keep: 5, // Valid: keep less than max
                cleanup_interval_ms: 1,
            })
            .unwrap();

        let buffer = TerminalBuffer::new(valid_config);
        assert!(buffer.is_ok());
    }
} 