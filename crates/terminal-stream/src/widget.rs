use crate::{
    buffer::{TerminalBuffer, TerminalLine},
    config::{TerminalConfig, TerminalColors},
    error::{Result, TerminalError},
    events::{LineType, StreamEvent},
};
use crossbeam_channel::{Receiver, Sender};
use egui::{
    Color32, Context, FontFamily, FontId, Id, Response, Sense, TextFormat, TextStyle, Ui,
    Vec2, Widget,
};
use log::error;
use std::collections::HashMap;

/// Terminal widget that displays streaming command output
pub struct TerminalWidget {
    /// Unique identifier for this widget
    id: Id,
    
    /// Terminal buffer containing the lines
    buffer: TerminalBuffer,
    
    /// Widget configuration
    config: TerminalConfig,
    
    /// Scroll position (None means auto-scroll to bottom)
    scroll_position: Option<f32>,
    
    /// Search query for filtering lines
    search_query: String,
    
    /// Whether the widget has focus
    has_focus: bool,
    
    /// Channel receiver for incoming events (optional)
    event_receiver: Option<Receiver<StreamEvent>>,
    
    /// Cached text formatting for performance
    text_formats: HashMap<LineType, TextFormat>,
}

/// Builder for creating terminal widgets
pub struct TerminalWidgetBuilder {
    id: Option<Id>,
    config: Option<TerminalConfig>,
    event_receiver: Option<Receiver<StreamEvent>>,
}

impl TerminalWidget {
    /// Create a new terminal widget with default configuration
    pub fn new(id: impl Into<Id>) -> Self {
        let config = TerminalConfig::default();
        let buffer = TerminalBuffer::new(config.clone()).expect("Default config should be valid");
        
        Self {
            id: id.into(),
            buffer,
            config: config.clone(),
            scroll_position: None,
            search_query: String::new(),
            has_focus: false,
            event_receiver: None,
            text_formats: Self::create_text_formats(&config.colors),
        }
    }

    /// Create a builder for configuring the terminal widget
    pub fn builder() -> TerminalWidgetBuilder {
        TerminalWidgetBuilder::new()
    }

    /// Update the widget configuration
    pub fn update_config(&mut self, config: TerminalConfig) -> Result<()> {
        self.buffer.update_config(config.clone())?;
        self.text_formats = Self::create_text_formats(&config.colors);
        self.config = config;
        Ok(())
    }

    /// Set the event receiver for streaming updates
    pub fn set_event_receiver(&mut self, receiver: Receiver<StreamEvent>) {
        self.event_receiver = Some(receiver);
    }

    /// Add an event to the terminal
    pub fn add_event(&mut self, event: &StreamEvent) -> Result<()> {
        self.buffer.add_event(event)
    }

    /// Add multiple events to the terminal
    pub fn add_events(&mut self, events: &[StreamEvent]) -> Result<()> {
        self.buffer.add_events(events)
    }

    /// Add text output to the terminal (convenience method)
    pub fn add_output(&mut self, text: &str) -> Result<()> {
        let event = StreamEvent::stdout(None, text.to_string());
        self.buffer.add_event(&event)
    }

    /// Clear the terminal
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Get the current search query
    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    /// Set the search query
    pub fn set_search_query(&mut self, query: String) {
        self.search_query = query;
    }

    /// Get whether auto-scroll is enabled
    pub fn auto_scroll(&self) -> bool {
        self.buffer.auto_scroll()
    }

    /// Set auto-scroll behavior
    pub fn set_auto_scroll(&mut self, auto_scroll: bool) {
        self.buffer.set_auto_scroll(auto_scroll);
        if auto_scroll {
            self.scroll_position = None; // Reset to bottom
        }
    }

    /// Get the number of lines in the buffer
    pub fn line_count(&self) -> usize {
        self.buffer.line_count()
    }

    /// Get buffer statistics
    pub fn stats(&self) -> crate::buffer::BufferStats {
        self.buffer.stats()
    }

    /// Process any pending events from the receiver
    fn process_pending_events(&mut self) -> Result<bool> {
        let mut processed_any = false;
        
        if let Some(receiver) = &self.event_receiver {
            // Process all available events without blocking
            while let Ok(event) = receiver.try_recv() {
                self.buffer.add_event(&event)?;
                processed_any = true;
            }
        }
        
        Ok(processed_any)
    }

