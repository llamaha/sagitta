use crate::error::{Result, TerminalError};
use egui::Color32;
use serde::{Deserialize, Serialize, Serializer, Deserializer};
use std::time::Duration;

/// Configuration for terminal widget behavior and appearance
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerminalConfig {
    /// Maximum number of lines to keep in buffer
    pub max_lines: usize,
    
    /// Whether to automatically scroll to bottom on new content
    pub auto_scroll: bool,
    
    /// Whether to show timestamps for each line
    pub show_timestamps: bool,
    
    /// Font size for terminal text
    pub font_size: f32,
    
    /// Colors for different line types
    pub colors: TerminalColors,
    
    /// Buffer management settings
    pub buffer: BufferConfig,
    
    /// Streaming settings
    pub streaming: StreamingConfig,
}

/// Color configuration for different types of terminal output
#[derive(Debug, Clone, PartialEq)]
pub struct TerminalColors {
    /// Color for stdout text
    pub stdout: Color32,
    
    /// Color for stderr text
    pub stderr: Color32,
    
    /// Color for command text
    pub command: Color32,
    
    /// Color for system messages
    pub system: Color32,
    
    /// Color for error messages
    pub error: Color32,
    
    /// Background color
    pub background: Color32,
    
    /// Color for timestamps
    pub timestamp: Color32,
}

// Custom serialization for TerminalColors since Color32 doesn't implement Serialize
impl Serialize for TerminalColors {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("TerminalColors", 7)?;
        state.serialize_field("stdout", &[self.stdout.r(), self.stdout.g(), self.stdout.b(), self.stdout.a()])?;
        state.serialize_field("stderr", &[self.stderr.r(), self.stderr.g(), self.stderr.b(), self.stderr.a()])?;
        state.serialize_field("command", &[self.command.r(), self.command.g(), self.command.b(), self.command.a()])?;
        state.serialize_field("system", &[self.system.r(), self.system.g(), self.system.b(), self.system.a()])?;
        state.serialize_field("error", &[self.error.r(), self.error.g(), self.error.b(), self.error.a()])?;
        state.serialize_field("background", &[self.background.r(), self.background.g(), self.background.b(), self.background.a()])?;
        state.serialize_field("timestamp", &[self.timestamp.r(), self.timestamp.g(), self.timestamp.b(), self.timestamp.a()])?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for TerminalColors {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct TerminalColorsVisitor;

        impl<'de> Visitor<'de> for TerminalColorsVisitor {
            type Value = TerminalColors;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct TerminalColors")
            }

            fn visit_map<V>(self, mut map: V) -> std::result::Result<TerminalColors, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut stdout = None;
                let mut stderr = None;
                let mut command = None;
                let mut system = None;
                let mut error = None;
                let mut background = None;
                let mut timestamp = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "stdout" => {
                            let rgba: [u8; 4] = map.next_value()?;
                            stdout = Some(Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]));
                        }
                        "stderr" => {
                            let rgba: [u8; 4] = map.next_value()?;
                            stderr = Some(Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]));
                        }
                        "command" => {
                            let rgba: [u8; 4] = map.next_value()?;
                            command = Some(Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]));
                        }
                        "system" => {
                            let rgba: [u8; 4] = map.next_value()?;
                            system = Some(Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]));
                        }
                        "error" => {
                            let rgba: [u8; 4] = map.next_value()?;
                            error = Some(Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]));
                        }
                        "background" => {
                            let rgba: [u8; 4] = map.next_value()?;
                            background = Some(Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]));
                        }
                        "timestamp" => {
                            let rgba: [u8; 4] = map.next_value()?;
                            timestamp = Some(Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]));
                        }
                        _ => {
                            return Err(de::Error::unknown_field(&key, &["stdout", "stderr", "command", "system", "error", "background", "timestamp"]));
                        }
                    }
                }

                Ok(TerminalColors {
                    stdout: stdout.ok_or_else(|| de::Error::missing_field("stdout"))?,
                    stderr: stderr.ok_or_else(|| de::Error::missing_field("stderr"))?,
                    command: command.ok_or_else(|| de::Error::missing_field("command"))?,
                    system: system.ok_or_else(|| de::Error::missing_field("system"))?,
                    error: error.ok_or_else(|| de::Error::missing_field("error"))?,
                    background: background.ok_or_else(|| de::Error::missing_field("background"))?,
                    timestamp: timestamp.ok_or_else(|| de::Error::missing_field("timestamp"))?,
                })
            }
        }

        deserializer.deserialize_struct("TerminalColors", &["stdout", "stderr", "command", "system", "error", "background", "timestamp"], TerminalColorsVisitor)
    }
}

/// Buffer management configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BufferConfig {
    /// Maximum number of lines before cleanup
    pub max_lines: usize,
    
    /// Number of lines to keep when cleanup occurs
    pub lines_to_keep: usize,
    
    /// Cleanup interval in milliseconds
    pub cleanup_interval_ms: u64,
}

