use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use crate::providers::types::{ProviderType, ProviderConfig};


/// Main configuration for Sagitta Code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagittaCodeConfig {
    
    /// Provider configuration - Current active provider
    #[serde(default = "default_current_provider")]
    pub current_provider: ProviderType,
    
    /// Provider-specific configurations
    #[serde(default)]
    pub provider_configs: HashMap<ProviderType, ProviderConfig>,
    
    /// Legacy Claude Code configuration (deprecated, for backward compatibility)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude_code: Option<ClaudeCodeConfig>,
    
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
    
    /// Auto-sync configuration (file watching, auto-commit, etc.)
    #[serde(default)]
    pub auto_sync: AutoSyncConfig,
}


impl SagittaCodeConfig {
    /// Migrates legacy configuration to the new provider-based system
    pub fn migrate_legacy_config(&mut self) {
        // If provider_configs is empty and we have a legacy claude_code config
        if self.provider_configs.is_empty() && self.claude_code.is_some() {
            if let Some(legacy_config) = &self.claude_code {
                // Convert legacy config to new provider config format
                let provider_config = ProviderConfig::try_from(legacy_config.clone())
                    .unwrap_or_else(|_| {
                        // Fallback to default config if conversion fails
                        ProviderConfig::default_for_provider(ProviderType::ClaudeCode)
                    });
                
                self.provider_configs.insert(ProviderType::ClaudeCode, provider_config);
                self.current_provider = ProviderType::ClaudeCode;
                
                // Keep legacy config for backward compatibility, but mark for future removal
                // We don't remove it here to avoid breaking existing workflows
            }
        }
        
        // If no providers are configured at all, set up default Claude Code provider
        if self.provider_configs.is_empty() {
            let default_config = ProviderConfig::default_for_provider(ProviderType::ClaudeCode);
            self.provider_configs.insert(ProviderType::ClaudeCode, default_config);
            self.current_provider = ProviderType::ClaudeCode;
        }
    }
    
    /// Gets the current provider configuration
    pub fn get_current_provider_config(&self) -> Option<&ProviderConfig> {
        self.provider_configs.get(&self.current_provider)
    }
    
    /// Gets a mutable reference to the current provider configuration
    pub fn get_current_provider_config_mut(&mut self) -> Option<&mut ProviderConfig> {
        self.provider_configs.get_mut(&self.current_provider)
    }
    
    /// Sets the current provider and ensures its configuration exists
    pub fn set_current_provider(&mut self, provider_type: ProviderType) {
        self.current_provider = provider_type;
        
        // Ensure configuration exists for the provider
        if !self.provider_configs.contains_key(&provider_type) {
            let default_config = ProviderConfig::default_for_provider(provider_type);
            self.provider_configs.insert(provider_type, default_config);
        }
    }

    /// Gets the path to the application configuration file
    pub fn config_path(&self) -> PathBuf {
        crate::config::paths::get_sagitta_code_app_config_path()
            .unwrap_or_else(|_| {
                dirs::config_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("sagitta")
                    .join("sagitta_code_config.json")
            })
    }

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
    
    /// Loads configuration from a specific path
    pub fn load_from_path(path: &Path) -> anyhow::Result<Self> {
        crate::config::loader::load_config_from_path(path)
    }
    
    /// Saves configuration to a specific path
    pub fn save_to_path(&self, path: &Path) -> anyhow::Result<()> {
        crate::config::loader::save_config_to_path(self, path)
    }

    /// Gets the repositories base path with proper fallback logic
    /// Order of precedence:
    /// 1. config.sagitta.repositories_base_path (if set)
    /// 2. default ~/.local/share/sagitta/repositories
    pub fn repositories_base_path(&self) -> PathBuf {
        // First try sagitta.repositories_base_path
        if let Some(ref path) = self.sagitta.repositories_base_path {
            return path.clone();
        }
        
        // Finally fall back to default repositories path
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("sagitta")
            .join("repositories")
    }
}

impl Default for SagittaCodeConfig {
    fn default() -> Self {
        let mut config = Self {
            current_provider: default_current_provider(),
            provider_configs: HashMap::new(),
            claude_code: None,
            sagitta: SagittaDbConfig::default(),
            ui: UiConfig::default(),
            logging: LoggingConfig::default(),
            conversation: ConversationConfig::default(),
            auto_sync: AutoSyncConfig::default(),
        };
        
        // Always ensure at least one provider is configured
        config.migrate_legacy_config();
        
        config
    }
}