    /// Create text formats for different line types
    fn create_text_formats(colors: &TerminalColors) -> HashMap<LineType, TextFormat> {
        let mut formats = HashMap::new();
        
        formats.insert(LineType::StdOut, TextFormat {
            font_id: FontId::monospace(12.0),
            color: colors.stdout,
            ..Default::default()
        });
        
        formats.insert(LineType::StdErr, TextFormat {
            font_id: FontId::monospace(12.0),
            color: colors.stderr,
            ..Default::default()
        });
        
        formats.insert(LineType::Command, TextFormat {
            font_id: FontId::monospace(12.0),
            color: colors.command,
            ..Default::default()
        });
        
        formats.insert(LineType::System, TextFormat {
            font_id: FontId::monospace(12.0),
            color: colors.system,
            ..Default::default()
        });
        
        formats.insert(LineType::Error, TextFormat {
            font_id: FontId::monospace(12.0),
            color: colors.error,
            ..Default::default()
        });
        
        formats
    }

    /// Get the filtered lines based on search query
    fn get_filtered_lines(&self) -> Vec<&TerminalLine> {
        if self.search_query.is_empty() {
            self.buffer.lines().iter().collect()
        } else {
            self.buffer.search_lines(&self.search_query)
        }
    }

    /// Render the search bar
    fn render_search_bar(&mut self, ui: &mut Ui) -> bool {
        let mut search_changed = false;
        
        ui.horizontal(|ui| {
            ui.label("Search:");
            let response = ui.text_edit_singleline(&mut self.search_query);
            if response.changed() {
                search_changed = true;
            }
            
            if ui.button("Clear").clicked() {
                self.search_query.clear();
                search_changed = true;
            }
        });
        
        search_changed
    }

    /// Render the control bar (auto-scroll, clear, etc.)
    fn render_controls(&mut self, ui: &mut Ui) -> (bool, bool) {
        let mut cleared = false;
        let mut auto_scroll_toggled = false;
        
        ui.horizontal(|ui| {
            if ui.button("Clear").clicked() {
                self.clear();
                cleared = true;
            }
            
            ui.separator();
            
            let mut auto_scroll = self.auto_scroll();
            if ui.checkbox(&mut auto_scroll, "Auto-scroll").changed() {
                self.set_auto_scroll(auto_scroll);
                auto_scroll_toggled = true;
            }
            
            ui.separator();
            
            let stats = self.stats();
            ui.label(format!("Lines: {}/{}", stats.total_lines, self.config.max_lines));
            
            if stats.stderr_lines > 0 || stats.error_lines > 0 {
                ui.colored_label(self.config.colors.error, format!("Errors: {}", stats.stderr_lines + stats.error_lines));
            }
        });
        
        (cleared, auto_scroll_toggled)
    }

    /// Render a single terminal line
    fn render_line(&self, ui: &mut Ui, line: &TerminalLine) {
        let text_format = self.text_formats
            .get(&line.line_type)
            .cloned()
            .unwrap_or_default();
        
        ui.label(
            egui::RichText::new(line.content.clone())
                .font(text_format.font_id)
                .color(text_format.color)
        );
    }

    /// Render the terminal widget
    pub fn show(&mut self, ui: &mut Ui) -> Response {
        // Process any pending events first
        if let Err(e) = self.process_pending_events() {
            error!("Failed to process terminal events: {}", e);
        }
        
        let mut cleared = false;
        let mut auto_scroll_toggled = false;
        let mut search_changed = false;
        
        // Main terminal frame
        let frame = egui::Frame::none()
            .fill(self.config.colors.background)
            .inner_margin(4.0);
        
        let response = frame.show(ui, |ui| {
            ui.vertical(|ui| {
                // Search bar
                search_changed = self.render_search_bar(ui);
                
                // Control bar
                let (controls_cleared, controls_auto_scroll_toggled) = self.render_controls(ui);
                cleared = controls_cleared;
                auto_scroll_toggled = controls_auto_scroll_toggled;
                
                ui.separator();
                
                // Terminal content area
                let scroll_area = egui::ScrollArea::vertical()
                    .id_source(self.id)
                    .auto_shrink([false, false])
                    .stick_to_bottom(self.auto_scroll());
                
                scroll_area.show(ui, |ui| {
                    let filtered_lines = self.get_filtered_lines();
                    
                    for line in &filtered_lines {
                        self.render_line(ui, line);
                    }
                    
                    // Add some spacing at the bottom
                    ui.add_space(10.0);
                });
            });
        });
        
        response.response
    }
}

