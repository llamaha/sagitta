use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main configuration for Sagitta Code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagittaCodeConfig {
    /// Gemini API configuration
    #[serde(default)]
    pub gemini: GeminiConfig,
    
    /// Vector DB configuration
    #[serde(default)]
    pub sagitta: SagittaDbConfig,
    
    /// UI configuration
    #[serde(default)]
    pub ui: UiConfig,
    
    /// Logging configuration
    #[serde(default)]
    pub logging: LoggingConfig,
    
    /// Conversation management configuration
    #[serde(default)]
    pub conversation: ConversationConfig,
}

impl Default for SagittaCodeConfig {
    fn default() -> Self {
        Self {
            gemini: GeminiConfig::default(),
            sagitta: SagittaDbConfig::default(),
            ui: UiConfig::default(),
            logging: LoggingConfig::default(),
            conversation: ConversationConfig::default(),
        }
    }
}

impl SagittaCodeConfig {
    /// Gets the path to the sagitta-search config file.
    /// Uses Sagitta Code's dedicated core config path.
    pub fn sagitta_config_path(&self) -> PathBuf {
        crate::config::paths::get_sagitta_code_core_config_path()
            .unwrap_or_else(|_| {
                let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
                path.push(".config");
                path.push("sagitta");
                path.push("config.toml");
                path
            })
    }
}

/// Configuration for Gemini API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiConfig {
    /// Gemini API key
    pub api_key: Option<String>,
    
    /// Gemini model to use
    #[serde(default = "default_gemini_model")]
    pub model: String,
    
    /// Maximum message history size
    #[serde(default = "default_max_history_size")]
    pub max_history_size: usize,
    
    /// Maximum reasoning steps to prevent infinite loops
    #[serde(default = "default_max_reasoning_steps")]
    pub max_reasoning_steps: u32,
}

impl Default for GeminiConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            model: default_gemini_model(),
            max_history_size: default_max_history_size(),
            max_reasoning_steps: default_max_reasoning_steps(),
        }
    }
}

/// Configuration for Sagitta Core
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagittaDbConfig {
    /// Base directory for repositories
    pub repositories_base_path: Option<PathBuf>,
    
    /// List of repository names to pre-load
    #[serde(default)]
    pub repositories: Vec<String>,
}

impl Default for SagittaDbConfig {
    fn default() -> Self {
        Self {
            repositories_base_path: None,
            repositories: Vec::new(),
        }
    }
}

/// Configuration for the UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Whether to use dark mode
    #[serde(default = "default_dark_mode")]
    pub dark_mode: bool,
    
    /// Theme to use
    #[serde(default = "default_theme")]
    pub theme: String,
    
    /// Window width
    #[serde(default = "default_window_width")]
    pub window_width: u32,
    
    /// Window height
    #[serde(default = "default_window_height")]
    pub window_height: u32,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            dark_mode: default_dark_mode(),
            theme: default_theme(),
            window_width: default_window_width(),
            window_height: default_window_height(),
        }
    }
}

/// Configuration for logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level
    #[serde(default = "default_log_level")]
    pub log_level: String,
    
    /// Whether to log to a file
    #[serde(default)]
    pub log_to_file: bool,
    
    /// Log file path
    pub log_file_path: Option<PathBuf>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            log_level: default_log_level(),
            log_to_file: false,
            log_file_path: None,
        }
    }
}

/// Configuration for conversation management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationConfig {
    /// Directory to store conversation data
    pub storage_path: Option<PathBuf>,
    
    /// Whether to auto-save conversations
    #[serde(default = "default_auto_save")]
    pub auto_save: bool,
    
    /// Auto-create conversations on first message
    #[serde(default = "default_auto_create")]
    pub auto_create: bool,
    
    /// Maximum number of conversations to keep
    pub max_conversations: Option<usize>,
    
    /// Auto-cleanup conversations older than this many days
    pub auto_cleanup_days: Option<u32>,
    
    /// Enable automatic checkpoints
    #[serde(default = "default_auto_checkpoints")]
    pub auto_checkpoints: bool,
    
    /// Enable automatic branching for major decisions
    #[serde(default)]
    pub auto_branching: bool,
    
    /// Default tags to apply to new conversations
    #[serde(default)]
    pub default_tags: Vec<String>,
}

impl Default for ConversationConfig {
    fn default() -> Self {
        Self {
            storage_path: None,
            auto_save: default_auto_save(),
            auto_create: default_auto_create(),
            max_conversations: Some(100),
            auto_cleanup_days: Some(30),
            auto_checkpoints: default_auto_checkpoints(),
            auto_branching: false,
            default_tags: Vec::new(),
        }
    }
}

fn default_gemini_model() -> String {
    "gemini-2.5-flash-preview-05-20".to_string()
}

fn default_max_history_size() -> usize {
    20
}

fn default_max_reasoning_steps() -> u32 {
    50
}

fn default_dark_mode() -> bool {
    true
}

fn default_theme() -> String {
    "default".to_string()
}

fn default_window_width() -> u32 {
    900
}

fn default_window_height() -> u32 {
    700
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_auto_save() -> bool {
    true
}

fn default_auto_create() -> bool {
    true
}

fn default_auto_checkpoints() -> bool {
    true
}

// Configuration structures will go here