/// Configuration for Claude Code provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeCodeConfig {
    /// Path to claude binary (default: "claude")
    #[serde(default = "default_claude_path")]
    pub claude_path: String,
    
    /// Selected Claude model
    #[serde(default = "default_claude_model")]
    pub model: String,
    
    /// Fallback model when default is overloaded
    pub fallback_model: Option<String>,
    
    /// Maximum output tokens
    #[serde(default = "default_claude_max_output_tokens")]
    pub max_output_tokens: u32,
    
    /// Enable debug mode
    #[serde(default)]
    pub debug: bool,
    
    /// Enable verbose logging for debugging
    #[serde(default)]
    pub verbose: bool,
    
    /// Request timeout in seconds
    #[serde(default = "default_claude_timeout")]
    pub timeout: u64,
    
    /// Maximum turns for multi-turn conversations (0 = unlimited)
    #[serde(default = "default_claude_max_turns")]
    pub max_turns: u32,
    
    /// Output format: "text", "json", "stream-json"
    #[serde(default = "default_output_format")]
    pub output_format: String,
    
    /// Input format: "text", "stream-json"
    #[serde(default = "default_input_format")]
    pub input_format: String,
    
    /// Bypass all permission checks (for sandboxes)
    #[serde(default)]
    pub dangerously_skip_permissions: bool,
    
    /// Allowed tools (comma-separated list)
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    
    /// Disallowed tools (comma-separated list)
    #[serde(default)]
    pub disallowed_tools: Vec<String>,
    
    /// Additional directories to allow tool access to
    #[serde(default)]
    pub additional_directories: Vec<PathBuf>,
    
    /// MCP configuration file or string
    pub mcp_config: Option<String>,
    
    /// Automatically connect to IDE on startup
    #[serde(default)]
    pub auto_ide: bool,
}

impl Default for ClaudeCodeConfig {
    fn default() -> Self {
        Self {
            claude_path: default_claude_path(),
            model: default_claude_model(),
            fallback_model: None,
            max_output_tokens: default_claude_max_output_tokens(),
            debug: false,
            verbose: false,
            timeout: default_claude_timeout(),
            max_turns: default_claude_max_turns(),
            output_format: default_output_format(),
            input_format: default_input_format(),
            dangerously_skip_permissions: false,
            allowed_tools: Vec::new(),
            disallowed_tools: Vec::new(),
            additional_directories: Vec::new(),
            mcp_config: None,
            auto_ide: false,
        }
    }
}

/// Configuration for Sagitta Core
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct SagittaDbConfig {
    /// Base directory for repositories
    pub repositories_base_path: Option<PathBuf>,
    
    /// List of repository names to pre-load
    #[serde(default)]
    pub repositories: Vec<String>,
}


/// Dialog preferences for controlling when to show confirmation dialogs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogPreferences {
    /// Whether to show confirmation dialog when creating a new conversation
    #[serde(default = "default_show_new_conversation_confirmation")]
    pub show_new_conversation_confirmation: bool,
    
    /// Whether to show the provider setup dialog on startup
    #[serde(default = "default_show_provider_setup")]
    pub show_provider_setup: bool,
}

impl Default for DialogPreferences {
    fn default() -> Self {
        Self {
            show_new_conversation_confirmation: default_show_new_conversation_confirmation(),
            show_provider_setup: default_show_provider_setup(),
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
    
    /// Path to custom theme file (*.sagitta-theme.json)
    pub custom_theme_path: Option<PathBuf>,
    
    /// Window width
    #[serde(default = "default_window_width")]
    pub window_width: u32,
    
    /// Window height
    #[serde(default = "default_window_height")]
    pub window_height: u32,
    
    /// Currently selected repository context
    pub current_repository_context: Option<String>,
    
    /// Automatically create CLAUDE.md files when accessing repositories
    #[serde(default = "default_auto_create_claude_md")]
    pub auto_create_claude_md: bool,
    
    /// Custom CLAUDE.md template content
    #[serde(default = "default_claude_md_template")]
    pub claude_md_template: String,
    
    /// Dialog preferences for controlling when to show confirmation dialogs
    #[serde(default)]
    pub dialog_preferences: DialogPreferences,
    
    /// Whether the user has completed the first-run provider setup
    #[serde(default = "default_first_run_completed")]
    pub first_run_completed: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            dark_mode: default_dark_mode(),
            theme: default_theme(),
            custom_theme_path: None,
            window_width: default_window_width(),
            window_height: default_window_height(),
            current_repository_context: None,
            auto_create_claude_md: default_auto_create_claude_md(),
            claude_md_template: default_claude_md_template(),
            dialog_preferences: DialogPreferences::default(),
            first_run_completed: default_first_run_completed(),
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
    
    /// Fast model for conversation features (titles, tags, etc.)
    #[serde(default = "default_fast_model")]
    pub fast_model: String,
    
    /// Enable fast model for conversation features
    #[serde(default = "default_enable_fast_model")]
    pub enable_fast_model: bool,
    
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
            fast_model: default_fast_model(),
            enable_fast_model: default_enable_fast_model(),
        }
    }
}

