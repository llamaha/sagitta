use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use egui::{Align, Color32, ComboBox, Frame, Grid, Layout, RichText, ScrollArea, Stroke, TextEdit, Ui, Vec2, WidgetText, Context, Margin, Response};
use egui_extras::{Size, StripBuilder};
use std::time::{Duration, Instant};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::agent::conversation::types::{ConversationSummary, ProjectType, ProjectContext};
use crate::project::workspace::types::WorkspaceSummary;
use crate::agent::conversation::clustering::ConversationCluster;
use crate::agent::conversation::branching::{BranchSuggestion, ConversationBranchingManager};
use crate::agent::conversation::checkpoints::CheckpointSuggestion;
use crate::agent::state::types::{AgentMode, ConversationStatus};
use crate::gui::theme::AppTheme;
use crate::gui::app::AppState;
use crate::config::{SagittaCodeConfig, SidebarPersistentConfig, save_config};
use super::branch_suggestions::{BranchSuggestionsUI, BranchSuggestionAction, BranchSuggestionsConfig};
use super::checkpoint_suggestions::{CheckpointSuggestionsUI, CheckpointSuggestionAction, CheckpointSuggestionsConfig};
use crate::agent::conversation::service::ConversationService;
use crate::gui::app::events::{AppEvent, ConversationEvent};
use tokio::sync::mpsc::UnboundedSender;

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

fn get_status_icon(status: ConversationStatus) -> String {
    match status {
        ConversationStatus::Active => "▶".to_string(),
        ConversationStatus::Paused => "⏸".to_string(),
        ConversationStatus::Completed => "✅".to_string(),
        ConversationStatus::Archived => "📦".to_string(),
        ConversationStatus::Summarizing => "⏳".to_string(),
    }
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

/// Configuration for the sidebar
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidebarConfig {
    /// Maximum number of conversations to show per group
    pub max_conversations_per_group: usize,
    
    /// Whether to show conversation previews
    pub show_previews: bool,
    
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

/// Responsive configuration for different screen sizes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsiveConfig {
    /// Enable responsive behavior
    pub enabled: bool,
    
    /// Small screen breakpoint (width in pixels)
    pub small_screen_breakpoint: f32,
    
    /// Compact mode settings
    pub compact_mode: CompactModeConfig,
}

/// Configuration for compact mode on smaller screens
#[derive(Debug, Clone, Serialize, Deserialize)]
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
            max_conversations_per_group: 20,
            show_previews: true,
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
            small_screen_breakpoint: 1366.0,
            compact_mode: CompactModeConfig::default(),
        }
    }
}

impl Default for CompactModeConfig {
    fn default() -> Self {
        Self {
            small_buttons: true,
            reduced_spacing: true,
            abbreviated_labels: true,
            hide_secondary_elements: false,
        }
    }
}

/// Organized conversation data for display
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

/// A group of conversations in the sidebar
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

/// Individual conversation item in the sidebar
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

/// Display metadata for conversations
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

/// Status indicators for conversations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusIndicator {
    Active,
    Paused,
    Completed,
    Failed,
    Archived,
    Branched,
    Checkpointed,
}

/// Visual indicators for conversations
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

/// Types of visual indicators
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Group metadata
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

