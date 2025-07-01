use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use egui::Color32;
use std::time::Instant;

use crate::agent::conversation::types::{ConversationSummary, ProjectType};
use crate::agent::conversation::clustering::ConversationCluster;
use crate::agent::conversation::branching::BranchSuggestion;
use crate::agent::conversation::checkpoints::CheckpointSuggestion;
use crate::agent::state::types::{AgentMode, ConversationStatus};
use super::super::branch_suggestions::{BranchSuggestionsUI, BranchSuggestionAction};
use super::super::checkpoint_suggestions::{CheckpointSuggestionsUI, CheckpointSuggestionAction};

// --- Sidebar Action for conversation management ---
#[derive(Debug, Clone)]
pub enum SidebarAction {
    RequestDeleteConversation(Uuid),
    RenameConversation(Uuid, String),
    SwitchToConversation(Uuid),
    CreateNewConversation,
    RefreshConversations,
    SetWorkspace(Uuid),
    // Branch-related actions
    CreateBranch(Uuid, BranchSuggestion),
    DismissBranchSuggestion(Uuid, Uuid), // conversation_id, message_id
    RefreshBranchSuggestions(Uuid),
    ShowBranchDetails(BranchSuggestion),
    // Checkpoint-related actions for Phase 5
    CreateCheckpoint(Uuid, Uuid, String), // conversation_id, message_id, title
    RestoreCheckpoint(Uuid, Uuid), // conversation_id, checkpoint_id
    JumpToCheckpoint(Uuid, Uuid), // conversation_id, checkpoint_id
    DeleteCheckpoint(Uuid, Uuid), // conversation_id, checkpoint_id
    ShowCheckpointDetails(Uuid, Uuid), // conversation_id, checkpoint_id
}

// --- Display types for conversation items ---
#[derive(Debug, Clone, Default)]
pub struct DisplayIndicator {
    pub display: String,
    pub color: Option<Color32>,
}

#[derive(Debug, Clone, Default)]
pub struct ConversationDisplayDetails {
    pub title: String,
    pub time_display: String,
    pub indicators: Vec<DisplayIndicator>,
}

#[derive(Debug, Clone)]
pub struct DisplayConversationItem {
    pub summary: ConversationSummary,
    pub display: ConversationDisplayDetails,
    pub preview: Option<String>,
}

/// Conversation sidebar component for smart organization
#[derive(Clone)]
pub struct ConversationSidebar {
    /// Current organization mode
    pub organization_mode: OrganizationMode,
    
    /// Filter settings
    pub filters: SidebarFilters,
    
    /// Search query that is actively being used for filtering
    pub search_query: Option<String>,
    
    /// Live input buffer for the search text field
    pub search_input: String,
    
    /// Expanded groups in the sidebar
    pub expanded_groups: std::collections::HashSet<String>,
    
    /// Selected conversation ID
    pub selected_conversation: Option<Uuid>,
    
    /// Sidebar configuration
    pub config: SidebarConfig,
    
    /// Clusters
    pub clusters: Vec<ConversationCluster>,
    
    /// Edit buffer
    pub edit_buffer: String,
    
    /// Pending action to be processed
    pub pending_action: Option<SidebarAction>,
    
    /// Currently editing conversation ID
    pub editing_conversation_id: Option<Uuid>,
    
    /// Show filters panel
    pub show_filters: bool,
    
    /// Filter flags for quick access
    pub filter_active: bool,
    pub filter_completed: bool,
    pub filter_archived: bool,
    
    /// Branch suggestions UI
    pub branch_suggestions_ui: BranchSuggestionsUI,
    
    /// Branch suggestions per conversation
    pub conversation_branch_suggestions: HashMap<Uuid, Vec<BranchSuggestion>>,
    
    /// Show branch suggestions panel
    pub show_branch_suggestions: bool,
    
    /// Checkpoint suggestions UI
    pub checkpoint_suggestions_ui: CheckpointSuggestionsUI,
    
    /// Checkpoint suggestions per conversation
    pub conversation_checkpoint_suggestions: HashMap<Uuid, Vec<CheckpointSuggestion>>,
    