/// Streaming configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamingConfig {
    /// Maximum time to wait for new content before updating UI
    pub update_interval_ms: u64,
    
    /// Buffer size for streaming content
    pub stream_buffer_size: usize,
    
    /// Whether to process ANSI escape codes
    pub process_ansi: bool,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            max_lines: 10000,
            auto_scroll: true,
            show_timestamps: false,
            font_size: 12.0,
            colors: TerminalColors::default(),
            buffer: BufferConfig::default(),
            streaming: StreamingConfig::default(),
        }
    }
}

impl Default for TerminalColors {
    fn default() -> Self {
        Self {
            stdout: Color32::from_rgb(200, 200, 200),      // Light gray
            stderr: Color32::from_rgb(255, 100, 100),      // Light red
            command: Color32::from_rgb(100, 255, 100),     // Light green
            system: Color32::from_rgb(100, 200, 255),      // Light blue
            error: Color32::from_rgb(255, 50, 50),         // Red
            background: Color32::from_rgb(20, 20, 20),     // Dark gray
            timestamp: Color32::from_rgb(150, 150, 150),   // Gray
        }
    }
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            max_lines: 10000,
            lines_to_keep: 5000,
            cleanup_interval_ms: 1000,
        }
    }
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            update_interval_ms: 50,    // 20 FPS
            stream_buffer_size: 8192,  // 8KB
            process_ansi: true,
        }
    }
}

impl TerminalConfig {
    /// Create a new configuration with validation
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum number of lines
    pub fn with_max_lines(mut self, max_lines: usize) -> Result<Self> {
        if max_lines == 0 {
            return Err(TerminalError::invalid_config("max_lines must be greater than 0"));
        }
        self.max_lines = max_lines;
        self.buffer.max_lines = max_lines;
        
        // Ensure lines_to_keep is valid relative to new max_lines
        if self.buffer.lines_to_keep >= max_lines {
            // Set lines_to_keep to be half of max_lines, ensuring it's always less than max_lines
            // but still at least 1
            self.buffer.lines_to_keep = std::cmp::max(1, max_lines / 2);
        }
        
        Ok(self)
    }

    /// Set auto-scroll behavior
    pub fn with_auto_scroll(mut self, auto_scroll: bool) -> Self {
        self.auto_scroll = auto_scroll;
        self
    }

    /// Set timestamp visibility
    pub fn with_timestamps(mut self, show_timestamps: bool) -> Self {
        self.show_timestamps = show_timestamps;
        self
    }

    /// Set font size
    pub fn with_font_size(mut self, font_size: f32) -> Result<Self> {
        if font_size <= 0.0 {
            return Err(TerminalError::invalid_config("font_size must be positive"));
        }
        self.font_size = font_size;
        Ok(self)
    }

    /// Set terminal colors
    pub fn with_colors(mut self, colors: TerminalColors) -> Self {
        self.colors = colors;
        self
    }

    /// Set buffer configuration
    pub fn with_buffer_config(mut self, buffer: BufferConfig) -> Result<Self> {
        buffer.validate()?;
        self.buffer = buffer;
        Ok(self)
    }

    /// Set streaming configuration
    pub fn with_streaming_config(mut self, streaming: StreamingConfig) -> Result<Self> {
        streaming.validate()?;
        self.streaming = streaming;
        Ok(self)
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        if self.max_lines == 0 {
            return Err(TerminalError::invalid_config("max_lines must be greater than 0"));
        }
        
        if self.font_size <= 0.0 {
            return Err(TerminalError::invalid_config("font_size must be positive"));
        }

        self.buffer.validate()?;
        self.streaming.validate()?;

        Ok(())
    }

    /// Get update interval as Duration
    pub fn update_interval(&self) -> Duration {
        Duration::from_millis(self.streaming.update_interval_ms)
    }

    /// Get cleanup interval as Duration
    pub fn cleanup_interval(&self) -> Duration {
        Duration::from_millis(self.buffer.cleanup_interval_ms)
    }
}

impl BufferConfig {
    /// Validate buffer configuration
    pub fn validate(&self) -> Result<()> {
        if self.max_lines == 0 {
            return Err(TerminalError::invalid_config("buffer max_lines must be greater than 0"));
        }
        
        if self.lines_to_keep >= self.max_lines {
            return Err(TerminalError::invalid_config("lines_to_keep must be less than max_lines"));
        }

        if self.lines_to_keep == 0 {
            return Err(TerminalError::invalid_config("lines_to_keep must be greater than 0"));
        }

        Ok(())
    }
}