/// Statistics for a conversation group
#[derive(Debug, Clone, Default)]
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
    
    /// Organize conversations for display
    pub fn organize_conversations(
        &self,
        conversations: &[ConversationSummary],
        clusters: Option<&[ConversationCluster]>,
        workspaces: &[WorkspaceSummary],
        active_workspace_id: Option<Uuid>,
    ) -> Result<OrganizedConversations> {
        // Apply search and filters first
        let filtered_conversations = self.apply_filters(conversations);
        
        // Apply search if present
        let searched_conversations = if let Some(query) = &self.search_query {
            if !query.is_empty() {
                self.apply_search(&filtered_conversations, query)
            } else {
                filtered_conversations
            }
        } else {
            filtered_conversations
        };
        
        // Organize into groups based on mode
        let groups = match &self.organization_mode {
            OrganizationMode::Recency => self.organize_by_recency(&searched_conversations),
            OrganizationMode::Project => self.organize_by_project(&searched_conversations, workspaces, active_workspace_id),
            OrganizationMode::Status => self.organize_by_status(&searched_conversations),
            OrganizationMode::Clusters => self.organize_by_clusters(&searched_conversations, clusters),
            OrganizationMode::Tags => self.organize_by_tags(&searched_conversations),
            OrganizationMode::Success => self.organize_by_success(&searched_conversations),
            OrganizationMode::Custom(mode) => self.organize_custom(&searched_conversations, mode),
        }?;
        
        Ok(OrganizedConversations {
            groups,
            total_count: conversations.len(),
            filtered_count: searched_conversations.len(),
            organization_mode: self.organization_mode.clone(),
        })
    }
    
    /// Apply filters to conversations
    fn apply_filters(&self, conversations: &[ConversationSummary]) -> Vec<ConversationSummary> {
        conversations
            .iter()
            .filter(|conv| {
                // Project type filter
                if !self.filters.project_types.is_empty() {
                    // Note: We'd need project type in ConversationSummary or fetch it separately
                    // For now, skip this filter
                }
                
                // Status filter
                if !self.filters.statuses.is_empty() && !self.filters.statuses.contains(&conv.status) {
                    return false;
                }
                
                // Tags filter
                if !self.filters.tags.is_empty() {
                    let has_matching_tag = self.filters.tags.iter().any(|tag| conv.tags.contains(tag));
                    if !has_matching_tag {
                        return false;
                    }
                }
                
                // Date range filter
                if let Some((start, end)) = self.filters.date_range {
                    if conv.last_active < start || conv.last_active > end {
                        return false;
                    }
                }
                
                // Message count filter
                if let Some(min_messages) = self.filters.min_messages {
                    if conv.message_count < min_messages {
                        return false;
                    }
                }
                
                // Branches filter
                if self.filters.branches_only && !conv.has_branches {
                    return false;
                }
                
                // Checkpoints filter
                if self.filters.checkpoints_only && !conv.has_checkpoints {
                    return false;
                }
                
                true
            })
            .cloned()
            .collect()
    }
    
    /// Apply search to conversations
    fn apply_search(&self, conversations: &[ConversationSummary], query: &str) -> Vec<ConversationSummary> {
        let query_lower = query.to_lowercase();
        
        conversations
            .iter()
            .filter(|conv| {
                // Search in title
                if conv.title.to_lowercase().contains(&query_lower) {
                    return true;
                }
                
                // Search in tags
                if conv.tags.iter().any(|tag| tag.to_lowercase().contains(&query_lower)) {
                    return true;
                }
                
                // Search in project name
                if let Some(ref project_name) = conv.project_name {
                    if project_name.to_lowercase().contains(&query_lower) {
                        return true;
                    }
                }
                
                false
            })
            .cloned()
            .collect()
    }
    
    /// Organize conversations by recency
    fn organize_by_recency(&self, conversations: &[ConversationSummary]) -> Result<Vec<ConversationGroup>> {
        let mut sorted_conversations = conversations.to_vec();
        sorted_conversations.sort_by(|a, b| b.last_active.cmp(&a.last_active));
        
        let mut groups = Vec::new();
        let now = Utc::now();
        
        // Group by time periods
        let mut today = Vec::new();
        let mut yesterday = Vec::new();
        let mut this_week = Vec::new();
        let mut this_month = Vec::new();
        let mut older = Vec::new();
        
        for conv in sorted_conversations {
            let age = now.signed_duration_since(conv.last_active);
            
            if age.num_days() == 0 {
                today.push(conv);
            } else if age.num_days() == 1 {
                yesterday.push(conv);
            } else if age.num_days() <= 7 {
                this_week.push(conv);
            } else if age.num_days() <= 30 {
                this_month.push(conv);
            } else {
                older.push(conv);
            }
        }
        
        // Create groups
        if !today.is_empty() {
            groups.push(self.create_group("today", "Today", today, 100)?);
        }
        if !yesterday.is_empty() {
            groups.push(self.create_group("yesterday", "Yesterday", yesterday, 90)?);
        }
        if !this_week.is_empty() {
            groups.push(self.create_group("this_week", "This Week", this_week, 80)?);
        }
        if !this_month.is_empty() {
            groups.push(self.create_group("this_month", "This Month", this_month, 70)?);
        }
        if !older.is_empty() {
            groups.push(self.create_group("older", "Older", older, 60)?);
        }
        
        Ok(groups)
    }
    
    /// Organize conversations by project
    fn organize_by_project(
        &self,
        conversations: &[ConversationSummary],
        workspaces: &[WorkspaceSummary],
        active_workspace_id: Option<Uuid>,
    ) -> Result<Vec<ConversationGroup>> {
        let mut groups: HashMap<Option<Uuid>, Vec<ConversationSummary>> = HashMap::new();

        // Filter conversations by active workspace if one is selected
        let conversations_to_organize = if let Some(active_id) = active_workspace_id {
            conversations
                .iter()
                .filter(|c| c.workspace_id == Some(active_id))
                .cloned()
                .collect()
        } else {
            conversations.to_vec()
        };

        for conv in conversations_to_organize {
            groups.entry(conv.workspace_id).or_default().push(conv);
        }

        let mut conversation_groups = Vec::new();
        for (workspace_id, convs) in groups {
            let (group_name, priority) = match workspace_id {
                Some(id) => {
                    let name = workspaces
                        .iter()
                        .find(|ws| ws.id == id)
                        .map(|ws| ws.name.clone())
                        .unwrap_or_else(|| "Unknown Workspace".to_string());
                    (name, 0)
                }
                None => ("No Workspace".to_string(), 1),
            };

            let group = self.create_group(
                &workspace_id.map(|id| id.to_string()).unwrap_or_else(|| "no-workspace".to_string()),
                &group_name,
                convs,
                priority,
            )?;
            conversation_groups.push(group);
        }

        conversation_groups.sort_by(|a, b| a.priority.cmp(&b.priority).then_with(|| a.name.cmp(&b.name)));

        Ok(conversation_groups)
    }
    
    /// Organize conversations by status
    fn organize_by_status(&self, conversations: &[ConversationSummary]) -> Result<Vec<ConversationGroup>> {
        let mut status_groups: HashMap<ConversationStatus, Vec<ConversationSummary>> = HashMap::new();
        
        for conv in conversations {
            status_groups.entry(conv.status.clone()).or_insert_with(Vec::new).push(conv.clone());
        }
        
        let mut groups = Vec::new();
        let status_order = [
            (ConversationStatus::Active, "Active", 100),
            (ConversationStatus::Paused, "Paused", 90),
            (ConversationStatus::Completed, "Completed", 80),
            (ConversationStatus::Archived, "Archived", 70),
            (ConversationStatus::Summarizing, "Summarizing", 75),
        ];
        
        for (status, name, priority) in status_order {
            if let Some(mut convs) = status_groups.remove(&status) {
                convs.sort_by(|a, b| b.last_active.cmp(&a.last_active));
                let group_id = format!("status_{:?}", status).to_lowercase();
                groups.push(self.create_group(&group_id, name, convs, priority)?);
            }
        }
        
        Ok(groups)
    }
    
    /// Organize conversations by clusters
    fn organize_by_clusters(
        &self,
        conversations: &[ConversationSummary],
        clusters: Option<&[ConversationCluster]>,
    ) -> Result<Vec<ConversationGroup>> {
        let mut groups = Vec::new();
        
        if let Some(clusters) = clusters {
            let conversation_map: HashMap<Uuid, &ConversationSummary> = conversations
                .iter()
                .map(|conv| (conv.id, conv))
                .collect();
            
            // Sort clusters by cohesion score (highest first)
            let mut sorted_clusters: Vec<&ConversationCluster> = clusters.iter().collect();
            sorted_clusters.sort_by(|a, b| b.cohesion_score.partial_cmp(&a.cohesion_score).unwrap_or(std::cmp::Ordering::Equal));
            
            for (index, cluster) in sorted_clusters.iter().enumerate() {
                let cluster_conversations: Vec<ConversationSummary> = cluster
                    .conversation_ids
                    .iter()
                    .filter_map(|id| conversation_map.get(id))
                    .map(|conv| (*conv).clone())
                    .collect();
                
                if !cluster_conversations.is_empty() {
                    let group_id = format!("cluster_{}", cluster.id);
                    
                    // Create enhanced group with cluster metadata
                    let mut group = self.create_group(&group_id, &cluster.title, cluster_conversations, 50 - index as i32)?;
                    
                    // Enhance metadata with cluster information
                    group.metadata.avg_success_rate = Some(cluster.cohesion_score);
                    group.metadata.statistics.common_tags = cluster.common_tags.clone();
                    
                    // Set time range from cluster
                    group.metadata.last_activity = Some(cluster.time_range.1);
                    
                    // Set priority based on cohesion score (higher cohesion = higher priority)
                    group.priority = (100.0 - cluster.cohesion_score * 100.0) as i32;
                    
                    groups.push(group);
                }
            }
            
            // Add unclustered conversations
            let clustered_ids: std::collections::HashSet<Uuid> = clusters
                .iter()
                .flat_map(|c| &c.conversation_ids)
                .copied()
                .collect();
            
            let unclustered: Vec<ConversationSummary> = conversations
                .iter()
                .filter(|conv| !clustered_ids.contains(&conv.id))
                .cloned()
                .collect();
            
            if !unclustered.is_empty() {
                groups.push(self.create_group("unclustered", "Unclustered", unclustered, 10)?);
            }
        } else {
            // No clusters available, create single group
            groups.push(self.create_group("all", "All Conversations", conversations.to_vec(), 50)?);
        }
        
        Ok(groups)
    }
    
    /// Organize conversations by tags
    fn organize_by_tags(&self, conversations: &[ConversationSummary]) -> Result<Vec<ConversationGroup>> {
        let mut tag_groups: HashMap<String, Vec<ConversationSummary>> = HashMap::new();
        let mut untagged = Vec::new();
        
        for conv in conversations {
            if conv.tags.is_empty() {
                untagged.push(conv.clone());
            } else {
                for tag in &conv.tags {
                    tag_groups.entry(tag.clone()).or_insert_with(Vec::new).push(conv.clone());
                }
            }
        }
        
        let mut groups = Vec::new();
        
        // Sort tags by frequency
        let mut tag_counts: Vec<(String, usize)> = tag_groups
            .iter()
            .map(|(tag, convs)| (tag.clone(), convs.len()))
            .collect();
        tag_counts.sort_by(|a, b| b.1.cmp(&a.1));
        
        for (tag, _) in tag_counts {
            if let Some(mut convs) = tag_groups.remove(&tag) {
                convs.sort_by(|a, b| b.last_active.cmp(&a.last_active));
                let group_id = format!("tag_{}", tag.to_lowercase().replace(' ', "_"));
                groups.push(self.create_group(&group_id, &tag, convs, 50)?);
            }
        }
        
        if !untagged.is_empty() {
            groups.push(self.create_group("untagged", "Untagged", untagged, 10)?);
        }
        
        Ok(groups)
    }
    
    /// Organize conversations by success rate
    fn organize_by_success(&self, conversations: &[ConversationSummary]) -> Result<Vec<ConversationGroup>> {
        // Note: This would require success rate data in ConversationSummary
        // For now, organize by completion status as a proxy
        let mut completed = Vec::new();
        let mut active = Vec::new();
        let mut other = Vec::new();
        
        for conv in conversations {
            match conv.status {
                ConversationStatus::Completed => completed.push(conv.clone()),
                ConversationStatus::Active => active.push(conv.clone()),
                _ => other.push(conv.clone()),
            }
        }
        
        let mut groups = Vec::new();
        
        if !completed.is_empty() {
            groups.push(self.create_group("successful", "Successful", completed, 100)?);
        }
        if !active.is_empty() {
            groups.push(self.create_group("in_progress", "In Progress", active, 80)?);
        }
        if !other.is_empty() {
            groups.push(self.create_group("other", "Other", other, 60)?);
        }
        
        Ok(groups)
    }
    
    /// Custom organization mode
    fn organize_custom(&self, conversations: &[ConversationSummary], _mode: &str) -> Result<Vec<ConversationGroup>> {
        // Placeholder for custom organization logic
        // Could be implemented based on user-defined rules
        self.organize_by_recency(conversations)
    }
    
    /// Create a conversation group
    fn create_group(
        &self,
        id: &str,
        name: &str,
        conversations: Vec<ConversationSummary>,
        priority: i32,
    ) -> Result<ConversationGroup> {
        let count = conversations.len();
        let total_messages: usize = conversations.iter().map(|c| c.message_count).sum();
        let active_count = conversations.iter().filter(|c| c.status == ConversationStatus::Active).count();
        let completed_count = conversations.iter().filter(|c| c.status == ConversationStatus::Completed).count();
        
        let avg_length = if count > 0 {
            total_messages as f64 / count as f64
        } else {
            0.0
        };
        
        let last_activity = conversations.iter().map(|c| c.last_active).max();
        
        // Extract common tags
        let mut tag_counts: HashMap<String, usize> = HashMap::new();
        for conv in &conversations {
            for tag in &conv.tags {
                *tag_counts.entry(tag.clone()).or_insert(0) += 1;
            }
        }
        
        let mut common_tags: Vec<(String, usize)> = tag_counts.into_iter().collect();
        common_tags.sort_by(|a, b| b.1.cmp(&a.1));
        let common_tags: Vec<String> = common_tags.into_iter().take(3).map(|(tag, _)| tag).collect();
        
        let statistics = GroupStatistics {
            total_messages,
            active_count,
            completed_count,
            avg_length,
            common_tags,
        };
        
        let metadata = GroupMetadata {
            count,
            avg_success_rate: None, // Would need success rate calculation
            last_activity,
            statistics,
        };
        
        let conversation_items: Vec<ConversationItem> = conversations
            .into_iter()
            .take(self.config.max_conversations_per_group)
            .map(|conv| self.create_conversation_item(conv))
            .collect();
        
        let expanded = self.expanded_groups.contains(id) || 
                      (self.config.auto_expand_active && active_count > 0);
        
        Ok(ConversationGroup {
            id: id.to_string(),
            name: name.to_string(),
            description: None,
            conversations: conversation_items,
            metadata,
            expanded,
            priority,
        })
    }
    
    /// Create a conversation item for display
    fn create_conversation_item(&self, summary: ConversationSummary) -> ConversationItem {
        let display = self.create_conversation_display(&summary);
        let preview = if self.config.show_previews {
            Some(format!("{} messages", summary.message_count))
        } else {
            None
        };
        
        ConversationItem {
            selected: self.selected_conversation == Some(summary.id),
            favorite: false, // Would need favorites tracking
            summary,
            display,
            preview,
        }
    }
    
    /// Create display metadata for a conversation
    fn create_conversation_display(&self, summary: &ConversationSummary) -> ConversationDisplay {
        let title = if summary.title.len() > 50 {
            format!("{}...", &summary.title[..47])
        } else {
            summary.title.clone()
        };
        
        let status_indicator = match summary.status {
            ConversationStatus::Active => StatusIndicator::Active,
            ConversationStatus::Paused => StatusIndicator::Paused,
            ConversationStatus::Completed => StatusIndicator::Completed,
            ConversationStatus::Archived => StatusIndicator::Archived,
            ConversationStatus::Summarizing => StatusIndicator::Active,
        };
        
        let time_display = self.format_relative_time(summary.last_active);
        
        let mut indicators = Vec::new();
        
        if summary.has_branches {
            indicators.push(VisualIndicator {
                indicator_type: IndicatorType::Branch,
                display: "🌿".to_string(),
                tooltip: Some("Has branches".to_string()),
                style: None,
            });
        }
        
        if summary.has_checkpoints {
            indicators.push(VisualIndicator {
                indicator_type: IndicatorType::Checkpoint,
                display: "📍".to_string(),
                tooltip: Some("Has checkpoints".to_string()),
                style: None,
            });
        }
        
        for tag in &summary.tags {
            indicators.push(VisualIndicator {
                indicator_type: IndicatorType::Tag,
                display: tag.clone(),
                tooltip: Some(format!("Tag: {}", tag)),
                style: Some("tag".to_string()),
            });
        }
        
        ConversationDisplay {
            title,
            status_indicator,
            time_display,
            progress: None,
            indicators,
            color_theme: None,
        }
    }
    
    /// Format relative time display
    fn format_relative_time(&self, time: DateTime<Utc>) -> String {
        let now = Utc::now();
        let duration = now.signed_duration_since(time);
        
        if duration.num_minutes() < 1 {
            "Just now".to_string()
        } else if duration.num_minutes() < 60 {
            format!("{}m ago", duration.num_minutes())
        } else if duration.num_hours() < 24 {
            format!("{}h ago", duration.num_hours())
        } else if duration.num_days() < 7 {
            format!("{}d ago", duration.num_days())
        } else {
            time.format("%m/%d").to_string()
        }
    }
    
    /// Set organization mode
    pub fn set_organization_mode(&mut self, mode: OrganizationMode) {
        self.organization_mode = mode;
    }
    
    /// Set search query
    pub fn set_search_query(&mut self, query: Option<String>) {
        self.search_query = query;
    }
    
    /// Update filters
    pub fn update_filters(&mut self, filters: SidebarFilters) {
        self.filters = filters;
    }
    
    /// Toggle group expansion
    pub fn toggle_group(&mut self, group_id: &str) {
        if self.expanded_groups.contains(group_id) {
            self.expanded_groups.remove(group_id);
        } else {
            self.expanded_groups.insert(group_id.to_string());
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
            CheckpointSuggestionAction::RefreshSuggestions { conversation_id } => {
                // Trigger refresh - this would be handled by the app
                None
            },
            CheckpointSuggestionAction::JumpToMessage { conversation_id, message_id } => {
                Some(SidebarAction::ShowCheckpointDetails(conversation_id, message_id))
            },
        }
    }

    pub fn show(&mut self, ctx: &Context, app_state: &mut AppState, theme: &AppTheme, conversation_service: Option<Arc<ConversationService>>, app_event_sender: UnboundedSender<AppEvent>, sagitta_config: Arc<tokio::sync::Mutex<SagittaCodeConfig>>) {
        // Phase 10: Auto-save state periodically
        self.auto_save_state(sagitta_config);
        
        // Phase 10: Load accessibility settings from config (would need app reference)
        // For now, use default values - this would be improved with proper config access
        self.accessibility_enabled = true; // app.config.conversation.sidebar.enable_accessibility;
        self.color_blind_friendly = false; // app.config.conversation.sidebar.color_blind_friendly;
        
        // Use theme's side panel frame for consistent styling
        let panel_frame = theme.side_panel_frame();

        // Get screen size for responsive constraints
        let screen_size = ctx.screen_rect().size();
        let is_small_screen = self.config.responsive.enabled && 
            screen_size.x <= self.config.responsive.small_screen_breakpoint;
        
        // Responsive width constraints
        let (default_width, min_width, max_width) = if is_small_screen {
            (280.0, 200.0, 360.0)
        } else {
            (320.0, 240.0, 500.0)
        };

        egui::SidePanel::left("conversation_sidebar")
            .frame(panel_frame)
            .default_width(default_width)
            .min_width(min_width)
            .max_width(max_width)
            .resizable(true)
            .show(ctx, |ui| {
                // Apply theme colors to the UI using correct egui visuals fields
                ui.style_mut().visuals.panel_fill = theme.panel_background();
                ui.style_mut().visuals.window_fill = theme.panel_background();
                ui.style_mut().visuals.extreme_bg_color = theme.input_background();
                // Note: text_color is handled per-widget, not globally
                
                // Wrap entire sidebar content in ScrollArea for comprehensive scrolling
                ScrollArea::vertical()
                    .auto_shrink(false)
                    .max_height(ui.available_height())
                    .show(ui, |ui| {
                        // Set consistent width for all content
                        ui.set_min_width(ui.available_width());
                        
                        self.render_header(ui, app_state, theme);
                        
                        // Responsive spacing
                        let spacing = if is_small_screen && self.config.responsive.compact_mode.reduced_spacing {
                            2.0
                        } else {
                            4.0
                        };
                        ui.add_space(spacing);
                        
                        self.render_search_bar(ui, app_state);
                        
                        ui.add_space(spacing);

                        if app_state.conversation_data_loading {
                            ui.centered_and_justified(|ui| {
                                ui.spinner();
                                ui.colored_label(theme.text_color(), "Loading conversations...");
                            });
                            return;
                        }

                        // Phase 10: Performance optimization with virtual scrolling
                        let performance_config = &crate::config::types::SidebarPerformanceConfig::default(); // &app.config.conversation.sidebar.performance;
                        let total_conversations = app_state.conversation_list.len();
                        let use_virtual_scrolling = performance_config.enable_virtual_scrolling && 
                            total_conversations > performance_config.virtual_scrolling_threshold;

                        // Show existing conversations with reduced opacity
                        match self.organize_conversations(
                            &app_state.conversation_list,
                            Some(&self.clusters),
                            &app_state.workspaces,
                            app_state.active_workspace_id,
                        ) {
                            Ok(organized_data) => {
                                if self.show_branch_suggestions {
                                    if let Some(conversation_id) = app_state.current_conversation_id {
                                        if let Ok(Some(action)) = self.branch_suggestions_ui.render(ui, conversation_id, theme) {
                                            if let Some(sidebar_action) = self.handle_branch_suggestion_action(action) {
                                                self.pending_action = Some(sidebar_action);
                                            }
                                        }
                                    }
                                }
                                
                                // Display organized groups with responsive spacing
                                for (index, group) in organized_data.groups.iter().enumerate() {
                                    self.render_conversation_group(ui, group, app_state, theme);
                                    
                                    // Only add space between groups, not after the last one
                                    if index < organized_data.groups.len() - 1 {
                                        ui.add_space(if is_small_screen { 1.0 } else { 2.0 });
                                    }
                                }
                                
                                // Show organization info with responsive spacing
                                ui.add_space(if is_small_screen { 4.0 } else { 6.0 });
                                ui.separator();
                                ui.add_space(if is_small_screen { 1.0 } else { 2.0 });
                                ui.colored_label(theme.hint_text_color(), format!("Showing {} of {} conversations", organized_data.filtered_count, organized_data.total_count));
                            },
                            Err(e) => {
                                log::error!("Failed to organize conversations: {}", e);
                                // Fallback to simple list
                                self.render_simple_conversation_list(ui, app_state, theme);
                            }
                        }
                        
                        // Show checkpoint suggestions if enabled and available
                        if self.show_checkpoint_suggestions {
                            if let Some(conversation_id) = app_state.current_conversation_id {
                                ui.add_space(if is_small_screen { 4.0 } else { 6.0 });
                                ui.separator();
                                ui.add_space(if is_small_screen { 1.0 } else { 2.0 });
                                
                                match self.checkpoint_suggestions_ui.render(ui, conversation_id, theme) {
                                    Ok(Some(action)) => {
                                        if let Some(sidebar_action) = self.handle_checkpoint_suggestion_action(action) {
                                            self.pending_action = Some(sidebar_action);
                                        }
                                    },
                                    Ok(None) => {
                                        // No action taken
                                    },
                                    Err(e) => {
                                        log::error!("Failed to render checkpoint suggestions: {}", e);
                                    }
                                }
                            }
                        }
                        
                        // Add bottom padding to ensure content doesn't get cut off
                        ui.add_space(8.0);
                    });
            });
        
        self.handle_sidebar_actions(app_state, ctx, conversation_service, app_event_sender);
    }

    // Render the header with organization mode selector
    fn render_header(&mut self, ui: &mut Ui, app_state: &mut AppState, theme: &AppTheme) {
        // Get screen size for responsive layout
        let screen_size = ui.ctx().screen_rect().size();
        let is_small_screen = self.config.responsive.enabled && 
            screen_size.x <= self.config.responsive.small_screen_breakpoint;
        
        ui.horizontal(|ui| {
            if is_small_screen && self.config.responsive.compact_mode.abbreviated_labels {
                ui.colored_label(theme.text_color(), "💬"); // Just icon for small screens
            } else {
                ui.colored_label(theme.text_color(), egui::RichText::new("💬 Conversations").heading());
            }
            
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let button_fn = if is_small_screen && self.config.responsive.compact_mode.small_buttons {
                    |ui: &mut Ui, text: &str, theme: &AppTheme| {
                        ui.add(
                            egui::Button::new(egui::RichText::new(text).small().color(theme.button_text_color()))
                                .fill(theme.button_background())
                                .stroke(egui::Stroke::new(1.0, theme.border_color()))
                        )
                    }
                } else {
                    |ui: &mut Ui, text: &str, theme: &AppTheme| {
                        ui.add(
                            egui::Button::new(egui::RichText::new(text).color(theme.button_text_color()))
                                .fill(theme.button_background())
                                .stroke(egui::Stroke::new(1.0, theme.border_color()))
                        )
                    }
                };
                
                if button_fn(ui, "🔄", theme).on_hover_text("Refresh conversations").clicked() {
                    // Trigger refresh action
                    self.pending_action = Some(SidebarAction::RefreshConversations);
                }
                if button_fn(ui, "➕", theme).on_hover_text("New conversation").clicked() {
                    self.pending_action = Some(SidebarAction::CreateNewConversation);
                }
                // Branch suggestions toggle
                let branch_icon = if self.show_branch_suggestions { "🌳" } else { "🌿" };
                if button_fn(ui, branch_icon, theme).on_hover_text("Toggle branch suggestions").clicked() {
                    self.toggle_branch_suggestions();
                }
                
                // Checkpoint suggestions toggle
                let checkpoint_icon = if self.show_checkpoint_suggestions { "📍" } else { "📌" };
                if button_fn(ui, checkpoint_icon, theme).on_hover_text("Toggle checkpoint suggestions").clicked() {
                    self.toggle_checkpoint_suggestions();
                }
            });
        });
        
        // Responsive spacing
        let spacing = if is_small_screen && self.config.responsive.compact_mode.reduced_spacing {
            1.0
        } else {
            2.0
        };
        ui.add_space(spacing);
        
        // Breadcrumb navigation for cluster mode - more compact
        if self.organization_mode == OrganizationMode::Clusters {
            ui.horizontal(|ui| {
                ui.colored_label(theme.hint_text_color(), "📍");
                
                // Always show "All" as root
                if ui.add(
                    egui::Button::new(egui::RichText::new("All").small().color(theme.accent_color()))
                        .fill(theme.button_background())
                        .stroke(egui::Stroke::new(1.0, theme.border_color()))
                ).clicked() {
                    // Clear all expanded groups to show all clusters
                    self.expanded_groups.clear();
                }
                
                ui.colored_label(theme.hint_text_color(), "→");
                
                // Show "Clusters" as current level
                ui.colored_label(theme.text_color(), "Clusters");
                
                // Show expanded cluster name if any
                let expanded_cluster_names: Vec<String> = self.expanded_groups
                    .iter()
                    .filter(|group_id| group_id.starts_with("cluster_"))
                    .map(|group_id| {
                        // Extract cluster name from group_id (remove "cluster_" prefix)
                        group_id.strip_prefix("cluster_").unwrap_or(group_id).to_string()
                    })
                    .collect();
                
                if !expanded_cluster_names.is_empty() {
                    ui.colored_label(theme.hint_text_color(), "→");
                    for (i, cluster_name) in expanded_cluster_names.iter().enumerate() {
                        if i > 0 {
                            ui.colored_label(theme.hint_text_color(), ",");
                        }
                        ui.colored_label(theme.accent_color(), format!("📂 {}", cluster_name));
                    }
                }
            });
            
            ui.add_space(spacing);
        }
        
        // Organization mode selector - more compact
        ui.horizontal(|ui| {
            if is_small_screen && self.config.responsive.compact_mode.abbreviated_labels {
                ui.colored_label(theme.hint_text_color(), "📋");
            } else {
                ui.colored_label(theme.hint_text_color(), "📋 Organize by:");
            }
            
            let combo_width = if is_small_screen { 120.0 } else { 150.0 };
            ComboBox::from_id_source("organization_mode")
                .selected_text(self.organization_mode_display_name())
                .width(if is_small_screen { 120.0 } else { 150.0 })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.organization_mode, OrganizationMode::Recency, "📅 Recency");
                    ui.selectable_value(&mut self.organization_mode, OrganizationMode::Project, "📁 Project");
                    ui.selectable_value(&mut self.organization_mode, OrganizationMode::Status, "📊 Status");
                    ui.selectable_value(&mut self.organization_mode, OrganizationMode::Clusters, "🔗 Clusters");
                    ui.selectable_value(&mut self.organization_mode, OrganizationMode::Tags, "🏷️ Tags");
                    ui.selectable_value(&mut self.organization_mode, OrganizationMode::Success, "✅ Success");
                });
        });

        // Workspace selector - only shown when in Project mode, more compact
        if self.organization_mode == OrganizationMode::Project {
            ui.add_space(2.0); // Reduced spacing
            ui.horizontal(|ui| {
                if is_small_screen {
                    ui.small("📁");
                } else {
                    ui.small("📁 Workspace:");
                }
                
                let mut selected_workspace = app_state.active_workspace_id;
                ComboBox::from_id_source("workspace_selector")
                    .selected_text(
                        selected_workspace
                            .and_then(|id| app_state.workspaces.iter().find(|ws| ws.id == id))
                            .map_or("All Workspaces", |ws| &ws.name),
                    )
                    .width(if is_small_screen { 100.0 } else { 130.0 })
                    .show_ui(ui, |ui| {
                        // Option for all workspaces
                        ui.selectable_value(&mut selected_workspace, None, "All Workspaces");

                        // Options for each workspace
                        for workspace in &app_state.workspaces {
                            ui.selectable_value(
                                &mut selected_workspace,
                                Some(workspace.id),
                                &workspace.name,
                            );
                        }
                    });

                if selected_workspace != app_state.active_workspace_id {
                    if let Some(id) = selected_workspace {
                        self.pending_action = Some(SidebarAction::SetWorkspace(id));
                    } else {
                        // Handle 'All Workspaces' selection
                        app_state.active_workspace_id = None;
                    }
                }
            });
        }
    }

    // Render search bar and filters
    fn render_search_bar(&mut self, ui: &mut Ui, _app_state: &mut AppState) {
        // Get screen size for responsive layout
        let screen_size = ui.ctx().screen_rect().size();
        let is_small_screen = self.config.responsive.enabled && 
            screen_size.x <= self.config.responsive.small_screen_breakpoint;
        
        // Get theme from app_state for consistent styling
        let theme = AppTheme::default(); // Simplified for now
        
        ui.horizontal(|ui| {
            ui.colored_label(theme.hint_text_color(), "🔍");
            
            let hint_text = if is_small_screen && self.config.responsive.compact_mode.abbreviated_labels {
                "Search..."
            } else {
                "Search conversations..."
            };
            
            let response = ui.add(
                TextEdit::singleline(&mut self.search_input)
                    .hint_text(hint_text)
                    .desired_width(f32::INFINITY)
                    .text_color(theme.text_color())
            );
            
            // Phase 10: Debounced search for performance
            if response.changed() {
                let debounce_ms = 300; // In a real app, this would come from config
                
                // Store search input in local variable to avoid borrowing issues
                let search_input = self.search_input.clone();
                if !self.should_debounce_search(&search_input, debounce_ms) {
                    if self.search_input.trim().is_empty() {
                        self.search_query = None;
                    } else {
                        self.search_query = Some(self.search_input.clone());
                    }
                    
                    if self.accessibility_enabled {
                        self.announce_to_screen_reader(format!("Searching for: {}", self.search_input));
                    }
                }
            }
            
            // Clear search button - only show if there's text in the input buffer
            if !self.search_input.is_empty() {
                let button_fn = if is_small_screen && self.config.responsive.compact_mode.small_buttons {
                    |ui: &mut Ui, text: &str| ui.small_button(text)
                } else {
                    |ui: &mut Ui, text: &str| ui.button(text)
                };
                
                if button_fn(ui, "✖").on_hover_text("Clear search").clicked() {
                    self.search_input.clear();
                    self.search_query = None;
                    self.search_debounce_timer = None;
                    self.last_search_query = None;
                    
                    if self.accessibility_enabled {
                        self.announce_to_screen_reader("Search cleared".to_string());
                    }
                }
            }
            
            // Phase 10: Show debounce indicator
            if self.search_debounce_timer.is_some() {
                ui.label(RichText::new("⏱").size(10.0).color(ui.style().visuals.weak_text_color()));
            }
        });
    }

    // Render filter controls
    fn render_filters(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label("Status:");
            ui.checkbox(&mut self.filter_active, "Active");
            ui.checkbox(&mut self.filter_completed, "Completed");
            ui.checkbox(&mut self.filter_archived, "Archived");
        });
        
        ui.horizontal(|ui| {
            ui.label("Features:");
            ui.checkbox(&mut self.filters.branches_only, "Has branches");
            ui.checkbox(&mut self.filters.checkpoints_only, "Has checkpoints");
            ui.checkbox(&mut self.filters.favorites_only, "Favorites only");
        });
    }

    // Render a conversation group
    fn render_conversation_group(&mut self, ui: &mut Ui, group: &ConversationGroup, app_state: &mut AppState, theme: &AppTheme) {
        let group_id = group.id.clone();
        let is_expanded = self.expanded_groups.contains(&group_id);
        
        // Group header with expand/collapse - use compact layout
        ui.horizontal(|ui| {
            let expand_icon = if is_expanded { "▼" } else { "▶" };
            let header_text = format!("{} {} ({})", expand_icon, group.name, group.metadata.count);
            
            let mut header_response = ui.add(
                egui::Button::new(egui::RichText::new(header_text).color(theme.text_color()))
                    .fill(theme.button_background())
                    .stroke(egui::Stroke::new(1.0, theme.border_color()))
            );
            
            // Add cohesion score tooltip for cluster groups
            if group.id.starts_with("cluster_") {
                if let Some(cohesion_score) = group.metadata.avg_success_rate {
                    header_response = header_response.on_hover_text(format!(
                        "Cluster Cohesion: {:.1}%\nCommon tags: {}\nClick to expand/collapse",
                        cohesion_score * 100.0,
                        group.metadata.statistics.common_tags.join(", ")
                    ));
                }
            }
            
            if header_response.clicked() {
                self.toggle_group(&group_id);
            }
            
            // Show group statistics in a more compact way
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Show cohesion score for clusters
                if group.id.starts_with("cluster_") {
                    if let Some(cohesion_score) = group.metadata.avg_success_rate {
                        let cohesion_color = if cohesion_score > 0.8 {
                            theme.success_color() // Green for high cohesion
                        } else if cohesion_score > 0.6 {
                            theme.warning_color() // Orange for medium cohesion
                        } else {
                            theme.error_color() // Red for low cohesion
                        };
                        
                        ui.colored_label(cohesion_color, format!("{:.0}%", cohesion_score * 100.0));
                    }
                } else {
                    // Show average success rate for non-cluster groups
                    if let Some(success_rate) = group.metadata.avg_success_rate {
                        ui.colored_label(theme.hint_text_color(), format!("📈{:.0}%", success_rate * 100.0));
                    }
                }
                
                // Compact status indicators
                if group.metadata.statistics.completed_count > 0 {
                    ui.colored_label(theme.success_color(), format!("✓{}", group.metadata.statistics.completed_count));
                }
                if group.metadata.statistics.active_count > 0 {
                    ui.colored_label(theme.accent_color(), format!("●{}", group.metadata.statistics.active_count));
                }
            });
        });
        
        // Show conversations in group if expanded
        if is_expanded {
            ui.indent(&group_id, |ui| {
                for conv_item in &group.conversations {
                    self.render_conversation_item(ui, conv_item, app_state, theme);
                }
            });
        }
        
        // Minimal spacing after group
        ui.add_space(2.0);
    }

    // Render a single conversation item
    fn render_conversation_item(&mut self, ui: &mut Ui, conv_item: &ConversationItem, app_state: &mut AppState, theme: &AppTheme) {
        let is_current = app_state.current_conversation_id == Some(conv_item.summary.id);
        let is_editing = self.editing_conversation_id == Some(conv_item.summary.id);
        
        let item_response = ui.scope(|ui| {
            let available_width = ui.available_width();
            let button_width = 60.0; // Estimated width for edit/delete buttons
            let text_width = available_width - button_width;

            ui.horizontal(|ui| {
                // Status indicator
                let status_icon = match conv_item.display.status_indicator {
                    StatusIndicator::Active => "●",
                    StatusIndicator::Paused => "⏸️",
                    StatusIndicator::Completed => "✅",
                    StatusIndicator::Failed => "❌",
                    StatusIndicator::Archived => "📦",
                    StatusIndicator::Branched => "🌿",
                    StatusIndicator::Checkpointed => "📍",
                };
                
                if is_editing {
                    let response = ui.add(
                        TextEdit::singleline(&mut self.edit_buffer)
                            .desired_width(f32::INFINITY)
                            .text_color(theme.text_color())
                    );
                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.pending_action = Some(SidebarAction::RenameConversation(conv_item.summary.id, self.edit_buffer.clone()));
                        self.editing_conversation_id = None;
                    }
                } else {
                    // More compact label format
                    let label_text = format!("{} {}", status_icon, conv_item.display.title);
                    let label_color = if is_current { theme.accent_color() } else { theme.text_color() };
                    
                    let text_response = ui.add_sized(
                        [text_width, ui.text_style_height(&egui::TextStyle::Body)],
                        egui::SelectableLabel::new(is_current, egui::RichText::new(label_text).color(label_color))
                    ).on_hover_text(&conv_item.display.title);

                    if text_response.clicked() {
                        self.pending_action = Some(SidebarAction::SwitchToConversation(conv_item.summary.id));
                    }
                }

                if !is_editing {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // Show branch suggestion badge if available
                        if let Some(suggestions) = self.get_branch_suggestions(conv_item.summary.id) {
                            if !suggestions.is_empty() {
                                let suggestion_count = suggestions.len();
                                let highest_confidence = suggestions.iter()
                                    .map(|s| s.confidence)
                                    .fold(0.0f32, f32::max);
                                
                                let badge_color = if highest_confidence >= 0.8 {
                                    theme.success_color() // Green
                                } else if highest_confidence >= 0.6 {
                                    theme.warning_color() // Yellow/Orange
                                } else {
                                    theme.error_color() // Red/Orange
                                };
                                
                                if ui.add_sized(
                                    Vec2::new(16.0, 16.0), // Smaller badge size
                                    egui::Button::new(RichText::new("🌳").small().color(theme.text_color()))
                                        .fill(badge_color.gamma_multiply(0.3))
                                        .stroke(Stroke::new(1.0, badge_color))
                                ).on_hover_text(format!(
                                    "{} branch suggestion{}\nHighest confidence: {:.0}%\nClick to show suggestions",
                                    suggestion_count,
                                    if suggestion_count == 1 { "" } else { "s" },
                                    highest_confidence * 100.0
                                )).clicked() {
                                    self.show_branch_suggestions = true;
                                    self.pending_action = Some(SidebarAction::SwitchToConversation(conv_item.summary.id));
                                }
                            }
                        }
                        
                        // Compact action buttons with theme colors
                        if ui.add(
                            egui::Button::new(egui::RichText::new("🗑").small().color(theme.error_color()))
                                .fill(theme.button_background())
                                .stroke(egui::Stroke::new(1.0, theme.border_color()))
                        ).on_hover_text("Delete conversation").clicked() {
                            self.pending_action = Some(SidebarAction::RequestDeleteConversation(conv_item.summary.id));
                        }
                        if ui.add(
                            egui::Button::new(egui::RichText::new("✏").small().color(theme.accent_color()))
                                .fill(theme.button_background())
                                .stroke(egui::Stroke::new(1.0, theme.border_color()))
                        ).on_hover_text("Rename conversation").clicked() {
                            self.edit_buffer = conv_item.summary.title.clone();
                            self.editing_conversation_id = Some(conv_item.summary.id);
                        }
                    });
                }
            });
        }).response;

        // Show visual indicators in a more compact way
        if !conv_item.display.indicators.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(16.0); // Reduced indentation
                for indicator in &conv_item.display.indicators {
                    ui.colored_label(theme.hint_text_color(), indicator.display.clone());
                }
            });
        }

        // Show preview if available - more compact
        if let Some(ref preview) = conv_item.preview {
            ui.indent(format!("{}_preview", conv_item.summary.id), |ui| {
                ui.colored_label(theme.hint_text_color(), RichText::new(preview).small().weak());
            });
        }
        
        // Add minimal spacing between conversation items
        ui.add_space(1.0);
    }

    // Fallback simple conversation list
    fn render_simple_conversation_list(&mut self, ui: &mut Ui, app_state: &mut AppState, theme: &AppTheme) {
        for summary in &app_state.conversation_list {
            let is_current = app_state.current_conversation_id == Some(summary.id);
            let status_icon = get_status_icon(summary.status.clone());
            
            ui.horizontal(|ui| {
                let label_text = format!("{} {}", status_icon, summary.title);
                let label_color = if is_current { theme.accent_color() } else { theme.text_color() };
                
                if ui.add(
                    egui::SelectableLabel::new(is_current, egui::RichText::new(label_text).color(label_color))
                ).clicked() {
                    self.pending_action = Some(SidebarAction::SwitchToConversation(summary.id));
                }
            });
        }
    }

    // Handle sidebar actions
    fn handle_sidebar_actions(&mut self, app_state: &mut AppState, _ctx: &egui::Context, conversation_service: Option<Arc<ConversationService>>, app_event_sender: UnboundedSender<AppEvent>) {
        if let Some(action) = self.pending_action.take() {
            // Clone the action for synchronous handling
            let action_clone = action.clone();
            
            if let Some(service) = conversation_service {
                let service = service.clone();
                let sender = app_event_sender.clone();
                tokio::spawn(async move {
                    let result = match action {
                        SidebarAction::SwitchToConversation(id) => {
                            // This action is handled synchronously and locally, but should be an event
                            Ok(())
                        },
                        SidebarAction::CreateNewConversation => {
                            log::info!("Executing: Create new conversation");
                            service.create_conversation("New Conversation".to_string()).await.map(|_|())
                        },
                        SidebarAction::RefreshConversations => {
                            log::info!("Executing: Refresh conversations");
                            service.refresh().await
                        },
                        SidebarAction::RequestDeleteConversation(id) => {
                            log::info!("Executing: Delete conversation {}", id);
                            // In a real app, you'd show a confirmation dialog first.
                            // For now, we delete directly.
                            service.delete_conversation(id).await
                        },
                        SidebarAction::RenameConversation(id, new_name) => {
                            log::info!("Executing: Rename conversation {} to '{}'", id, new_name);
                            service.rename_conversation(id, new_name).await
                        },
                        // Other actions are not implemented yet and will do nothing.
                        _ => {
                            log::warn!("Sidebar action {:?} is not implemented yet.", action);
                            Ok(())
                        }
                    };

                    if let Err(e) = result {
                        log::error!("Error handling sidebar action: {}", e);
                    } else {
                        // On success, request a refresh of the conversation list
                        if let Err(e) = sender.send(AppEvent::RefreshConversationList) {
                            log::error!("Failed to send refresh event: {}", e);
                        }
                    }
                });
            } else {
                log::warn!("Conversation service not available, cannot handle sidebar action.");
            }

            // Handle synchronous actions using the cloned action
            if let SidebarAction::SwitchToConversation(id) = action_clone {
                // Send event to trigger proper conversation switching with chat history loading
                if let Err(e) = app_event_sender.send(AppEvent::SwitchToConversation(id)) {
                    log::error!("Failed to send SwitchToConversation event: {}", e);
                } else {
                    log::info!("Sent SwitchToConversation event for conversation: {}", id);
                }
                
                // Also update the conversation title for immediate display
                if let Some(summary) = app_state.conversation_list.iter().find(|s| s.id == id) {
                    app_state.current_conversation_title = Some(summary.title.clone());
                }
            }
        }
    }

    // Helper method to get organization mode display name
    fn organization_mode_display_name(&self) -> &str {
        match self.organization_mode {
            OrganizationMode::Recency => "📅 Recency",
            OrganizationMode::Project => "📁 Project", 
            OrganizationMode::Status => "📊 Status",
            OrganizationMode::Clusters => "🔗 Clusters",
            OrganizationMode::Tags => "🏷️ Tags",
            OrganizationMode::Success => "✅ Success",
            OrganizationMode::Custom(ref name) => name,
        }
    }
    
    // Phase 10: Persistent state management methods
    
    /// Load persistent state from configuration
    pub fn load_persistent_state(&mut self, config: &SidebarPersistentConfig) {
        // Load organization mode
        self.organization_mode = match config.last_organization_mode.as_str() {
            "Recency" => OrganizationMode::Recency,
            "Project" => OrganizationMode::Project,
            "Status" => OrganizationMode::Status,
            "Clusters" => OrganizationMode::Clusters,
            "Tags" => OrganizationMode::Tags,
            "Success" => OrganizationMode::Success,
            custom => OrganizationMode::Custom(custom.to_string()),
        };
        
        // Load expanded groups
        self.expanded_groups = config.expanded_groups.iter().cloned().collect();
        
        // Load search query and initialize input buffer
        self.search_query = config.last_search_query.clone();
        self.search_input = self.search_query.clone().unwrap_or_default();
        
        // Load filter settings
        self.filters = SidebarFilters {
            project_types: config.filters.project_types.iter()
                .filter_map(|pt_str| match pt_str.as_str() {
                    "Unknown" => Some(ProjectType::Unknown),
                    "Rust" => Some(ProjectType::Rust),
                    "Python" => Some(ProjectType::Python),
                    "JavaScript" => Some(ProjectType::JavaScript),
                    "TypeScript" => Some(ProjectType::TypeScript),
                    "Go" => Some(ProjectType::Go),
                    "Ruby" => Some(ProjectType::Ruby),
                    "Markdown" => Some(ProjectType::Markdown),
                    "Yaml" => Some(ProjectType::Yaml),
                    "Html" => Some(ProjectType::Html),
                    _ => None,
                })
                .collect(),
            statuses: config.filters.statuses.iter()
                .filter_map(|s| match s.as_str() {
                    "Active" => Some(ConversationStatus::Active),
                    "Paused" => Some(ConversationStatus::Paused),
                    "Completed" => Some(ConversationStatus::Completed),
                    "Archived" => Some(ConversationStatus::Archived),
                    "Summarizing" => Some(ConversationStatus::Summarizing),
                    _ => None,
                })
                .collect(),
            tags: config.filters.tags.clone(),
            date_range: None, // Date ranges are not persisted for now
            min_messages: config.filters.min_messages,
            min_success_rate: config.filters.min_success_rate,
            favorites_only: config.filters.favorites_only,
            branches_only: config.filters.branches_only,
            checkpoints_only: config.filters.checkpoints_only,
        };
        
        // Load UI state
        self.show_filters = config.show_filters;
        self.show_branch_suggestions = config.show_branch_suggestions;
        self.show_checkpoint_suggestions = config.show_checkpoint_suggestions;
        
        // Load accessibility settings
        self.accessibility_enabled = config.enable_accessibility;
        self.color_blind_friendly = config.color_blind_friendly;
    }
    
    /// Save persistent state to configuration
    pub fn save_persistent_state(&mut self, app_config: &mut SagittaCodeConfig) -> Result<()> {
        let config = &mut app_config.conversation.sidebar;
        
        // Save organization mode
        config.last_organization_mode = match &self.organization_mode {
            OrganizationMode::Recency => "Recency".to_string(),
            OrganizationMode::Project => "Project".to_string(),
            OrganizationMode::Status => "Status".to_string(),
            OrganizationMode::Clusters => "Clusters".to_string(),
            OrganizationMode::Tags => "Tags".to_string(),
            OrganizationMode::Success => "Success".to_string(),
            OrganizationMode::Custom(name) => name.clone(),
        };
        
        // Save expanded groups
        config.expanded_groups = self.expanded_groups.iter().cloned().collect();
        
        // Save search query
        config.last_search_query = self.search_query.clone();
        
        // Save filter settings
        config.filters.project_types = self.filters.project_types.iter()
            .map(|pt| match pt {
                ProjectType::Unknown => "Unknown".to_string(),
                ProjectType::Rust => "Rust".to_string(),
                ProjectType::Python => "Python".to_string(),
                ProjectType::JavaScript => "JavaScript".to_string(),
                ProjectType::TypeScript => "TypeScript".to_string(),
                ProjectType::Go => "Go".to_string(),
                ProjectType::Ruby => "Ruby".to_string(),
                ProjectType::Markdown => "Markdown".to_string(),
                ProjectType::Yaml => "Yaml".to_string(),
                ProjectType::Html => "Html".to_string(),
            })
            .collect();
            
        config.filters.statuses = self.filters.statuses.iter()
            .map(|status| match status {
                ConversationStatus::Active => "Active".to_string(),
                ConversationStatus::Paused => "Paused".to_string(),
                ConversationStatus::Completed => "Completed".to_string(),
                ConversationStatus::Archived => "Archived".to_string(),
                ConversationStatus::Summarizing => "Summarizing".to_string(),
            })
            .collect();
            
        config.filters.tags = self.filters.tags.clone();
        config.filters.min_messages = self.filters.min_messages;
        config.filters.min_success_rate = self.filters.min_success_rate;
        config.filters.favorites_only = self.filters.favorites_only;
        config.filters.branches_only = self.filters.branches_only;
        config.filters.checkpoints_only = self.filters.checkpoints_only;
        
        // Save UI state
        config.show_filters = self.show_filters;
        config.show_branch_suggestions = self.show_branch_suggestions;
        config.show_checkpoint_suggestions = self.show_checkpoint_suggestions;
        
        // Save accessibility settings
        config.enable_accessibility = self.accessibility_enabled;
        config.color_blind_friendly = self.color_blind_friendly;
        
        // Save configuration to disk
        save_config(app_config)?;
        self.last_state_save = Some(Instant::now());
        
        Ok(())
    }
    
    /// Auto-save state if enough time has passed
    pub fn auto_save_state(&mut self, config: Arc<tokio::sync::Mutex<SagittaCodeConfig>>) {
        let should_save = match self.last_state_save {
            Some(last_save) => last_save.elapsed() > Duration::from_secs(30), // Auto-save every 30 seconds
            None => true, // First save
        };

        if should_save && self.config.persist_state {
            match config.try_lock() {
                Ok(mut config_guard) => {
                    if let Err(e) = self.save_persistent_state(&mut config_guard) {
                        log::error!("Failed to auto-save sidebar state: {}", e);
                    } else {
                        self.last_state_save = Some(Instant::now());
                    }
                },
                Err(_) => {
                    log::warn!("Failed to acquire config lock for auto-save");
                }
            }
        }
    }
    
    /// Get color-blind friendly color palette
    pub fn get_accessible_color(&self, base_color: Color32, color_type: &str) -> Color32 {
        if !self.color_blind_friendly {
            return base_color;
        }
        
        // Color-blind friendly palette (Viridis-inspired)
        match color_type {
            "success" => Color32::from_rgb(68, 1, 84),      // Dark purple
            "warning" => Color32::from_rgb(253, 231, 37),   // Bright yellow
            "error" => Color32::from_rgb(94, 201, 98),      // Green (counter-intuitive but accessible)
            "info" => Color32::from_rgb(33, 145, 140),      // Teal
            "primary" => Color32::from_rgb(59, 82, 139),    // Blue
            "secondary" => Color32::from_rgb(180, 180, 180), // Gray
            _ => base_color,
        }
    }
    
    /// Add screen reader announcement
    pub fn announce_to_screen_reader(&mut self, message: String) {
        if !self.accessibility_enabled {
            return;
        }
        
        // Limit announcements to prevent spam
        let now = Instant::now();
        if let Some(last_time) = self.last_announcement_time {
            if now.duration_since(last_time) < Duration::from_millis(500) {
                return;
            }
        }
        
        self.screen_reader_announcements.push(message);
        self.last_announcement_time = Some(now);
        
        // Keep only the last 5 announcements
        if self.screen_reader_announcements.len() > 5 {
            self.screen_reader_announcements.remove(0);
        }
    }
    
    /// Check if search should be debounced
    pub fn should_debounce_search(&mut self, query: &str, debounce_ms: u64) -> bool {
        let now = Instant::now();
        
        // If query changed, reset timer
        if self.last_search_query.as_ref() != Some(&query.to_string()) {
            self.search_debounce_timer = Some(now);
            self.last_search_query = Some(query.to_string());
            return true; // Debounce new query
        }
        
        // Check if enough time has passed
        if let Some(timer) = self.search_debounce_timer {
            if now.duration_since(timer) >= Duration::from_millis(debounce_ms) {
                self.search_debounce_timer = None;
                return false; // Don't debounce, execute search
            }
        }
        
        true // Still debouncing
    }
    
    /// Get virtual scrolling range for performance
    pub fn get_virtual_scroll_range(&self, total_items: usize, max_rendered: usize) -> (usize, usize) {
        let start = self.virtual_scroll_offset;
        let end = (start + max_rendered).min(total_items);
        (start, end)
    }
    
    /// Update virtual scroll offset
    pub fn update_virtual_scroll_offset(&mut self, new_offset: usize, total_items: usize, max_rendered: usize) {
        let max_offset = total_items.saturating_sub(max_rendered);
        self.virtual_scroll_offset = new_offset.min(max_offset);
    }
    
    /// Clear cached items when data changes
    pub fn invalidate_cache(&mut self) {
        self.cached_rendered_items = None;
    }
}