    /// Show checkpoint suggestions panel
    pub show_checkpoint_suggestions: bool,
    
    // Phase 10: Persistent state and performance features
    /// Last time state was saved
    pub last_state_save: Option<Instant>,
    
    /// Search debounce timer
    pub search_debounce_timer: Option<Instant>,
    
    /// Last search query for debouncing
    pub last_search_query: Option<String>,
    
    /// Virtual scrolling state
    pub virtual_scroll_offset: usize,
    
    /// Cached rendered items for performance
    pub cached_rendered_items: Option<(usize, Vec<ConversationItem>)>,
    
    /// Accessibility features enabled
    pub accessibility_enabled: bool,
    
    /// Color blind friendly mode
    pub color_blind_friendly: bool,
    
    /// High contrast mode
    pub high_contrast_mode: bool,
    
    /// Screen reader announcements
    pub screen_reader_announcements: Vec<String>,
    
    /// Last announcement time
    pub last_announcement_time: Option<Instant>,
}

/// Organization modes for the sidebar
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OrganizationMode {
    /// Organize by recency (most recent first)
    Recency,
    
    /// Organize by project/workspace
    Project,
    
    /// Organize by conversation status
    Status,
    
    /// Organize by semantic clusters
    Clusters,
    
    /// Organize by tags
    Tags,
    
    /// Organize by success rate
    Success,
    
    /// Custom organization
    Custom(String),
}

/// Filter settings for the sidebar
#[derive(Debug, Clone, Default)]
pub struct SidebarFilters {
    /// Filter by project type
    pub project_types: Vec<ProjectType>,
    
    /// Filter by status
    pub statuses: Vec<ConversationStatus>,
    
    /// Filter by tags
    pub tags: Vec<String>,
    
    /// Filter by date range
    pub date_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
    
    /// Filter by minimum message count
    pub min_messages: Option<usize>,
    
    /// Filter by success rate
    pub min_success_rate: Option<f32>,
    
    /// Show only favorites
    pub favorites_only: bool,
    
    /// Show only with branches
    pub branches_only: bool,
    
    /// Show only with checkpoints
    pub checkpoints_only: bool,
}

/// Configuration for the sidebar behavior
#[derive(Debug, Clone)]
pub struct SidebarConfig {
    /// Maximum number of conversations to show per group
    pub max_conversations_per_group: usize,
    
    /// Whether to show conversation previews
    pub show_previews: bool,
    
    /// Whether to show conversation tags
    pub show_tags: bool,
    
    /// Whether to show conversation statistics
    pub show_statistics: bool,
    
    /// Whether to auto-expand active groups
    pub auto_expand_active: bool,
    
    /// Whether to show empty groups
    pub show_empty_groups: bool,
    
    /// Default organization mode
    pub default_organization: OrganizationMode,
    
    /// Whether to persist sidebar state
    pub persist_state: bool,
    
    /// Refresh interval for live updates (in seconds)
    pub refresh_interval_seconds: u64,
    
    /// Responsive UI settings
    pub responsive: ResponsiveConfig,
}

/// Responsive UI configuration
#[derive(Debug, Clone)]
pub struct ResponsiveConfig {
    /// Enable responsive behavior
    pub enabled: bool,
    
    /// Small screen breakpoint (width in pixels)
    pub small_screen_breakpoint: f32,
    
    /// Compact mode settings
    pub compact_mode: CompactModeConfig,
}

/// Compact mode configuration
#[derive(Debug, Clone)]
pub struct CompactModeConfig {
    /// Use smaller buttons and icons
    pub small_buttons: bool,
    
    /// Reduce spacing between elements
    pub reduced_spacing: bool,
    
    /// Show abbreviated labels
    pub abbreviated_labels: bool,
    
    /// Hide less important UI elements
    pub hide_secondary_elements: bool,
}

impl Default for SidebarConfig {
    fn default() -> Self {
        Self {
            max_conversations_per_group: 50,
            show_previews: true,
            show_tags: true,
            show_statistics: true,
            auto_expand_active: true,
            show_empty_groups: false,
            default_organization: OrganizationMode::Recency,
            persist_state: true,
            refresh_interval_seconds: 30,
            responsive: ResponsiveConfig::default(),
        }
    }
}