impl StreamingConfig {
    /// Validate streaming configuration
    pub fn validate(&self) -> Result<()> {
        if self.update_interval_ms == 0 {
            return Err(TerminalError::invalid_config("update_interval_ms must be greater than 0"));
        }

        if self.stream_buffer_size == 0 {
            return Err(TerminalError::invalid_config("stream_buffer_size must be greater than 0"));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = TerminalConfig::default();
        assert_eq!(config.max_lines, 10000);
        assert!(config.auto_scroll);
        assert!(!config.show_timestamps);
        assert_eq!(config.font_size, 12.0);
        config.validate().unwrap();
    }

    #[test]
    fn test_config_with_max_lines() {
        let config = TerminalConfig::new().with_max_lines(5000).unwrap();
        assert_eq!(config.max_lines, 5000);
        assert_eq!(config.buffer.max_lines, 5000);

        // Test invalid max_lines
        let result = TerminalConfig::new().with_max_lines(0);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_with_auto_scroll() {
        let config = TerminalConfig::new().with_auto_scroll(false);
        assert!(!config.auto_scroll);
    }

    #[test]
    fn test_config_with_timestamps() {
        let config = TerminalConfig::new().with_timestamps(true);
        assert!(config.show_timestamps);
    }

    #[test]
    fn test_config_with_font_size() {
        let config = TerminalConfig::new().with_font_size(14.0).unwrap();
        assert_eq!(config.font_size, 14.0);

        // Test invalid font size
        let result = TerminalConfig::new().with_font_size(0.0);
        assert!(result.is_err());

        let result = TerminalConfig::new().with_font_size(-1.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_with_colors() {
        let colors = TerminalColors {
            stdout: Color32::WHITE,
            stderr: Color32::RED,
            command: Color32::GREEN,
            system: Color32::BLUE,
            error: Color32::YELLOW,
            background: Color32::BLACK,
            timestamp: Color32::GRAY,
        };

        let config = TerminalConfig::new().with_colors(colors.clone());
        assert_eq!(config.colors, colors);
    }

    #[test]
    fn test_buffer_config_validation() {
        // Valid config
        let buffer = BufferConfig {
            max_lines: 1000,
            lines_to_keep: 500,
            cleanup_interval_ms: 1000,
        };
        buffer.validate().unwrap();

        // Invalid: max_lines is 0
        let buffer = BufferConfig {
            max_lines: 0,
            lines_to_keep: 500,
            cleanup_interval_ms: 1000,
        };
        assert!(buffer.validate().is_err());

        // Invalid: lines_to_keep >= max_lines
        let buffer = BufferConfig {
            max_lines: 1000,
            lines_to_keep: 1000,
            cleanup_interval_ms: 1000,
        };
        assert!(buffer.validate().is_err());

        // Invalid: lines_to_keep is 0
        let buffer = BufferConfig {
            max_lines: 1000,
            lines_to_keep: 0,
            cleanup_interval_ms: 1000,
        };
        assert!(buffer.validate().is_err());
    }

    #[test]
    fn test_streaming_config_validation() {
        // Valid config
        let streaming = StreamingConfig {
            update_interval_ms: 50,
            stream_buffer_size: 8192,
            process_ansi: true,
        };
        streaming.validate().unwrap();

        // Invalid: update_interval_ms is 0
        let streaming = StreamingConfig {
            update_interval_ms: 0,
            stream_buffer_size: 8192,
            process_ansi: true,
        };
        assert!(streaming.validate().is_err());

        // Invalid: stream_buffer_size is 0
        let streaming = StreamingConfig {
            update_interval_ms: 50,
            stream_buffer_size: 0,
            process_ansi: true,
        };
        assert!(streaming.validate().is_err());
    }

    #[test]
    fn test_config_builder_pattern() {
        let config = TerminalConfig::new()
            .with_max_lines(5000).unwrap()
            .with_auto_scroll(false)
            .with_timestamps(true)
            .with_font_size(14.0).unwrap();

        assert_eq!(config.max_lines, 5000);
        assert!(!config.auto_scroll);
        assert!(config.show_timestamps);
        assert_eq!(config.font_size, 14.0);
        config.validate().unwrap();
    }

    #[test]
    fn test_config_serialization() {
        let config = TerminalConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: TerminalConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_terminal_colors_default() {
        let colors = TerminalColors::default();
        assert_ne!(colors.stdout, colors.stderr);
        assert_ne!(colors.command, colors.system);
        assert_ne!(colors.error, colors.background);
    }

    #[test]
    fn test_duration_getters() {
        let config = TerminalConfig::default();
        assert_eq!(config.update_interval(), Duration::from_millis(50));
        assert_eq!(config.cleanup_interval(), Duration::from_millis(1000));
    }

    #[test]
    fn test_config_validation_comprehensive() {
        // Test valid config
        let config = TerminalConfig::default();
        config.validate().unwrap();

        // Test invalid max_lines
        let mut config = TerminalConfig::default();
        config.max_lines = 0;
        assert!(config.validate().is_err());

        // Test invalid font_size
        let mut config = TerminalConfig::default();
        config.font_size = -1.0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_color_equality() {
        let colors1 = TerminalColors::default();
        let colors2 = TerminalColors::default();
        assert_eq!(colors1, colors2);

        let mut colors3 = TerminalColors::default();
        colors3.stdout = Color32::RED;
        assert_ne!(colors1, colors3);
    }
} 