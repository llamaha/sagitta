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

    /// Workspace configuration
    #[serde(default)]
    pub workspaces: WorkspaceConfig,
}

impl Default for SagittaCodeConfig {
    fn default() -> Self {
        Self {
            gemini: GeminiConfig::default(),
            sagitta: SagittaDbConfig::default(),
            ui: UiConfig::default(),
            logging: LoggingConfig::default(),
            conversation: ConversationConfig::default(),
            workspaces: WorkspaceConfig::default(),
        }
    }
}

impl SagittaCodeConfig {
    /// Gets the path to the shared sagitta-search config file.
    /// Now uses the shared ~/.config/sagitta/config.toml
    pub fn sagitta_config_path(&self) -> PathBuf {
        sagitta_search::config::get_config_path()
            .unwrap_or_else(|_| {
                dirs::config_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("sagitta")
                    .join("config.toml")
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
            log_file_path: crate::config::paths::get_logs_path().ok()
                .map(|p| p.join("sagitta_code.log")),
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
    
    /// Sidebar configuration for persistent state
    #[serde(default)]
    pub sidebar: SidebarPersistentConfig,
}

/// Configuration for persistent sidebar state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidebarPersistentConfig {
    /// Last used organization mode
    #[serde(default = "default_organization_mode")]
    pub last_organization_mode: String,
    
    /// Expanded groups (group IDs)
    #[serde(default)]
    pub expanded_groups: Vec<String>,
    
    /// Last search query
    pub last_search_query: Option<String>,
    
    /// Filter settings
    #[serde(default)]
    pub filters: SidebarFiltersConfig,
    
    /// Show filters panel
    #[serde(default)]
    pub show_filters: bool,
    
    /// Show branch suggestions
    #[serde(default)]
    pub show_branch_suggestions: bool,
    
    /// Show checkpoint suggestions
    #[serde(default)]
    pub show_checkpoint_suggestions: bool,
    
    /// Sidebar width
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: f32,
    
    /// Enable keyboard shortcuts
    #[serde(default = "default_enable_keyboard_shortcuts")]
    pub enable_keyboard_shortcuts: bool,
    
    /// Enable accessibility features
    #[serde(default = "default_enable_accessibility")]
    pub enable_accessibility: bool,
    
    /// Color blind friendly palette
    #[serde(default)]
    pub color_blind_friendly: bool,
    
    /// Performance settings
    #[serde(default)]
    pub performance: SidebarPerformanceConfig,
}

impl Default for SidebarPersistentConfig {
    fn default() -> Self {
        Self {
            last_organization_mode: default_organization_mode(),
            expanded_groups: Vec::new(),
            last_search_query: None,
            filters: SidebarFiltersConfig::default(),
            show_filters: false,
            show_branch_suggestions: false,
            show_checkpoint_suggestions: false,
            sidebar_width: default_sidebar_width(),
            enable_keyboard_shortcuts: default_enable_keyboard_shortcuts(),
            enable_accessibility: default_enable_accessibility(),
            color_blind_friendly: false,
            performance: SidebarPerformanceConfig::default(),
        }
    }
}

/// Filter configuration for persistence
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SidebarFiltersConfig {
    /// Filter by project types
    #[serde(default)]
    pub project_types: Vec<String>,
    
    /// Filter by statuses
    #[serde(default)]
    pub statuses: Vec<String>,
    
    /// Filter by tags
    #[serde(default)]
    pub tags: Vec<String>,
    
    /// Minimum message count
    pub min_messages: Option<usize>,
    
    /// Minimum success rate
    pub min_success_rate: Option<f32>,
    
    /// Show only favorites
    #[serde(default)]
    pub favorites_only: bool,
    
    /// Show only with branches
    #[serde(default)]
    pub branches_only: bool,
    
    /// Show only with checkpoints
    #[serde(default)]
    pub checkpoints_only: bool,
}

/// Performance configuration for sidebar
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidebarPerformanceConfig {
    /// Enable virtual scrolling for large lists
    #[serde(default = "default_enable_virtual_scrolling")]
    pub enable_virtual_scrolling: bool,
    
    /// Threshold for enabling virtual scrolling
    #[serde(default = "default_virtual_scrolling_threshold")]
    pub virtual_scrolling_threshold: usize,
    
    /// Maximum items to render at once
    #[serde(default = "default_max_rendered_items")]
    pub max_rendered_items: usize,
    
    /// Enable lazy loading of conversation details
    #[serde(default = "default_enable_lazy_loading")]
    pub enable_lazy_loading: bool,
    
    /// Debounce delay for search (milliseconds)
    #[serde(default = "default_search_debounce_ms")]
    pub search_debounce_ms: u64,
}

impl Default for SidebarPerformanceConfig {
    fn default() -> Self {
        Self {
            enable_virtual_scrolling: default_enable_virtual_scrolling(),
            virtual_scrolling_threshold: default_virtual_scrolling_threshold(),
            max_rendered_items: default_max_rendered_items(),
            enable_lazy_loading: default_enable_lazy_loading(),
            search_debounce_ms: default_search_debounce_ms(),
        }
    }
}

impl Default for ConversationConfig {
    fn default() -> Self {
        Self {
            storage_path: crate::config::paths::get_conversations_path().ok(),
            auto_save: default_auto_save(),
            auto_create: default_auto_create(),
            max_conversations: Some(100),
            auto_cleanup_days: Some(30),
            auto_checkpoints: default_auto_checkpoints(),
            auto_branching: false,
            default_tags: Vec::new(),
            sidebar: SidebarPersistentConfig::default(),
        }
    }
}

/// Configuration for project workspaces
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    /// Directory to store workspace data
    pub storage_path: Option<PathBuf>,
    
    /// Automatically detect and switch workspaces
    #[serde(default = "default_auto_detect_workspaces")]
    pub auto_detect: bool,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            storage_path: crate::config::paths::get_workspaces_path().ok(),
            auto_detect: default_auto_detect_workspaces(),
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

fn default_auto_detect_workspaces() -> bool {
    true
}

fn default_organization_mode() -> String {
    "Recency".to_string()
}

fn default_sidebar_width() -> f32 {
    280.0
}

fn default_enable_keyboard_shortcuts() -> bool {
    true
}

fn default_enable_accessibility() -> bool {
    true
}

fn default_enable_virtual_scrolling() -> bool {
    true
}

fn default_virtual_scrolling_threshold() -> usize {
    1000
}

fn default_max_rendered_items() -> usize {
    100
}

fn default_enable_lazy_loading() -> bool {
    true
}

fn default_search_debounce_ms() -> u64 {
    300
}

// Configuration structures will go here