impl Default for ResponsiveConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            small_screen_breakpoint: 800.0,
            compact_mode: CompactModeConfig::default(),
        }
    }
}

impl Default for CompactModeConfig {
    fn default() -> Self {
        Self {
            small_buttons: true,
            reduced_spacing: true,
            abbreviated_labels: false,
            hide_secondary_elements: false,
        }
    }
}

/// Organized conversations structure
#[derive(Debug, Clone)]
pub struct OrganizedConversations {
    /// Groups of conversations
    pub groups: Vec<ConversationGroup>,
    
    /// Total number of conversations
    pub total_count: usize,
    
    /// Number of filtered conversations
    pub filtered_count: usize,
    
    /// Organization mode used
    pub organization_mode: OrganizationMode,
}

/// A group of conversations
#[derive(Debug, Clone)]
pub struct ConversationGroup {
    /// Group identifier
    pub id: String,
    
    /// Group display name
    pub name: String,
    
    /// Group description
    pub description: Option<String>,
    
    /// Conversations in this group
    pub conversations: Vec<ConversationItem>,
    
    /// Group metadata
    pub metadata: GroupMetadata,
    
    /// Whether the group is expanded
    pub expanded: bool,
    
    /// Group priority for sorting
    pub priority: i32,
}

/// A conversation item within a group
#[derive(Debug, Clone)]
pub struct ConversationItem {
    /// Conversation summary
    pub summary: ConversationSummary,
    
    /// Display metadata
    pub display: ConversationDisplay,
    
    /// Whether this conversation is selected
    pub selected: bool,
    
    /// Whether this conversation is a favorite
    pub favorite: bool,
    
    /// Conversation preview text
    pub preview: Option<String>,
}

/// Display information for a conversation
#[derive(Debug, Clone)]
pub struct ConversationDisplay {
    /// Display title (may be truncated)
    pub title: String,
    
    /// Status indicator
    pub status_indicator: StatusIndicator,
    
    /// Time display (relative or absolute)
    pub time_display: String,
    
    /// Progress indicator (for active conversations)
    pub progress: Option<f32>,
    
    /// Visual indicators (badges, icons)
    pub indicators: Vec<VisualIndicator>,
    
    /// Color theme for this conversation
    pub color_theme: Option<String>,
}

/// Status indicator for conversations
#[derive(Debug, Clone)]
pub enum StatusIndicator {
    Active,
    Paused,
    Completed,
    Failed,
    Archived,
    Branched,
    Checkpointed,
}

/// Visual indicator for conversations
#[derive(Debug, Clone)]
pub struct VisualIndicator {
    /// Indicator type
    pub indicator_type: IndicatorType,
    
    /// Display text or icon
    pub display: String,
    
    /// Tooltip text
    pub tooltip: Option<String>,
    
    /// Color or style
    pub style: Option<String>,
}

/// Type of visual indicator
#[derive(Debug, Clone)]
pub enum IndicatorType {
    Branch,
    Checkpoint,
    Success,
    Warning,
    Error,
    Tag,
    Project,
    Favorite,
    Recent,
    Popular,
}

/// Metadata for conversation groups
#[derive(Debug, Clone)]
pub struct GroupMetadata {
    /// Number of conversations in group
    pub count: usize,
    
    /// Average success rate for group
    pub avg_success_rate: Option<f32>,
    
    /// Most recent activity in group
    pub last_activity: Option<DateTime<Utc>>,
    
    /// Group statistics
    pub statistics: GroupStatistics,
}

/// Statistics for conversation groups
#[derive(Debug, Clone)]
pub struct GroupStatistics {
    /// Total messages in group
    pub total_messages: usize,
    
    /// Active conversations
    pub active_count: usize,
    
    /// Completed conversations
    pub completed_count: usize,
    
    /// Average conversation length
    pub avg_length: f64,
    
    /// Most common tags
    pub common_tags: Vec<String>,
}