/// Configuration for auto-sync features (file watching, auto-commit, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoSyncConfig {
    /// Enable auto-sync features
    #[serde(default = "default_auto_sync_enabled")]
    pub enabled: bool,
    
    /// File watcher configuration
    #[serde(default)]
    pub file_watcher: FileWatcherConfig,
    
    /// Auto-commit configuration
    #[serde(default)]
    pub auto_commit: AutoCommitConfig,
    
    /// Auto-sync after commit
    #[serde(default = "default_sync_after_commit")]
    pub sync_after_commit: bool,
    
    /// Auto-sync when switching repositories
    #[serde(default = "default_sync_on_repo_switch")]
    pub sync_on_repo_switch: bool,
    
    /// Auto-sync when adding new repositories
    #[serde(default = "default_sync_on_repo_add")]
    pub sync_on_repo_add: bool,
}

impl Default for AutoSyncConfig {
    fn default() -> Self {
        Self {
            enabled: default_auto_sync_enabled(),
            file_watcher: FileWatcherConfig::default(),
            auto_commit: AutoCommitConfig::default(),
            sync_after_commit: default_sync_after_commit(),
            sync_on_repo_switch: default_sync_on_repo_switch(),
            sync_on_repo_add: default_sync_on_repo_add(),
        }
    }
}

/// Configuration for file watching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWatcherConfig {
    /// Enable file watching
    #[serde(default = "default_file_watcher_enabled")]
    pub enabled: bool,
    
    /// Debounce interval in milliseconds
    #[serde(default = "default_file_watcher_debounce_ms")]
    pub debounce_ms: u64,
    
    /// Patterns to exclude from watching
    #[serde(default = "default_file_watcher_exclude_patterns")]
    pub exclude_patterns: Vec<String>,
    
    /// Maximum number of events to buffer
    #[serde(default = "default_file_watcher_max_buffer_size")]
    pub max_buffer_size: usize,
}

impl Default for FileWatcherConfig {
    fn default() -> Self {
        Self {
            enabled: default_file_watcher_enabled(),
            debounce_ms: default_file_watcher_debounce_ms(),
            exclude_patterns: default_file_watcher_exclude_patterns(),
            max_buffer_size: default_file_watcher_max_buffer_size(),
        }
    }
}

/// Configuration for auto-commit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoCommitConfig {
    /// Enable auto-commit
    #[serde(default = "default_auto_commit_enabled")]
    pub enabled: bool,
    
    /// Custom commit message template
    #[serde(default = "default_auto_commit_template")]
    pub commit_message_template: String,
    
    /// Attribution line to add to commit messages
    #[serde(default = "default_auto_commit_attribution")]
    pub attribution: String,
    
    /// Whether to skip pre-commit hooks
    #[serde(default = "default_auto_commit_skip_hooks")]
    pub skip_hooks: bool,
    
    /// Minimum time between auto-commits in seconds
    #[serde(default = "default_auto_commit_cooldown_seconds")]
    pub cooldown_seconds: u64,
}

impl Default for AutoCommitConfig {
    fn default() -> Self {
        Self {
            enabled: default_auto_commit_enabled(),
            commit_message_template: default_auto_commit_template(),
            attribution: default_auto_commit_attribution(),
            skip_hooks: default_auto_commit_skip_hooks(),
            cooldown_seconds: default_auto_commit_cooldown_seconds(),
        }
    }
}

/// Configuration for project workspaces
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

fn default_show_new_conversation_confirmation() -> bool {
    true
}