impl TerminalWidgetBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            id: None,
            config: None,
            event_receiver: None,
        }
    }

    /// Set the widget ID
    pub fn id(mut self, id: impl Into<Id>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the widget configuration
    pub fn config(mut self, config: TerminalConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Set the event receiver
    pub fn event_receiver(mut self, receiver: Receiver<StreamEvent>) -> Self {
        self.event_receiver = Some(receiver);
        self
    }

    /// Build the terminal widget
    pub fn build(self) -> Result<TerminalWidget> {
        let id = self.id.unwrap_or_else(|| Id::new("terminal_widget"));
        let config = self.config.unwrap_or_default();
        let buffer = TerminalBuffer::new(config.clone())?;
        
        let mut widget = TerminalWidget {
            id,
            buffer,
            config: config.clone(),
            scroll_position: None,
            search_query: String::new(),
            has_focus: false,
            event_receiver: None,
            text_formats: TerminalWidget::create_text_formats(&config.colors),
        };
        
        if let Some(receiver) = self.event_receiver {
            widget.set_event_receiver(receiver);
        }
        
        Ok(widget)
    }
}

impl Default for TerminalWidgetBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{CommandInfo, ExitInfo};
    use chrono::Utc;
    use crossbeam_channel::unbounded;
    use std::time::Duration;
    use uuid::Uuid;

    #[test]
    fn test_terminal_widget_new() {
        let widget = TerminalWidget::new("test_terminal");
        assert_eq!(widget.line_count(), 0);
        assert!(widget.auto_scroll());
        assert_eq!(widget.search_query(), "");
    }

    #[test]
    fn test_terminal_widget_builder() {
        let config = TerminalConfig::new().with_max_lines(100).unwrap();
        let (sender, receiver) = unbounded();
        
        let widget = TerminalWidget::builder()
            .id("test_id")
            .config(config.clone())
            .event_receiver(receiver)
            .build()
            .unwrap();
        
        assert_eq!(widget.config.max_lines, 100);
        assert!(widget.event_receiver.is_some());
    }

    #[test]
    fn test_terminal_widget_add_event() {
        let mut widget = TerminalWidget::new("test");
        
        let event = StreamEvent::stdout(None, "test output".to_string());
        widget.add_event(&event).unwrap();
        
        assert_eq!(widget.line_count(), 1);
    }

    #[test]
    fn test_terminal_widget_add_events() {
        let mut widget = TerminalWidget::new("test");
        
        let events = vec![
            StreamEvent::stdout(None, "line 1".to_string()),
            StreamEvent::stderr(None, "line 2".to_string()),
            StreamEvent::command("test command".to_string()),
        ];
        
        widget.add_events(&events).unwrap();
        assert_eq!(widget.line_count(), 3);
    }

    #[test]
    fn test_terminal_widget_add_output() {
        let mut widget = TerminalWidget::new("test");
        
        widget.add_output("Hello, world!").unwrap();
        widget.add_output("Second line").unwrap();
        
        assert_eq!(widget.line_count(), 2);
        
        let lines = widget.buffer.lines();
        assert_eq!(lines[0].content, "Hello, world!");
        assert_eq!(lines[1].content, "Second line");
        assert_eq!(lines[0].line_type, LineType::StdOut);
        assert_eq!(lines[1].line_type, LineType::StdOut);
    }

    #[test]
    fn test_terminal_widget_clear() {
        let mut widget = TerminalWidget::new("test");
        
        widget.add_event(&StreamEvent::stdout(None, "test".to_string())).unwrap();
        assert_eq!(widget.line_count(), 1);
        
        widget.clear();
        assert_eq!(widget.line_count(), 0);
    }

    #[test]
    fn test_terminal_widget_search() {
        let mut widget = TerminalWidget::new("test");
        
        let events = vec![
            StreamEvent::stdout(None, "hello world".to_string()),
            StreamEvent::stderr(None, "error occurred".to_string()),
            StreamEvent::stdout(None, "hello again".to_string()),
        ];
        
        widget.add_events(&events).unwrap();
        
        // Test search functionality
        widget.set_search_query("hello".to_string());
        assert_eq!(widget.search_query(), "hello");
        
        let filtered = widget.get_filtered_lines();
        assert_eq!(filtered.len(), 2);
        
        // Test empty search
        widget.set_search_query("".to_string());
        let filtered = widget.get_filtered_lines();
        assert_eq!(filtered.len(), 3);
    }

    #[test]
    fn test_terminal_widget_auto_scroll() {
        let mut widget = TerminalWidget::new("test");
        
        assert!(widget.auto_scroll());
        
        widget.set_auto_scroll(false);
        assert!(!widget.auto_scroll());
        
        widget.set_auto_scroll(true);
        assert!(widget.auto_scroll());
    }

    #[test]
    fn test_terminal_widget_update_config() {
        let mut widget = TerminalWidget::new("test");
        
        let new_config = TerminalConfig::new()
            .with_max_lines(500)
            .unwrap()
            .with_font_size(14.0)
            .unwrap();
        
        widget.update_config(new_config.clone()).unwrap();
        assert_eq!(widget.config.max_lines, 500);
        assert_eq!(widget.config.font_size, 14.0);
    }

    #[test]
    fn test_terminal_widget_stats() {
        let mut widget = TerminalWidget::new("test");
        
        let events = vec![
            StreamEvent::stdout(None, "stdout 1".to_string()),
            StreamEvent::stdout(None, "stdout 2".to_string()),
            StreamEvent::stderr(None, "stderr 1".to_string()),
            StreamEvent::command("command 1".to_string()),
        ];
        
        widget.add_events(&events).unwrap();
        
        let stats = widget.stats();
        assert_eq!(stats.total_lines, 4);
        assert_eq!(stats.stdout_lines, 2);
        assert_eq!(stats.stderr_lines, 1);
        assert_eq!(stats.command_lines, 1);
    }

    #[test]
    fn test_terminal_widget_process_events_from_receiver() {
        let mut widget = TerminalWidget::new("test");
        let (sender, receiver) = unbounded();
        
        widget.set_event_receiver(receiver);
        
        // Send some events
        sender.send(StreamEvent::stdout(None, "received 1".to_string())).unwrap();
        sender.send(StreamEvent::stderr(None, "received 2".to_string())).unwrap();
        
        // Process events
        let processed = widget.process_pending_events().unwrap();
        assert!(processed);
        assert_eq!(widget.line_count(), 2);
    }

    #[test]
    fn test_terminal_widget_text_formats() {
        let colors = crate::config::TerminalColors::default();
        let formats = TerminalWidget::create_text_formats(&colors);
        
        assert!(formats.contains_key(&LineType::StdOut));
        assert!(formats.contains_key(&LineType::StdErr));
        assert!(formats.contains_key(&LineType::Command));
        assert!(formats.contains_key(&LineType::System));
        assert!(formats.contains_key(&LineType::Error));
        
        // Check that colors are different
        let stdout_color = formats[&LineType::StdOut].color;
        let stderr_color = formats[&LineType::StdErr].color;
        assert_ne!(stdout_color, stderr_color);
    }

    #[test]
    fn test_terminal_widget_builder_default() {
        let builder = TerminalWidgetBuilder::default();
        let widget = builder.build().unwrap();
        
        assert_eq!(widget.line_count(), 0);
        assert!(widget.auto_scroll());
    }

    #[test]
    fn test_terminal_widget_event_receiver_none() {
        let mut widget = TerminalWidget::new("test");
        
        // Should not panic when no receiver is set
        let processed = widget.process_pending_events().unwrap();
        assert!(!processed);
    }

    #[test]
    fn test_terminal_widget_various_event_types() {
        let mut widget = TerminalWidget::new("test");
        
        let cmd_id = Uuid::new_v4();
        let events = vec![
            StreamEvent::CommandStarted(CommandInfo {
                id: cmd_id,
                command: "ls -la".to_string(),
                working_dir: Some("/tmp".to_string()),
                started_at: Utc::now(),
            }),
            StreamEvent::stdout(Some(cmd_id), "file1.txt".to_string()),
            StreamEvent::stdout(Some(cmd_id), "file2.txt".to_string()),
            StreamEvent::CommandFinished(ExitInfo {
                command_id: cmd_id,
                exit_code: Some(0),
                duration: Duration::from_millis(150),
                finished_at: Utc::now(),
            }),
            StreamEvent::StreamError {
                message: "Connection lost".to_string(),
                timestamp: Utc::now(),
            },
        ];
        
        widget.add_events(&events).unwrap();
        
        let stats = widget.stats();
        assert_eq!(stats.total_lines, 5);
        assert_eq!(stats.command_lines, 1);
        assert_eq!(stats.stdout_lines, 2);
        assert_eq!(stats.system_lines, 1);
        assert_eq!(stats.error_lines, 1);
    }

    #[test]
    fn test_terminal_widget_filtered_lines_with_search() {
        let mut widget = TerminalWidget::new("test");
        
        let events = vec![
            StreamEvent::stdout(None, "Building project...".to_string()),
            StreamEvent::stdout(None, "Compiling src/main.rs".to_string()),
            StreamEvent::stderr(None, "warning: unused variable".to_string()),
            StreamEvent::stdout(None, "Finished dev build".to_string()),
        ];
        
        widget.add_events(&events).unwrap();
        
        // Test no filter
        let all_lines = widget.get_filtered_lines();
        assert_eq!(all_lines.len(), 4);
        
        // Test filter for "build"
        widget.set_search_query("build".to_string());
        let filtered_lines = widget.get_filtered_lines();
        assert_eq!(filtered_lines.len(), 2); // "Building" and "build"
        
        // Test filter for "warning"
        widget.set_search_query("warning".to_string());
        let filtered_lines = widget.get_filtered_lines();
        assert_eq!(filtered_lines.len(), 1);
        
        // Test filter with no matches
        widget.set_search_query("nonexistent".to_string());
        let filtered_lines = widget.get_filtered_lines();
        assert_eq!(filtered_lines.len(), 0);
    }
} 