impl ConversationSidebar {
    /// Create a new conversation sidebar
    pub fn new(config: SidebarConfig) -> Self {
        Self {
            organization_mode: config.default_organization.clone(),
            filters: SidebarFilters::default(),
            search_query: None,
            search_input: String::new(),
            expanded_groups: std::collections::HashSet::new(),
            selected_conversation: None,
            config,
            clusters: Vec::new(),
            edit_buffer: String::new(),
            pending_action: None,
            editing_conversation_id: None,
            show_filters: false,
            filter_active: false,
            filter_completed: false,
            filter_archived: false,
            branch_suggestions_ui: BranchSuggestionsUI::new(),
            conversation_branch_suggestions: HashMap::new(),
            show_branch_suggestions: false,
            checkpoint_suggestions_ui: CheckpointSuggestionsUI::with_default_config(),
            conversation_checkpoint_suggestions: HashMap::new(),
            show_checkpoint_suggestions: false,
            last_state_save: None,
            search_debounce_timer: None,
            last_search_query: None,
            virtual_scroll_offset: 0,
            cached_rendered_items: None,
            accessibility_enabled: true,
            color_blind_friendly: false,
            high_contrast_mode: false,
            screen_reader_announcements: Vec::new(),
            last_announcement_time: None,
        }
    }
    
    /// Create sidebar with default configuration
    pub fn with_default_config() -> Self {
        Self::new(SidebarConfig::default())
    }

    /// Set organization mode
    pub fn set_organization_mode(&mut self, mode: OrganizationMode) {
        self.organization_mode = mode;
        self.invalidate_cache();
    }

    /// Toggle group expansion
    pub fn toggle_group(&mut self, group_id: &str) {
        let collapsed_key = format!("collapsed_{}", group_id);
        let expanded_key = format!("expanded_{}", group_id);
        
        // Determine current state
        let is_currently_expanded = if self.expanded_groups.contains(&collapsed_key) {
            false
        } else if self.expanded_groups.contains(&expanded_key) {
            true
        } else {
            // Default state
            group_id == "today"
        };
        
        // Clear both keys
        self.expanded_groups.remove(&collapsed_key);
        self.expanded_groups.remove(&expanded_key);
        
        // Set new state (opposite of current)
        if is_currently_expanded {
            self.expanded_groups.insert(collapsed_key);
        } else {
            self.expanded_groups.insert(expanded_key);
        }
    }

    /// Select conversation
    pub fn select_conversation(&mut self, conversation_id: Option<Uuid>) {
        self.selected_conversation = conversation_id;
    }

    /// Get current configuration
    pub fn get_config(&self) -> &SidebarConfig {
        &self.config
    }

    /// Update configuration
    pub fn update_config(&mut self, config: SidebarConfig) {
        self.config = config;
        self.invalidate_cache();
    }

    /// Update branch suggestions for a conversation
    pub fn update_branch_suggestions(&mut self, conversation_id: Uuid, suggestions: Vec<BranchSuggestion>) {
        self.conversation_branch_suggestions.insert(conversation_id, suggestions.clone());
        
        // If this is the currently selected conversation, update the UI
        if self.selected_conversation == Some(conversation_id) {
            self.branch_suggestions_ui.update_suggestions(suggestions);
        }
    }

    /// Get branch suggestions for a conversation
    pub fn get_branch_suggestions(&self, conversation_id: Uuid) -> Option<&Vec<BranchSuggestion>> {
        self.conversation_branch_suggestions.get(&conversation_id)
    }

    /// Clear branch suggestions for a conversation
    pub fn clear_branch_suggestions(&mut self, conversation_id: Uuid) {
        self.conversation_branch_suggestions.remove(&conversation_id);
        
        // If this is the currently selected conversation, clear the UI
        if self.selected_conversation == Some(conversation_id) {
            self.branch_suggestions_ui.update_suggestions(Vec::new());
        }
    }

    /// Toggle branch suggestions panel
    pub fn toggle_branch_suggestions(&mut self) {
        self.show_branch_suggestions = !self.show_branch_suggestions;
    }