fn default_show_provider_setup() -> bool {
    true
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

fn default_current_provider() -> ProviderType {
    ProviderType::ClaudeCode
}

fn default_claude_path() -> String {
    "claude".to_string()
}

fn default_claude_model() -> String {
    "claude-sonnet-4-20250514".to_string()
}

fn default_claude_max_output_tokens() -> u32 {
    64000
}

fn default_claude_timeout() -> u64 {
    600 // 10 minutes
}

fn default_claude_max_turns() -> u32 {
    0 // 0 means unlimited turns
}

fn default_output_format() -> String {
    "text".to_string()
}

fn default_input_format() -> String {
    "text".to_string()
}

fn default_fast_model() -> String {
    "claude-3-5-haiku-20241022".to_string()
}

fn default_enable_fast_model() -> bool {
    true
}

fn default_auto_create_claude_md() -> bool {
    true
}

fn default_claude_md_template() -> String {
    include_str!("../../templates/CLAUDE.md").to_string()
}

fn default_first_run_completed() -> bool {
    false
}

// Auto-sync configuration defaults
fn default_auto_sync_enabled() -> bool {
    true
}

fn default_sync_after_commit() -> bool {
    true
}

fn default_sync_on_repo_switch() -> bool {
    true
}

fn default_sync_on_repo_add() -> bool {
    true
}

// File watcher configuration defaults
fn default_file_watcher_enabled() -> bool {
    true
}

fn default_file_watcher_debounce_ms() -> u64 {
    2000 // 2 seconds like aider
}

fn default_file_watcher_exclude_patterns() -> Vec<String> {
    vec![
        ".git/".to_string(),
        "target/".to_string(),
        "node_modules/".to_string(),
        ".cache/".to_string(),
        "build/".to_string(),
        "dist/".to_string(),
        ".next/".to_string(),
        "__pycache__/".to_string(),
        "*.tmp".to_string(),
        "*.temp".to_string(),
        "*.swp".to_string(),
        "*.swo".to_string(),
        "*~".to_string(),
        ".DS_Store".to_string(),
        "Thumbs.db".to_string(),
    ]
}

fn default_file_watcher_max_buffer_size() -> usize {
    1000
}

// Auto-commit configuration defaults
fn default_auto_commit_enabled() -> bool {
    true
}

fn default_auto_commit_template() -> String {
    "Auto-commit: {summary}

{details}".to_string()
}

fn default_auto_commit_attribution() -> String {
    "Co-authored-by: Sagitta AI <noreply@sagitta.ai>".to_string()
}

fn default_auto_commit_skip_hooks() -> bool {
    false
}

fn default_auto_commit_cooldown_seconds() -> u64 {
    30 // 30 seconds minimum between auto-commits
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_ui_config_default() {
        let config = UiConfig::default();
        assert!(config.dark_mode); // default_dark_mode() returns true
        assert_eq!(config.theme, "default"); // default_theme() returns "default"
        assert_eq!(config.custom_theme_path, None);
        assert_eq!(config.window_width, 900); // default_window_width() returns 900
        assert_eq!(config.window_height, 700); // default_window_height() returns 700
        assert_eq!(config.current_repository_context, None);
        assert!(config.auto_create_claude_md); // default_auto_create_claude_md() returns true
        assert!(!config.claude_md_template.is_empty()); // Should contain default template
        assert!(!config.first_run_completed); // default_first_run_completed() returns false
    }

    #[test]
    fn test_ui_config_with_repository_context() {
        let mut config = UiConfig::default();
        config.current_repository_context = Some("test-repo".to_string());
        
        assert_eq!(config.current_repository_context, Some("test-repo".to_string()));
    }

    #[test]
    fn test_sagitta_code_config_default() {
        let config = SagittaCodeConfig::default();
        
        // Check that UI config has None for repository context by default
        assert_eq!(config.ui.current_repository_context, None);
    }

    #[test]
    fn test_repositories_base_path() {
        let config = SagittaCodeConfig::default();
        
        // Default should use data dir
        let base_path = config.repositories_base_path();
        assert!(base_path.to_string_lossy().contains("sagitta"));
        assert!(base_path.to_string_lossy().contains("repositories"));
    }

    #[test]
    fn test_repositories_base_path_with_override() {
        let mut config = SagittaCodeConfig::default();
        let custom_path = PathBuf::from("/custom/repo/path");
        config.sagitta.repositories_base_path = Some(custom_path.clone());
        
        assert_eq!(config.repositories_base_path(), custom_path);
    }
}