/// Create a test conversation for testing purposes
pub fn create_test_conversation(
    title: &str,
    status: ConversationStatus,
    project_type: Option<ProjectType>,
    workspace_id: Option<Uuid>,
) -> ConversationSummary {
    ConversationSummary {
        id: Uuid::new_v4(),
        title: title.to_string(),
        last_active: Utc::now(),
        status,
        project_name: project_type.map(|p| format!("{:?} Project", p)),
        workspace_id,
        tags: vec!["test".to_string()], // Add the "test" tag that the test expects
        ..Default::default()
    }
}

fn render_conversation_list_item(
    ui: &mut Ui,
    conv_item: &DisplayConversationItem,
    app_state: &mut AppState,
    _theme: &AppTheme, // theme might not be needed
    is_current: bool,
    is_editing: bool,
    edit_buffer: &mut String,
    ctx: &egui::Context,
) {
    let status_icon = get_status_icon(conv_item.summary.status.clone());
    ui.horizontal(|ui| {
        if is_editing && is_current {
            let response = ui.add(TextEdit::singleline(edit_buffer)
                .desired_width(f32::INFINITY)
                .min_size(Vec2::new(ui.available_width() * 0.8, 0.0))); // Ensure it takes available width
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                app_state.sidebar_action = Some(SidebarAction::RenameConversation(conv_item.summary.id, edit_buffer.clone()));
                app_state.editing_conversation_id = None;
            }
            // Request focus on the TextEdit when editing starts
            if response.changed() { // Or a more direct way to check if this is the first frame of editing
                ctx.memory_mut(|mem| mem.request_focus(response.id));
            }

        } else {
            let label_text = format!("{} {} {}", status_icon, conv_item.display.title, conv_item.display.time_display);
            if ui.selectable_label(is_current, label_text).on_hover_text(&conv_item.display.title).clicked() {
                app_state.switch_to_conversation(conv_item.summary.id);
            }
        }

        if !(is_editing && is_current) {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("🗑").on_hover_text("Delete Conversation").clicked() {
                    app_state.sidebar_action = Some(SidebarAction::RequestDeleteConversation(conv_item.summary.id));
                }
                if ui.button("✏").on_hover_text("Rename Conversation").clicked() {
                    *edit_buffer = conv_item.summary.title.clone();
                    app_state.editing_conversation_id = Some(conv_item.summary.id);
                }
            });
        }
    });

    if !conv_item.display.indicators.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(20.0); 
            for indicator in &conv_item.display.indicators {
                ui.label(&indicator.display);
            }
        });
    }

    if let Some(ref preview) = conv_item.preview {
        ui.indent(format!("{}_preview", conv_item.summary.id), |ui| {
            ui.label(RichText::new(preview).small().weak());
        });
    }
}