    /// Handle branch suggestion actions
    pub fn handle_branch_suggestion_action(&mut self, action: BranchSuggestionAction) -> Option<SidebarAction> {
        match action {
            BranchSuggestionAction::CreateBranch { conversation_id, suggestion } => {
                Some(SidebarAction::CreateBranch(conversation_id, suggestion))
            },
            BranchSuggestionAction::DismissSuggestion { conversation_id, message_id } => {
                self.branch_suggestions_ui.dismiss_suggestion(message_id);
                Some(SidebarAction::DismissBranchSuggestion(conversation_id, message_id))
            },
            BranchSuggestionAction::ShowDetails { suggestion } => {
                Some(SidebarAction::ShowBranchDetails(suggestion))
            },
            BranchSuggestionAction::RefreshSuggestions { conversation_id } => {
                Some(SidebarAction::RefreshBranchSuggestions(conversation_id))
            },
        }
    }

    /// Update checkpoint suggestions for a conversation
    pub fn update_checkpoint_suggestions(&mut self, conversation_id: Uuid, suggestions: Vec<CheckpointSuggestion>) {
        self.conversation_checkpoint_suggestions.insert(conversation_id, suggestions.clone());
        
        // If this is the currently selected conversation, update the UI
        if self.selected_conversation == Some(conversation_id) {
            self.checkpoint_suggestions_ui.update_suggestions(suggestions);
        }
    }

    /// Get checkpoint suggestions for a conversation
    pub fn get_checkpoint_suggestions(&self, conversation_id: Uuid) -> Option<&Vec<CheckpointSuggestion>> {
        self.conversation_checkpoint_suggestions.get(&conversation_id)
    }

    /// Clear checkpoint suggestions for a conversation
    pub fn clear_checkpoint_suggestions(&mut self, conversation_id: Uuid) {
        self.conversation_checkpoint_suggestions.remove(&conversation_id);
        
        // If this is the currently selected conversation, clear the UI
        if self.selected_conversation == Some(conversation_id) {
            self.checkpoint_suggestions_ui.update_suggestions(Vec::new());
        }
    }

    /// Toggle checkpoint suggestions panel
    pub fn toggle_checkpoint_suggestions(&mut self) {
        self.show_checkpoint_suggestions = !self.show_checkpoint_suggestions;
    }

    /// Handle checkpoint suggestion actions
    pub fn handle_checkpoint_suggestion_action(&mut self, action: CheckpointSuggestionAction) -> Option<SidebarAction> {
        match action {
            CheckpointSuggestionAction::CreateCheckpoint { conversation_id, suggestion } => {
                Some(SidebarAction::CreateCheckpoint(conversation_id, suggestion.message_id, suggestion.suggested_title))
            },
            CheckpointSuggestionAction::DismissSuggestion { conversation_id, message_id } => {
                self.checkpoint_suggestions_ui.dismiss_suggestion(message_id);
                None
            },
            CheckpointSuggestionAction::ShowDetails { suggestion } => {
                // Since CheckpointSuggestion doesn't have conversation_id, we'll need to find it
                // For now, use a placeholder - this would need to be passed differently
                Some(SidebarAction::ShowCheckpointDetails(Uuid::new_v4(), suggestion.message_id))
            },
            CheckpointSuggestionAction::RefreshSuggestions { conversation_id: _ } => {
                // Trigger refresh - this would be handled by the app
                None
            },
            CheckpointSuggestionAction::JumpToMessage { conversation_id, message_id } => {
                // Handle jumping to a specific message - this could be a new sidebar action or handled directly
                // For now, we'll return None since this doesn't require a sidebar action
                None
            },
        }
    }

    /// Invalidate cached items
    pub fn invalidate_cache(&mut self) {
        self.cached_rendered_items = None;
    }
}

// Utility function for status icons
pub fn get_status_icon(status: ConversationStatus) -> String {
    match status {
        ConversationStatus::Active => "▶".to_string(),
        ConversationStatus::Paused => "⏸".to_string(),
        ConversationStatus::Completed => "✅".to_string(),
        ConversationStatus::Archived => "📦".to_string(),
        ConversationStatus::Summarizing => "⏳".to_string(),
    }
} 