fn render_cluster_item(
    ui: &mut Ui,
    cluster: &ConversationCluster,
    app_state: &mut AppState, 
    _theme: &AppTheme,
) {
    ui.push_id(format!("cluster_{}", cluster.id), |ui| {
        let header = RichText::new(format!("{} ({})", cluster.title, cluster.conversation_ids.len())).strong();
        egui::collapsing_header::CollapsingHeader::new(header) 
            .default_open(true)
            .show(ui, |ui_body| {
                for conv_id_in_cluster in &cluster.conversation_ids {
                    if let Some(summary) = app_state.conversation_list.iter().find(|s| s.id == *conv_id_in_cluster) {
                        let is_current_chat = app_state.current_conversation_id == Some(summary.id);
                        if ui_body.selectable_label(is_current_chat, &summary.title).on_hover_text(&summary.title).clicked() {
                            app_state.switch_to_conversation(summary.id);
                        }
                    }
                }
            });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use uuid::Uuid;
    use crate::agent::conversation::types::{ConversationSummary, ProjectType};
    use crate::agent::state::types::ConversationStatus;
    use crate::agent::conversation::clustering::ConversationCluster;
    use crate::gui::conversation::branch_suggestions::BranchSuggestionAction;
    use crate::gui::conversation::checkpoint_suggestions::CheckpointSuggestionAction;

    fn create_test_conversations() -> Vec<ConversationSummary> {
        vec![
            create_test_conversation("Rust talk", ConversationStatus::Active, Some(ProjectType::Rust), None),
            create_test_conversation("JS progress", ConversationStatus::Completed, Some(ProjectType::JavaScript), None),
            create_test_conversation("Python script", ConversationStatus::Paused, Some(ProjectType::Python), None),
        ]
    }

    #[test]
    fn test_sidebar_creation() {
        let config = SidebarConfig::default();
        let sidebar = ConversationSidebar::new(config);
        
        assert_eq!(sidebar.organization_mode, OrganizationMode::Recency);
        assert!(sidebar.search_query.is_none());
        assert!(sidebar.expanded_groups.is_empty());
    }

    #[test]
    fn test_filter_application() {
        let config = SidebarConfig::default();
        let sidebar = ConversationSidebar::new(config);
        
        let conversations = vec![
            create_test_conversation("Active Conv", ConversationStatus::Active, Some(ProjectType::Rust), None),
            create_test_conversation("Completed Conv", ConversationStatus::Completed, Some(ProjectType::Python), None),
            create_test_conversation("Archived Conv", ConversationStatus::Archived, Some(ProjectType::JavaScript), None),
        ];
        
        // Test status filter
        let mut sidebar_with_filter = sidebar.clone();
        sidebar_with_filter.filters.statuses = vec![ConversationStatus::Active];
        let filtered = sidebar_with_filter.apply_filters(&conversations);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "Active Conv");
        
        // Test tag filter
        let mut sidebar_with_tag_filter = sidebar.clone();
        sidebar_with_tag_filter.filters.tags = vec!["test".to_string()];
        let filtered = sidebar_with_tag_filter.apply_filters(&conversations);
        assert_eq!(filtered.len(), 3); // All have "test" tag
        
        // Test non-matching tag filter
        sidebar_with_tag_filter.filters.tags = vec!["nonexistent".to_string()];
        let filtered = sidebar_with_tag_filter.apply_filters(&conversations);
        assert_eq!(filtered.len(), 0);
    }

    #[test]
    fn test_search_application() {
        let config = SidebarConfig::default();
        let mut sidebar = ConversationSidebar::new(config);
        
        let conversations = vec![
            create_test_conversation("Rust Programming Help", ConversationStatus::Active, Some(ProjectType::Rust), None),
            create_test_conversation("Python Data Analysis", ConversationStatus::Active, Some(ProjectType::Python), None),
            create_test_conversation("JavaScript Frontend", ConversationStatus::Active, Some(ProjectType::JavaScript), None),
        ];
        
        // Test title search
        sidebar.search_query = Some("rust".to_string());
        let results = sidebar.apply_search(&conversations, sidebar.search_query.as_ref().unwrap());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust Programming Help");
        
        // Test case insensitive search
        sidebar.search_query = Some("PYTHON".to_string());
        let results = sidebar.apply_search(&conversations, sidebar.search_query.as_ref().unwrap());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Python Data Analysis");
        
        // Test partial match
        sidebar.search_query = Some("data".to_string());
        let results = sidebar.apply_search(&conversations, sidebar.search_query.as_ref().unwrap());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Python Data Analysis");
        
        // Test no match
        sidebar.search_query = Some("nonexistent".to_string());
        let results = sidebar.apply_search(&conversations, sidebar.search_query.as_ref().unwrap());
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_organization_mode_switching() {
        let config = SidebarConfig::default();
        let mut sidebar = ConversationSidebar::new(config);
        
        assert_eq!(sidebar.organization_mode, OrganizationMode::Recency);
        
        sidebar.set_organization_mode(OrganizationMode::Project);
        assert_eq!(sidebar.organization_mode, OrganizationMode::Project);
        
        sidebar.set_organization_mode(OrganizationMode::Status);
        assert_eq!(sidebar.organization_mode, OrganizationMode::Status);
    }

    #[test]
    fn test_group_expansion() {
        let config = SidebarConfig::default();
        let mut sidebar = ConversationSidebar::new(config);
        
        let group_id = "test_group";
        
        // Initially not expanded
        assert!(!sidebar.expanded_groups.contains(group_id));
        
        // Toggle to expand
        sidebar.toggle_group(group_id);
        assert!(sidebar.expanded_groups.contains(group_id));
        
        // Toggle to collapse
        sidebar.toggle_group(group_id);
        assert!(!sidebar.expanded_groups.contains(group_id));
    }

    #[test]
    fn test_relative_time_formatting() {
        let config = SidebarConfig::default();
        let sidebar = ConversationSidebar::new(config);
        
        let now = Utc::now();
        let one_hour_ago = now - chrono::Duration::hours(1);
        let one_day_ago = now - chrono::Duration::days(1);
        let one_week_ago = now - chrono::Duration::weeks(1);
        
        let formatted = sidebar.format_relative_time(one_hour_ago);
        assert!(formatted.contains("h ago") || formatted.contains("hour"));
        
        let formatted = sidebar.format_relative_time(one_day_ago);
        assert!(formatted.contains("d ago") || formatted.contains("day"));
        
        let formatted = sidebar.format_relative_time(one_week_ago);
        // For times older than a week, it shows date format like "12/25"
        assert!(formatted.contains("/") || formatted.contains("week") || formatted.contains("7d") || formatted.contains("1w"));
    }

    // Tests for advanced features that need to be implemented
    #[test]
    fn test_context_aware_branching() {
        let config = SidebarConfig::default();
        let sidebar = ConversationSidebar::new(config);
        
        let conversations = vec![
            create_test_conversation("Main Conversation", ConversationStatus::Active, Some(ProjectType::Rust), None),
        ];
        
        // Test that conversations with branches are properly identified
        let mut conv_with_branches = conversations[0].clone();
        conv_with_branches.has_branches = true;
        
        let filtered = sidebar.apply_filters(&[conv_with_branches.clone()]);
        assert_eq!(filtered.len(), 1);
        
        // Test branch filter
        let mut sidebar_with_branch_filter = sidebar.clone();
        sidebar_with_branch_filter.filters.branches_only = true;
        let filtered = sidebar_with_branch_filter.apply_filters(&[conv_with_branches]);
        assert_eq!(filtered.len(), 1);
        
        // Test that conversations without branches are filtered out
        let filtered = sidebar_with_branch_filter.apply_filters(&conversations);
        assert_eq!(filtered.len(), 0);
    }

    #[test]
    fn test_smart_checkpoints() {
        let mut sidebar = ConversationSidebar::with_default_config();
        let conversation_id = Uuid::new_v4();
        
        // Test checkpoint suggestions management
        let checkpoint_suggestions = vec![
            CheckpointSuggestion {
                message_id: Uuid::new_v4(),
                importance: 0.9,
                reason: crate::agent::conversation::checkpoints::CheckpointReason::SuccessfulSolution,
                suggested_title: "Successful Implementation".to_string(),
                context: crate::agent::conversation::checkpoints::CheckpointContext {
                    relevant_messages: vec![],
                    trigger_keywords: vec!["success".to_string()],
                    conversation_phase: crate::agent::conversation::checkpoints::ConversationPhase::Implementation,
                    modified_files: vec![std::path::PathBuf::from("src/main.rs")],
                    executed_tools: vec!["cargo".to_string()],
                    success_indicators: vec!["working".to_string()],
                },
                restoration_value: 0.8,
            }
        ];
        
        // Test updating checkpoint suggestions
        sidebar.update_checkpoint_suggestions(conversation_id, checkpoint_suggestions.clone());
        assert_eq!(sidebar.get_checkpoint_suggestions(conversation_id).unwrap().len(), 1);
        
        // Test clearing checkpoint suggestions
        sidebar.clear_checkpoint_suggestions(conversation_id);
        assert!(sidebar.get_checkpoint_suggestions(conversation_id).is_none());
        
        // Test toggle functionality
        assert!(!sidebar.show_checkpoint_suggestions);
        sidebar.toggle_checkpoint_suggestions();
        assert!(sidebar.show_checkpoint_suggestions);
        
        // Test checkpoint suggestion actions
        let action = CheckpointSuggestionAction::CreateCheckpoint {
            conversation_id,
            suggestion: CheckpointSuggestion {
                message_id: Uuid::new_v4(),
                importance: 0.8,
                reason: crate::agent::conversation::checkpoints::CheckpointReason::SuccessfulSolution,
                suggested_title: "Successful Implementation".to_string(),
                context: crate::agent::conversation::checkpoints::CheckpointContext {
                    relevant_messages: vec![],
                    trigger_keywords: vec!["success".to_string()],
                    conversation_phase: crate::agent::conversation::checkpoints::ConversationPhase::Implementation,
                    modified_files: vec![std::path::PathBuf::from("src/main.rs")],
                    executed_tools: vec!["cargo".to_string()],
                    success_indicators: vec!["working".to_string()],
                },
                restoration_value: 0.9,
            },
        };
        
        let sidebar_action = sidebar.handle_checkpoint_suggestion_action(action);
        assert!(sidebar_action.is_some());
        
        if let Some(SidebarAction::CreateCheckpoint(conv_id, msg_id, title)) = sidebar_action {
            assert_eq!(conv_id, conversation_id);
            assert_eq!(title, "Successful Implementation");
        } else {
            panic!("Expected CreateCheckpoint action");
        }
    }

    #[test]
    fn test_semantic_clustering_organization() {
        let config = SidebarConfig::default();
        let sidebar = ConversationSidebar::new(config);
        
        let conversations = vec![
            create_test_conversation("Rust Error Handling", ConversationStatus::Active, Some(ProjectType::Rust), None),
            create_test_conversation("Python Error Handling", ConversationStatus::Active, Some(ProjectType::Python), None),
            create_test_conversation("JavaScript Async", ConversationStatus::Active, Some(ProjectType::JavaScript), None),
        ];
        
        // Create mock clusters
        let clusters = vec![
            ConversationCluster {
                id: Uuid::new_v4(),
                title: "Error Handling".to_string(),
                conversation_ids: vec![conversations[0].id, conversations[1].id],
                centroid: vec![0.1, 0.2, 0.3],
                cohesion_score: 0.85,
                common_tags: vec!["error".to_string(), "handling".to_string()],
                dominant_project_type: Some(ProjectType::Rust),
                time_range: (Utc::now() - chrono::Duration::days(7), Utc::now()),
            },
            ConversationCluster {
                id: Uuid::new_v4(),
                title: "Async Programming".to_string(),
                conversation_ids: vec![conversations[2].id],
                centroid: vec![0.4, 0.5, 0.6],
                cohesion_score: 0.75,
                common_tags: vec!["async".to_string(), "programming".to_string()],
                dominant_project_type: Some(ProjectType::JavaScript),
                time_range: (Utc::now() - chrono::Duration::days(3), Utc::now()),
            },
        ];
        
        let organized = sidebar.organize_by_clusters(&conversations, Some(&clusters)).unwrap();
        
        // Should have 2 cluster groups
        assert_eq!(organized.len(), 2);
        
        // Check first cluster
        let error_handling_group = organized.iter().find(|g| g.name == "Error Handling").unwrap();
        assert_eq!(error_handling_group.conversations.len(), 2);
        
        // Check second cluster
        let async_group = organized.iter().find(|g| g.name == "Async Programming").unwrap();
        assert_eq!(async_group.conversations.len(), 1);
    }

    #[test]
    fn test_conversation_analytics_integration() {
        let config = SidebarConfig::default();
        let sidebar = ConversationSidebar::new(config);
        
        let conversations = vec![
            create_test_conversation("High Success Conv", ConversationStatus::Completed, Some(ProjectType::Rust), None),
            create_test_conversation("Low Success Conv", ConversationStatus::Active, Some(ProjectType::Python), None),
            create_test_conversation("Failed Conv", ConversationStatus::Archived, Some(ProjectType::JavaScript), None),
        ];
        
        // Test success rate filtering
        let mut sidebar_with_success_filter = sidebar.clone();
        sidebar_with_success_filter.filters.min_success_rate = Some(0.8);
        
        // This test assumes we have a way to determine success rate
        // For now, we'll test the structure is in place
        let filtered = sidebar_with_success_filter.apply_filters(&conversations);
        // Since we don't have actual success rate calculation yet, this will pass all
        assert_eq!(filtered.len(), 3);
    }

    #[test]
    fn test_project_workspace_organization() {
        let config = SidebarConfig::default();
        let sidebar = ConversationSidebar::new(config);
        
        let workspace1_id = Uuid::new_v4();
        let workspaces = vec![WorkspaceSummary {
            id: workspace1_id,
            name: "Workspace 1".to_string(),
            ..Default::default()
        }];

        let conversations = vec![
            create_test_conversation("Rust Project Conv", ConversationStatus::Active, Some(ProjectType::Rust), Some(workspace1_id)),
            create_test_conversation("Python Project Conv", ConversationStatus::Active, Some(ProjectType::Python), None),
            create_test_conversation("Another Rust Conv", ConversationStatus::Completed, Some(ProjectType::Rust), Some(workspace1_id)),
            create_test_conversation("No Project Conv", ConversationStatus::Active, None, None),
        ];
        
        let organized = sidebar
            .organize_by_project(&conversations, &workspaces, None)
            .unwrap();
        
        // Should have 2 groups: Workspace 1, No Workspace
        assert_eq!(organized.len(), 2);
        
        // Check Workspace 1 group
        let rust_group = organized.iter().find(|g| g.name.contains("Workspace 1")).unwrap();
        assert_eq!(rust_group.conversations.len(), 2);
        
        // Check No Workspace group
        let no_project_group = organized.iter().find(|g| g.name == "No Workspace").unwrap();
        assert_eq!(no_project_group.conversations.len(), 2);
    }

    #[test]
    fn test_project_workspace_filtering() {
        let config = SidebarConfig::default();
        let sidebar = ConversationSidebar::new(config);

        let workspace1_id = Uuid::new_v4();
        let workspace2_id = Uuid::new_v4();

        let workspaces = vec![
            WorkspaceSummary {
                id: workspace1_id,
                name: "Workspace 1".to_string(),
                ..Default::default()
            },
            WorkspaceSummary {
                id: workspace2_id,
                name: "Workspace 2".to_string(),
                ..Default::default()
            },
        ];

        let mut conv1 = create_test_conversation("Conv 1", ConversationStatus::Active, None, Some(workspace1_id));
        let mut conv2 = create_test_conversation("Conv 2", ConversationStatus::Active, None, Some(workspace2_id));
        let conv3 = create_test_conversation("Conv 3", ConversationStatus::Active, None, None);

        let conversations = vec![conv1, conv2, conv3];

        // Test filtering for Workspace 1
        let organized1 = sidebar
            .organize_by_project(&conversations, &workspaces, Some(workspace1_id))
            .unwrap();
        assert_eq!(organized1.len(), 1);
        assert_eq!(organized1[0].name, "Workspace 1");
        assert_eq!(organized1[0].conversations.len(), 1);

        // Test filtering for Workspace 2
        let organized2 = sidebar
            .organize_by_project(&conversations, &workspaces, Some(workspace2_id))
            .unwrap();
        assert_eq!(organized2.len(), 1);
        assert_eq!(organized2[0].name, "Workspace 2");
        assert_eq!(organized2[0].conversations.len(), 1);

        // Test with no active workspace (should show all)
        let organized_all = sidebar
            .organize_by_project(&conversations, &workspaces, None)
            .unwrap();
        assert_eq!(organized_all.len(), 3); // Workspace 1, Workspace 2, and No Workspace
    }

    #[test]
    fn test_branch_suggestions_integration() {
        let config = SidebarConfig::default();
        let mut sidebar = ConversationSidebar::new(config);
        
        let conversation_id = Uuid::new_v4();
        let message_id = Uuid::new_v4();
        
        // Create a test branch suggestion
        let suggestion = BranchSuggestion {
            message_id,
            confidence: 0.8,
            reason: crate::agent::conversation::branching::BranchReason::AlternativeApproach,
            suggested_title: "Alternative Solution".to_string(),
            success_probability: Some(0.7),
            context: crate::agent::conversation::branching::BranchContext {
                relevant_messages: vec![],
                trigger_keywords: vec!["alternative".to_string()],
                conversation_state: crate::agent::conversation::branching::ConversationState::SolutionDevelopment,
                project_context: None,
                mentioned_tools: vec!["git".to_string()],
            },
        };
        
        // Test updating branch suggestions
        sidebar.update_branch_suggestions(conversation_id, vec![suggestion.clone()]);
        assert!(sidebar.get_branch_suggestions(conversation_id).is_some());
        assert_eq!(sidebar.get_branch_suggestions(conversation_id).unwrap().len(), 1);
        
        // Test clearing branch suggestions
        sidebar.clear_branch_suggestions(conversation_id);
        assert!(sidebar.get_branch_suggestions(conversation_id).is_none());
        
        // Test toggle branch suggestions
        assert!(!sidebar.show_branch_suggestions);
        sidebar.toggle_branch_suggestions();
        assert!(sidebar.show_branch_suggestions);
        
        // Test handling branch suggestion actions
        let action = BranchSuggestionAction::CreateBranch {
            conversation_id,
            suggestion: suggestion.clone(),
        };
        
        let sidebar_action = sidebar.handle_branch_suggestion_action(action);
        assert!(sidebar_action.is_some());
        
        match sidebar_action.unwrap() {
            SidebarAction::CreateBranch(id, _) => {
                assert_eq!(id, conversation_id);
            },
            _ => panic!("Expected CreateBranch action"),
        }
    }
    
    #[test]
    fn test_branch_suggestion_dismiss() {
        let config = SidebarConfig::default();
        let mut sidebar = ConversationSidebar::new(config);
        
        let conversation_id = Uuid::new_v4();
        let message_id = Uuid::new_v4();
        
        let suggestion = BranchSuggestion {
            message_id,
            confidence: 0.8,
            reason: crate::agent::conversation::branching::BranchReason::ErrorRecovery,
            suggested_title: "Error Recovery".to_string(),
            success_probability: Some(0.6),
            context: crate::agent::conversation::branching::BranchContext {
                relevant_messages: vec![],
                trigger_keywords: vec!["error".to_string()],
                conversation_state: crate::agent::conversation::branching::ConversationState::ErrorState,
                project_context: None,
                mentioned_tools: vec![],
            },
        };
        
        sidebar.update_branch_suggestions(conversation_id, vec![suggestion]);
        
        // Test dismissing suggestion
        let action = BranchSuggestionAction::DismissSuggestion {
            conversation_id,
            message_id,
        };
        
        let sidebar_action = sidebar.handle_branch_suggestion_action(action);
        assert!(sidebar_action.is_some());
        
        match sidebar_action.unwrap() {
            SidebarAction::DismissBranchSuggestion(conv_id, msg_id) => {
                assert_eq!(conv_id, conversation_id);
                assert_eq!(msg_id, message_id);
            },
            _ => panic!("Expected DismissBranchSuggestion action"),
        }
    }

    #[test]
    fn test_cluster_cohesion_score_display() {
        let sidebar = ConversationSidebar::with_default_config();
        let conversations = create_test_conversations();
        
        let clusters = vec![
            ConversationCluster {
                id: Uuid::new_v4(),
                title: "High Cohesion Cluster".to_string(),
                conversation_ids: vec![conversations[0].id, conversations[1].id],
                centroid: vec![0.1, 0.2, 0.3],
                cohesion_score: 0.95, // High cohesion
                common_tags: vec!["rust".to_string(), "async".to_string()],
                dominant_project_type: Some(ProjectType::Rust),
                time_range: (Utc::now() - chrono::Duration::days(7), Utc::now()),
            },
            ConversationCluster {
                id: Uuid::new_v4(),
                title: "Medium Cohesion Cluster".to_string(),
                conversation_ids: vec![conversations[2].id],
                centroid: vec![0.4, 0.5, 0.6],
                cohesion_score: 0.65, // Medium cohesion
                common_tags: vec!["javascript".to_string()],
                dominant_project_type: Some(ProjectType::JavaScript),
                time_range: (Utc::now() - chrono::Duration::days(3), Utc::now()),
            },
        ];
        
        let organized = sidebar.organize_by_clusters(&conversations, Some(&clusters)).unwrap();
        
        // Should have 2 cluster groups
        assert_eq!(organized.len(), 2);
        
        // Check high cohesion cluster
        let high_cohesion_group = organized.iter().find(|g| g.name == "High Cohesion Cluster").unwrap();
        assert_eq!(high_cohesion_group.conversations.len(), 2);
        // Cohesion score should be stored in metadata for display
        assert!(high_cohesion_group.metadata.avg_success_rate.is_some());
        
        // Check medium cohesion cluster
        let medium_cohesion_group = organized.iter().find(|g| g.name == "Medium Cohesion Cluster").unwrap();
        assert_eq!(medium_cohesion_group.conversations.len(), 1);
    }

    #[test]
    fn test_cluster_breadcrumb_navigation() {
        let mut sidebar = ConversationSidebar::with_default_config();
        let conversations = create_test_conversations();
        
        let clusters = vec![
            ConversationCluster {
                id: Uuid::new_v4(),
                title: "Error Handling".to_string(),
                conversation_ids: vec![conversations[0].id, conversations[1].id],
                centroid: vec![0.1, 0.2, 0.3],
                cohesion_score: 0.85,
                common_tags: vec!["error".to_string(), "handling".to_string()],
                dominant_project_type: Some(ProjectType::Rust),
                time_range: (Utc::now() - chrono::Duration::days(7), Utc::now()),
            },
        ];
        
        // Set organization mode to clusters
        sidebar.set_organization_mode(OrganizationMode::Clusters);
        
        // Test breadcrumb state tracking
        assert_eq!(sidebar.organization_mode, OrganizationMode::Clusters);
        
        // Test cluster group expansion
        let cluster_group_id = format!("cluster_{}", clusters[0].id);
        sidebar.toggle_group(&cluster_group_id);
        assert!(sidebar.expanded_groups.contains(&cluster_group_id));
        
        // Test breadcrumb path: All → Clusters → Error Handling
        let organized = sidebar.organize_by_clusters(&conversations, Some(&clusters)).unwrap();
        let error_handling_group = organized.iter().find(|g| g.name == "Error Handling").unwrap();
        
        // Verify breadcrumb components
        assert_eq!(error_handling_group.name, "Error Handling");
        assert_eq!(error_handling_group.conversations.len(), 2);
        assert!(error_handling_group.id.starts_with("cluster_"));
    }

    #[test]
    fn test_cluster_tooltip_information() {
        let sidebar = ConversationSidebar::with_default_config();
        let conversations = create_test_conversations();
        
        let cluster = ConversationCluster {
            id: Uuid::new_v4(),
            title: "Async Programming".to_string(),
            conversation_ids: vec![conversations[0].id, conversations[1].id],
            centroid: vec![0.1, 0.2, 0.3],
            cohesion_score: 0.78,
            common_tags: vec!["async".to_string(), "tokio".to_string()],
            dominant_project_type: Some(ProjectType::Rust),
            time_range: (Utc::now() - chrono::Duration::days(5), Utc::now()),
        };
        
        let clusters = vec![cluster.clone()];
        let organized = sidebar.organize_by_clusters(&conversations, Some(&clusters)).unwrap();
        
        let async_group = organized.iter().find(|g| g.name == "Async Programming").unwrap();
        
        // Verify tooltip information is available in metadata
        assert_eq!(async_group.conversations.len(), 2);
        assert!(async_group.metadata.statistics.common_tags.contains(&"async".to_string()));
        assert!(async_group.metadata.statistics.common_tags.contains(&"tokio".to_string()));
        
        // Cohesion score should be reflected in success rate for display
        assert!(async_group.metadata.avg_success_rate.is_some());
        let cohesion_as_success = async_group.metadata.avg_success_rate.unwrap();
        assert!((cohesion_as_success - 0.78).abs() < 0.01); // Should match cohesion score
    }

    #[test]
    fn test_cluster_empty_state_handling() {
        let sidebar = ConversationSidebar::with_default_config();
        let conversations = create_test_conversations();
        
        // Test with no clusters
        let organized = sidebar.organize_by_clusters(&conversations, None).unwrap();
        
        // Should create a single "All Conversations" group
        assert_eq!(organized.len(), 1);
        let all_group = &organized[0];
        assert_eq!(all_group.name, "All Conversations");
        assert_eq!(all_group.conversations.len(), conversations.len());
        assert_eq!(all_group.id, "all");
    }

    #[test]
    fn test_cluster_unclustered_conversations() {
        let sidebar = ConversationSidebar::with_default_config();
        let conversations = create_test_conversations();
        
        // Create cluster that only includes some conversations
        let clusters = vec![
            ConversationCluster {
                id: Uuid::new_v4(),
                title: "Partial Cluster".to_string(),
                conversation_ids: vec![conversations[0].id], // Only first conversation
                centroid: vec![0.1, 0.2, 0.3],
                cohesion_score: 0.90,
                common_tags: vec!["rust".to_string()],
                dominant_project_type: Some(ProjectType::Rust),
                time_range: (Utc::now() - chrono::Duration::days(1), Utc::now()),
            },
        ];
        
        let organized = sidebar.organize_by_clusters(&conversations, Some(&clusters)).unwrap();
        
        // Should have cluster group + unclustered group
        assert_eq!(organized.len(), 2);
        
        let partial_cluster = organized.iter().find(|g| g.name == "Partial Cluster").unwrap();
        assert_eq!(partial_cluster.conversations.len(), 1);
        
        let unclustered = organized.iter().find(|g| g.name == "Unclustered").unwrap();
        assert_eq!(unclustered.conversations.len(), conversations.len() - 1);
        assert_eq!(unclustered.id, "unclustered");
    }

    #[test]
    fn test_cluster_sorting_by_cohesion() {
        let sidebar = ConversationSidebar::with_default_config();
        let conversations = create_test_conversations();
        
        let clusters = vec![
            ConversationCluster {
                id: Uuid::new_v4(),
                title: "Low Cohesion".to_string(),
                conversation_ids: vec![conversations[0].id],
                centroid: vec![0.1, 0.2, 0.3],
                cohesion_score: 0.45, // Low cohesion
                common_tags: vec![],
                dominant_project_type: None,
                time_range: (Utc::now() - chrono::Duration::days(1), Utc::now()),
            },
            ConversationCluster {
                id: Uuid::new_v4(),
                title: "High Cohesion".to_string(),
                conversation_ids: vec![conversations[1].id],
                centroid: vec![0.4, 0.5, 0.6],
                cohesion_score: 0.95, // High cohesion
                common_tags: vec!["excellent".to_string()],
                dominant_project_type: Some(ProjectType::Rust),
                time_range: (Utc::now() - chrono::Duration::days(1), Utc::now()),
            },
        ];
        
        let organized = sidebar.organize_by_clusters(&conversations, Some(&clusters)).unwrap();
        
        // Should have both clusters
        assert_eq!(organized.len(), 3); // 2 clusters + unclustered
        
        // Find clusters by name
        let high_cohesion = organized.iter().find(|g| g.name == "High Cohesion").unwrap();
        let low_cohesion = organized.iter().find(|g| g.name == "Low Cohesion").unwrap();
        
        // High cohesion should have higher priority (lower number = higher priority)
        assert!(high_cohesion.priority < low_cohesion.priority);
        
        // Verify cohesion scores are preserved in metadata
        assert!(high_cohesion.metadata.avg_success_rate.unwrap() > low_cohesion.metadata.avg_success_rate.unwrap());
    }

    #[test]
    fn test_cluster_time_range_display() {
        let sidebar = ConversationSidebar::with_default_config();
        let conversations = create_test_conversations();
        
        let start_time = Utc::now() - chrono::Duration::days(7);
        let end_time = Utc::now() - chrono::Duration::days(1);
        
        let cluster = ConversationCluster {
            id: Uuid::new_v4(),
            title: "Time Range Cluster".to_string(),
            conversation_ids: vec![conversations[0].id, conversations[1].id],
            centroid: vec![0.1, 0.2, 0.3],
            cohesion_score: 0.80,
            common_tags: vec!["temporal".to_string()],
            dominant_project_type: Some(ProjectType::Rust),
            time_range: (start_time, end_time),
        };
        
        let clusters = vec![cluster];
        let organized = sidebar.organize_by_clusters(&conversations, Some(&clusters)).unwrap();
        
        let time_range_group = organized.iter().find(|g| g.name == "Time Range Cluster").unwrap();
        
        // Verify time range information is available
        assert_eq!(time_range_group.conversations.len(), 2);
        assert!(time_range_group.metadata.last_activity.is_some());
        
        // The last activity should reflect the cluster's time range
        let last_activity = time_range_group.metadata.last_activity.unwrap();
        assert!(last_activity >= start_time && last_activity <= end_time + chrono::Duration::hours(1));
    }

    #[test]
    fn test_cluster_common_tags_integration() {
        let sidebar = ConversationSidebar::with_default_config();
        let conversations = create_test_conversations();
        
        let cluster = ConversationCluster {
            id: Uuid::new_v4(),
            title: "Tagged Cluster".to_string(),
            conversation_ids: vec![conversations[0].id, conversations[1].id],
            centroid: vec![0.1, 0.2, 0.3],
            cohesion_score: 0.88,
            common_tags: vec!["rust".to_string(), "async".to_string(), "performance".to_string()],
            dominant_project_type: Some(ProjectType::Rust),
            time_range: (Utc::now() - chrono::Duration::days(3), Utc::now()),
        };
        
        let clusters = vec![cluster];
        let organized = sidebar.organize_by_clusters(&conversations, Some(&clusters)).unwrap();
        
        let tagged_group = organized.iter().find(|g| g.name == "Tagged Cluster").unwrap();
        
        // Verify common tags are preserved in group metadata
        assert!(tagged_group.metadata.statistics.common_tags.contains(&"rust".to_string()));
        assert!(tagged_group.metadata.statistics.common_tags.contains(&"async".to_string()));
        assert!(tagged_group.metadata.statistics.common_tags.contains(&"performance".to_string()));
        assert_eq!(tagged_group.metadata.statistics.common_tags.len(), 3);
    }

    #[test]
    fn test_cluster_navigation_state_persistence() {
        let mut sidebar = ConversationSidebar::with_default_config();
        let conversations = create_test_conversations();
        
        let cluster_id = Uuid::new_v4();
        let cluster = ConversationCluster {
            id: cluster_id,
            title: "Navigation Test Cluster".to_string(),
            conversation_ids: vec![conversations[0].id],
            centroid: vec![0.1, 0.2, 0.3],
            cohesion_score: 0.75,
            common_tags: vec!["navigation".to_string()],
            dominant_project_type: Some(ProjectType::Rust),
            time_range: (Utc::now() - chrono::Duration::days(2), Utc::now()),
        };
        
        let clusters = vec![cluster];
        
        // Set organization mode to clusters
        sidebar.set_organization_mode(OrganizationMode::Clusters);
        
        // Expand the cluster group
        let cluster_group_id = format!("cluster_{}", cluster_id);
        sidebar.toggle_group(&cluster_group_id);
        
        // Verify state persistence
        assert_eq!(sidebar.organization_mode, OrganizationMode::Clusters);
        assert!(sidebar.expanded_groups.contains(&cluster_group_id));
        
        // Switch to different mode and back
        sidebar.set_organization_mode(OrganizationMode::Recency);
        sidebar.set_organization_mode(OrganizationMode::Clusters);
        
        // Expanded state should persist
        assert!(sidebar.expanded_groups.contains(&cluster_group_id));
    }

    #[test]
    fn test_responsive_ui_configuration() {
        let mut config = SidebarConfig::default();
        
        // Test default responsive configuration
        assert!(config.responsive.enabled);
        assert_eq!(config.responsive.small_screen_breakpoint, 1366.0);
        assert!(config.responsive.compact_mode.small_buttons);
        assert!(config.responsive.compact_mode.reduced_spacing);
        assert!(config.responsive.compact_mode.abbreviated_labels);
        assert!(!config.responsive.compact_mode.hide_secondary_elements);
        
        // Test custom responsive configuration
        config.responsive.enabled = false;
        config.responsive.small_screen_breakpoint = 1024.0;
        config.responsive.compact_mode.small_buttons = false;
        
        let sidebar = ConversationSidebar::new(config.clone());
        assert_eq!(sidebar.config.responsive.small_screen_breakpoint, 1024.0);
        assert!(!sidebar.config.responsive.enabled);
        assert!(!sidebar.config.responsive.compact_mode.small_buttons);
    }

    #[test]
    fn test_responsive_ui_behavior() {
        let mut sidebar = ConversationSidebar::with_default_config();
        
        // Test responsive configuration updates
        let mut responsive_config = ResponsiveConfig::default();
        responsive_config.enabled = true;
        responsive_config.small_screen_breakpoint = 1366.0;
        responsive_config.compact_mode.small_buttons = true;
        responsive_config.compact_mode.reduced_spacing = true;
        
        sidebar.config.responsive = responsive_config;
        
        // Verify responsive settings are applied
        assert!(sidebar.config.responsive.enabled);
        assert_eq!(sidebar.config.responsive.small_screen_breakpoint, 1366.0);
        assert!(sidebar.config.responsive.compact_mode.small_buttons);
        assert!(sidebar.config.responsive.compact_mode.reduced_spacing);
    }
    
    // Phase 10: Tests for persistent state management
    #[test]
    fn test_persistent_state_save_load() {
        use crate::config::{SagittaCodeConfig, SidebarPersistentConfig};
        
        let mut sidebar = ConversationSidebar::with_default_config();
        let mut config = SagittaCodeConfig::default();
        
        // Set up test state
        sidebar.organization_mode = OrganizationMode::Tags;
        sidebar.expanded_groups.insert("test_group".to_string());
        sidebar.search_query = Some("test query".to_string());
        sidebar.search_input = "test query".to_string();
        sidebar.show_filters = true;
        sidebar.accessibility_enabled = true;
        sidebar.color_blind_friendly = true;
        
        // Save state
        assert!(sidebar.save_persistent_state(&mut config).is_ok());
        
        // Create new sidebar and load state
        let mut new_sidebar = ConversationSidebar::with_default_config();
        new_sidebar.load_persistent_state(&config.conversation.sidebar);
        
        // Verify state was loaded correctly
        assert_eq!(new_sidebar.organization_mode, OrganizationMode::Tags);
        assert!(new_sidebar.expanded_groups.contains("test_group"));
        assert_eq!(new_sidebar.search_query, Some("test query".to_string()));
        assert_eq!(new_sidebar.search_input, "test query");
        assert!(new_sidebar.show_filters);
        assert!(new_sidebar.accessibility_enabled);
        assert!(new_sidebar.color_blind_friendly);
    }
    
    #[test]
    fn test_search_debouncing() {
        let mut sidebar = ConversationSidebar::with_default_config();
        
        // Test initial search - should debounce (return true) since it's a new query
        sidebar.search_input = "test".to_string();
        let search_input = sidebar.search_input.clone();
        assert!(sidebar.should_debounce_search(&search_input, 300));
        
        // Test same search immediately - should still debounce
        let search_input = sidebar.search_input.clone();
        assert!(sidebar.should_debounce_search(&search_input, 300));
        
        // Test different search - should reset timer and debounce
        sidebar.search_input = "different".to_string();
        let search_input = sidebar.search_input.clone();
        assert!(sidebar.should_debounce_search(&search_input, 300));
        
        // Simulate time passing by manually setting an old timer
        sidebar.search_debounce_timer = Some(Instant::now() - std::time::Duration::from_millis(400));
        // Now it should not debounce since enough time has passed
        let search_input = sidebar.search_input.clone();
        assert!(!sidebar.should_debounce_search(&search_input, 300));
    }
    
    #[test]
    fn test_virtual_scrolling() {
        let sidebar = ConversationSidebar::with_default_config();
        
        // Test virtual scroll range calculation
        let (start, end) = sidebar.get_virtual_scroll_range(1000, 100);
        assert_eq!(start, 0);
        assert_eq!(end, 100);
        
        // Test with offset
        let mut sidebar_with_offset = sidebar.clone();
        sidebar_with_offset.virtual_scroll_offset = 50;
        let (start, end) = sidebar_with_offset.get_virtual_scroll_range(1000, 100);
        assert_eq!(start, 50);
        assert_eq!(end, 150);
        
        // Test offset update
        let mut sidebar_mut = sidebar.clone();
        sidebar_mut.update_virtual_scroll_offset(200, 1000, 100);
        assert_eq!(sidebar_mut.virtual_scroll_offset, 200);
        
        // Test offset clamping
        sidebar_mut.update_virtual_scroll_offset(950, 1000, 100);
        assert_eq!(sidebar_mut.virtual_scroll_offset, 900); // 1000 - 100
    }
    
    #[test]
    fn test_accessibility_features() {
        let mut sidebar = ConversationSidebar::with_default_config();
        sidebar.accessibility_enabled = true;
        sidebar.color_blind_friendly = true;
        
        // Test accessible color mapping
        let success_color = sidebar.get_accessible_color(Color32::GREEN, "success");
        assert_eq!(success_color, Color32::from_rgb(68, 1, 84)); // Viridis dark purple
        
        let warning_color = sidebar.get_accessible_color(Color32::YELLOW, "warning");
        assert_eq!(warning_color, Color32::from_rgb(253, 231, 37)); // Viridis bright yellow
        
        // Test screen reader announcements
        sidebar.announce_to_screen_reader("Test announcement".to_string());
        assert_eq!(sidebar.screen_reader_announcements.len(), 1);
        assert_eq!(sidebar.screen_reader_announcements[0], "Test announcement");
        
        // Test that announcements are limited to 5 maximum
        // Clear the rate limiting timer to allow multiple announcements
        sidebar.last_announcement_time = None;
        
        // Add exactly 6 announcements to test the limit
        for i in 0..6 {
            sidebar.last_announcement_time = None; // Reset rate limiting
            sidebar.announce_to_screen_reader(format!("Announcement {}", i));
        }
        
        // Should be limited to 5 announcements total
        assert_eq!(sidebar.screen_reader_announcements.len(), 5);
    }
    
    #[test]
    fn test_cache_invalidation() {
        let mut sidebar = ConversationSidebar::with_default_config();
        
        // Set up cached items
        let test_items = vec![
            ConversationItem {
                summary: create_test_conversation("Test 1", ConversationStatus::Active, None, None),
                display: ConversationDisplay {
                    title: "Test 1".to_string(),
                    status_indicator: StatusIndicator::Active,
                    time_display: "now".to_string(),
                    progress: None,
                    indicators: vec![],
                    color_theme: None,
                },
                selected: false,
                favorite: false,
                preview: None,
            }
        ];
        
        sidebar.cached_rendered_items = Some((100, test_items));
        assert!(sidebar.cached_rendered_items.is_some());
        
        // Test cache invalidation
        sidebar.invalidate_cache();
        assert!(sidebar.cached_rendered_items.is_none());
    }
    
    #[test]
    fn test_auto_save_timing() {
        let config = SidebarConfig::default();
        let mut sidebar = ConversationSidebar::new(config);
        let config_arc = Arc::new(Mutex::new(SagittaCodeConfig::default()));
        
        // First call should trigger save
        sidebar.auto_save_state(config_arc.clone());
        assert!(sidebar.last_state_save.is_some());
        
        let first_save_time = sidebar.last_state_save.unwrap();
        
        // Immediate second call should not trigger save
        sidebar.auto_save_state(config_arc.clone());
        assert_eq!(sidebar.last_state_save.unwrap(), first_save_time);
        
        // Simulate time passing by manually setting an old timestamp
        sidebar.last_state_save = Some(Instant::now() - std::time::Duration::from_secs(31));
        sidebar.auto_save_state(config_arc);
        assert!(sidebar.last_state_save.unwrap() > first_save_time);
    }
    
    #[test]
    fn test_organization_mode_persistence() {
        use crate::config::SagittaCodeConfig;
        
        let mut sidebar = ConversationSidebar::with_default_config();
        let mut config = SagittaCodeConfig::default();
        
        // Test all organization modes
        let modes = vec![
            OrganizationMode::Recency,
            OrganizationMode::Project,
            OrganizationMode::Status,
            OrganizationMode::Clusters,
            OrganizationMode::Tags,
            OrganizationMode::Success,
            OrganizationMode::Custom("Custom Mode".to_string()),
        ];
        
        for mode in modes {
            sidebar.organization_mode = mode.clone();
            assert!(sidebar.save_persistent_state(&mut config).is_ok());
            
            let mut new_sidebar = ConversationSidebar::with_default_config();
            new_sidebar.load_persistent_state(&config.conversation.sidebar);
            assert_eq!(new_sidebar.organization_mode, mode);
        }
    }
    
    #[test]
    fn test_filter_persistence() {
        use crate::config::SagittaCodeConfig;
        use crate::agent::conversation::types::ProjectType;
        use crate::agent::state::types::ConversationStatus;
        
        let mut sidebar = ConversationSidebar::with_default_config();
        let mut config = SagittaCodeConfig::default();
        
        // Set up complex filter state
        sidebar.filters.project_types = vec![ProjectType::Rust, ProjectType::Python];
        sidebar.filters.statuses = vec![ConversationStatus::Active, ConversationStatus::Completed];
        sidebar.filters.tags = vec!["important".to_string(), "urgent".to_string()];
        sidebar.filters.min_messages = Some(5);
        sidebar.filters.min_success_rate = Some(0.8);
        sidebar.filters.favorites_only = true;
        sidebar.filters.branches_only = true;
        sidebar.filters.checkpoints_only = true;
        
        // Save and load
        assert!(sidebar.save_persistent_state(&mut config).is_ok());
        
        let mut new_sidebar = ConversationSidebar::with_default_config();
        new_sidebar.load_persistent_state(&config.conversation.sidebar);
        
        // Verify all filter settings were preserved
        assert_eq!(new_sidebar.filters.project_types.len(), 2);
        assert_eq!(new_sidebar.filters.statuses.len(), 2);
        assert_eq!(new_sidebar.filters.tags, vec!["important", "urgent"]);
        assert_eq!(new_sidebar.filters.min_messages, Some(5));
        assert_eq!(new_sidebar.filters.min_success_rate, Some(0.8));
        assert!(new_sidebar.filters.favorites_only);
        assert!(new_sidebar.filters.branches_only);
        assert!(new_sidebar.filters.checkpoints_only);
    }
} 

// --- Start of new tests ---

#[test]
fn test_status_organization_mode() {
    let sidebar = ConversationSidebar::with_default_config();
    let conversations = vec![
        create_test_conversation("Active Conv", ConversationStatus::Active, None, None),
        create_test_conversation("Completed Conv", ConversationStatus::Completed, None, None),
        create_test_conversation("Paused Conv", ConversationStatus::Paused, None, None),
    ];
    let organized = sidebar.organize_by_status(&conversations).unwrap();
    assert_eq!(organized.len(), 3);
    assert!(organized.iter().any(|g| g.name == "Active"));
    assert!(organized.iter().any(|g| g.name == "Completed"));
    assert!(organized.iter().any(|g| g.name == "Paused"));
}

#[test]
fn test_tags_organization_mode() {
    let sidebar = ConversationSidebar::with_default_config();
    let mut conv1 = create_test_conversation("Conv 1", ConversationStatus::Active, None, None);
    conv1.tags = vec!["rust".to_string(), "backend".to_string()];
    let mut conv2 = create_test_conversation("Conv 2", ConversationStatus::Active, None, None);
    conv2.tags = vec!["python".to_string(), "backend".to_string()];
    let conv3 = create_test_conversation("Conv 3", ConversationStatus::Active, None, None);
    
    let conversations = vec![conv1, conv2, conv3];
    let organized = sidebar.organize_by_tags(&conversations).unwrap();
    
    // Debug: Print what groups we actually get
    println!("Number of groups: {}", organized.len());
    for group in &organized {
        println!("Group: '{}' with {} conversations", group.name, group.conversations.len());
    }
    
    // The test expects 4 groups, but let's see what we actually get
    // conv1 has tags: ["rust", "backend"]
    // conv2 has tags: ["python", "backend"] 
    // conv3 has tags: ["test"] (from create_test_conversation)
    // So we should get groups: "backend" (2 convs), "rust" (1 conv), "python" (1 conv), "test" (1 conv)
    assert_eq!(organized.len(), 4); // backend, rust, python, test
    
    // Find the backend group (should have 2 conversations)
    let backend_group = organized.iter().find(|g| g.name == "backend");
    assert!(backend_group.is_some(), "Backend group should exist. Available groups: {:?}", 
            organized.iter().map(|g| &g.name).collect::<Vec<_>>());
    let backend_group = backend_group.unwrap();
    assert_eq!(backend_group.conversations.len(), 2);
    
    // Find the test group (conv3 should be in here since create_test_conversation adds "test" tag)
    let test_group = organized.iter().find(|g| g.name == "test");
    assert!(test_group.is_some(), "Test group should exist. Available groups: {:?}", 
            organized.iter().map(|g| &g.name).collect::<Vec<_>>());
    let test_group = test_group.unwrap();
    assert_eq!(test_group.conversations.len(), 1);
}

#[test]
fn test_success_organization_mode() {
    let sidebar = ConversationSidebar::with_default_config();
    let conversations = vec![
        create_test_conversation("Successful", ConversationStatus::Completed, None, None),
        create_test_conversation("In Progress", ConversationStatus::Active, None, None),
        create_test_conversation("Other", ConversationStatus::Paused, None, None),
    ];
    let organized = sidebar.organize_by_success(&conversations).unwrap();
    assert_eq!(organized.len(), 3);
    assert!(organized.iter().any(|g| g.name == "Successful"));
    assert!(organized.iter().any(|g| g.name == "In Progress"));
    assert!(organized.iter().any(|g| g.name == "Other"));
}

#[test]
fn test_action_handling_triggers_pending_action() {
    let mut sidebar = ConversationSidebar::with_default_config();
    assert!(sidebar.pending_action.is_none());

    // Simulate clicking "New Conversation"
    // This is normally done in the UI render code
    sidebar.pending_action = Some(SidebarAction::CreateNewConversation);
    assert!(matches!(sidebar.pending_action, Some(SidebarAction::CreateNewConversation)));

    // The handle_sidebar_actions function would then consume this.
    // We can't test the async part here, but we've verified the state is set.
}