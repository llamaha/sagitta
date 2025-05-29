use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use egui::{Align, Color32, ComboBox, Frame, Grid, Layout, RichText, ScrollArea, Stroke, TextEdit, Ui, Vec2, WidgetText, Context};
use egui_extras::{Size, StripBuilder};

use crate::agent::conversation::types::{ConversationSummary, ProjectType};
use crate::agent::conversation::clustering::ConversationCluster;
use crate::agent::state::types::AgentMode;
use crate::agent::state::types::ConversationStatus;
use crate::gui::theme::AppTheme;

// --- Minimal Placeholder definitions --- 
#[derive(Debug, Clone, Default)] pub struct FredAgentAppState { pub current_conversation_id: Option<Uuid>, pub editing_conversation_id: Option<Uuid>, pub sidebar_action: Option<SidebarAction>, pub conversation_list: Vec<ConversationSummary>, pub show_clustered_conversations: bool, pub current_agent_mode: AgentMode, pub target_conversation_id: Option<Uuid>}
impl FredAgentAppState { pub fn switch_to_conversation(&mut self, _id: Uuid) {} }
#[derive(Debug, Clone)] pub enum SidebarAction { RequestDeleteConversation(Uuid), RenameConversation(Uuid, String) }
#[derive(Debug, Clone, Default)] pub struct DisplayIndicator { pub display: String, pub color: Option<Color32>}
#[derive(Debug, Clone, Default)] pub struct ConversationDisplayDetails { pub title: String, pub time_display: String, pub indicators: Vec<DisplayIndicator>}
#[derive(Debug, Clone)] pub struct DisplayConversationItem { pub summary: ConversationSummary, pub display: ConversationDisplayDetails, pub preview: Option<String>}
fn get_status_icon(status: ConversationStatus) -> String {
    match status {
        ConversationStatus::Active => "▶".to_string(),
        ConversationStatus::Paused => "⏸".to_string(),
        ConversationStatus::Completed => "✅".to_string(),
        ConversationStatus::Archived => "📦".to_string(),
        ConversationStatus::Summarizing => "⏳".to_string(),
    }
}
// --- End Placeholder definitions ---

/// Conversation sidebar component for smart organization
#[derive(Clone)]
pub struct ConversationSidebar {
    /// Current organization mode
    pub organization_mode: OrganizationMode,
    
    /// Filter settings
    pub filters: SidebarFilters,
    
    /// Search query
    pub search_query: Option<String>,
    
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
            expanded_groups: std::collections::HashSet::new(),
            selected_conversation: None,
            config,
            clusters: Vec::new(),
            edit_buffer: String::new(),
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
    ) -> Result<OrganizedConversations> {
        // Apply filters first
        let filtered_conversations = self.apply_filters(conversations);
        
        // Apply search if present
        let searched_conversations = if let Some(ref query) = self.search_query {
            self.apply_search(&filtered_conversations, query)
        } else {
            filtered_conversations
        };
        
        // Organize into groups based on mode
        let groups = match &self.organization_mode {
            OrganizationMode::Recency => self.organize_by_recency(&searched_conversations),
            OrganizationMode::Project => self.organize_by_project(&searched_conversations),
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
    fn organize_by_project(&self, conversations: &[ConversationSummary]) -> Result<Vec<ConversationGroup>> {
        let mut project_groups: HashMap<String, Vec<ConversationSummary>> = HashMap::new();
        
        for conv in conversations {
            let project_name = conv.project_name.clone().unwrap_or_else(|| "No Project".to_string());
            project_groups.entry(project_name).or_insert_with(Vec::new).push(conv.clone());
        }
        
        let mut groups = Vec::new();
        for (project_name, mut convs) in project_groups {
            convs.sort_by(|a, b| b.last_active.cmp(&a.last_active));
            let group_id = format!("project_{}", project_name.to_lowercase().replace(' ', "_"));
            groups.push(self.create_group(&group_id, &project_name, convs, 50)?);
        }
        
        // Sort groups by name
        groups.sort_by(|a, b| a.name.cmp(&b.name));
        
        Ok(groups)
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
            
            for cluster in clusters {
                let cluster_conversations: Vec<ConversationSummary> = cluster
                    .conversation_ids
                    .iter()
                    .filter_map(|id| conversation_map.get(id))
                    .map(|conv| (*conv).clone())
                    .collect();
                
                if !cluster_conversations.is_empty() {
                    let group_id = format!("cluster_{}", cluster.id);
                    groups.push(self.create_group(&group_id, &cluster.title, cluster_conversations, 50)?);
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

    pub fn show(&mut self, ctx: &egui::Context, app_state: &mut FredAgentAppState, theme: &AppTheme) {
        egui::SidePanel::left("conversation_sidebar")
            .resizable(true)
            .default_width(250.0)
            .show(ctx, |ui| {
                self.render_header(ui, app_state, theme);
                self.render_search_bar(ui, app_state);
                ui.separator();

                ScrollArea::vertical().show(ui, |ui| {
                    if app_state.show_clustered_conversations && !self.clusters.is_empty() {
                        for cluster in &self.clusters {
                            render_cluster_item(ui, cluster, app_state, theme);
                        }
                    } else {
                        let grouped_conversations = self.group_conversations(&app_state.conversation_list);
                        for (_group_key, conv_items) in grouped_conversations {
                            for conv_item in conv_items {
                                ui.push_id(format!("conv_item_{}", conv_item.summary.id), |ui| {
                                    let is_current_chat = app_state.current_conversation_id == Some(conv_item.summary.id);
                                    let is_editing_this = app_state.editing_conversation_id == Some(conv_item.summary.id);
                                    
                                    render_conversation_list_item(
                                        ui, 
                                        &conv_item, 
                                        app_state, 
                                        theme, 
                                        is_current_chat, 
                                        is_editing_this, 
                                        &mut self.edit_buffer,
                                        ctx 
                                    );
                                });
                            }
                        }
                    }
                });
                self.handle_sidebar_actions(app_state, ctx);
            });
    }

    // Stubs for missing methods
    fn render_header(&mut self, _ui: &mut Ui, _app_state: &mut FredAgentAppState, _theme: &AppTheme) { /* Placeholder */ }
    fn render_search_bar(&mut self, _ui: &mut Ui, _app_state: &mut FredAgentAppState) { /* Placeholder */ }
    fn group_conversations(&self, conv_list: &[ConversationSummary]) -> HashMap<String, Vec<DisplayConversationItem>> {
        let mut map = HashMap::new();
        let items = conv_list.iter().map(|summary| DisplayConversationItem {
            summary: summary.clone(),
            display: ConversationDisplayDetails { title: summary.title.clone(), time_display: "-".to_string(), indicators: vec![] },
            preview: None,
        }).collect();
        map.insert("All".to_string(), items);
        map
    }
    fn handle_sidebar_actions(&mut self, _app_state: &mut FredAgentAppState, _ctx: &egui::Context) { /* Placeholder */ }
}

fn render_conversation_list_item(
    ui: &mut Ui,
    conv_item: &DisplayConversationItem,
    app_state: &mut FredAgentAppState,
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
    app_state: &mut FredAgentAppState, 
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
    use crate::agent::conversation::types::{ConversationSummary, ProjectType};
    use crate::agent::state::types::ConversationStatus;
    use chrono::Utc;
    use uuid::Uuid;

    fn create_test_conversation(title: &str, status: ConversationStatus, project_type: Option<ProjectType>) -> ConversationSummary {
        ConversationSummary {
            id: Uuid::new_v4(),
            title: title.to_string(),
            created_at: Utc::now(),
            last_active: Utc::now(),
            message_count: 5,
            status,
            tags: vec!["test".to_string()],
            workspace_id: None,
            has_branches: false,
            has_checkpoints: false,
            project_name: project_type.map(|pt| format!("{:?} Project", pt)),
        }
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
            create_test_conversation("Active Conv", ConversationStatus::Active, Some(ProjectType::Rust)),
            create_test_conversation("Completed Conv", ConversationStatus::Completed, Some(ProjectType::Python)),
            create_test_conversation("Archived Conv", ConversationStatus::Archived, Some(ProjectType::JavaScript)),
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
        let sidebar = ConversationSidebar::new(config);
        
        let conversations = vec![
            create_test_conversation("Rust Programming Help", ConversationStatus::Active, Some(ProjectType::Rust)),
            create_test_conversation("Python Data Analysis", ConversationStatus::Active, Some(ProjectType::Python)),
            create_test_conversation("JavaScript Frontend", ConversationStatus::Active, Some(ProjectType::JavaScript)),
        ];
        
        // Test title search
        let results = sidebar.apply_search(&conversations, "rust");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust Programming Help");
        
        // Test case insensitive search
        let results = sidebar.apply_search(&conversations, "PYTHON");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Python Data Analysis");
        
        // Test partial match
        let results = sidebar.apply_search(&conversations, "data");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Python Data Analysis");
        
        // Test no match
        let results = sidebar.apply_search(&conversations, "nonexistent");
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
            create_test_conversation("Main Conversation", ConversationStatus::Active, Some(ProjectType::Rust)),
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
        let config = SidebarConfig::default();
        let sidebar = ConversationSidebar::new(config);
        
        let conversations = vec![
            create_test_conversation("Main Conversation", ConversationStatus::Active, Some(ProjectType::Rust)),
        ];
        
        // Test that conversations with checkpoints are properly identified
        let mut conv_with_checkpoints = conversations[0].clone();
        conv_with_checkpoints.has_checkpoints = true;
        
        let filtered = sidebar.apply_filters(&[conv_with_checkpoints.clone()]);
        assert_eq!(filtered.len(), 1);
        
        // Test checkpoint filter
        let mut sidebar_with_checkpoint_filter = sidebar.clone();
        sidebar_with_checkpoint_filter.filters.checkpoints_only = true;
        let filtered = sidebar_with_checkpoint_filter.apply_filters(&[conv_with_checkpoints]);
        assert_eq!(filtered.len(), 1);
        
        // Test that conversations without checkpoints are filtered out
        let filtered = sidebar_with_checkpoint_filter.apply_filters(&conversations);
        assert_eq!(filtered.len(), 0);
    }

    #[test]
    fn test_semantic_clustering_organization() {
        let config = SidebarConfig::default();
        let sidebar = ConversationSidebar::new(config);
        
        let conversations = vec![
            create_test_conversation("Rust Error Handling", ConversationStatus::Active, Some(ProjectType::Rust)),
            create_test_conversation("Python Error Handling", ConversationStatus::Active, Some(ProjectType::Python)),
            create_test_conversation("JavaScript Async", ConversationStatus::Active, Some(ProjectType::JavaScript)),
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
            create_test_conversation("High Success Conv", ConversationStatus::Completed, Some(ProjectType::Rust)),
            create_test_conversation("Low Success Conv", ConversationStatus::Active, Some(ProjectType::Python)),
            create_test_conversation("Failed Conv", ConversationStatus::Archived, Some(ProjectType::JavaScript)),
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
        
        let conversations = vec![
            create_test_conversation("Rust Project Conv", ConversationStatus::Active, Some(ProjectType::Rust)),
            create_test_conversation("Python Project Conv", ConversationStatus::Active, Some(ProjectType::Python)),
            create_test_conversation("Another Rust Conv", ConversationStatus::Completed, Some(ProjectType::Rust)),
            create_test_conversation("No Project Conv", ConversationStatus::Active, None),
        ];
        
        let organized = sidebar.organize_by_project(&conversations).unwrap();
        
        // Should have 3 groups: Rust Project, Python Project, No Project
        assert_eq!(organized.len(), 3);
        
        // Check Rust project group
        let rust_group = organized.iter().find(|g| g.name.contains("Rust")).unwrap();
        assert_eq!(rust_group.conversations.len(), 2);
        
        // Check Python project group
        let python_group = organized.iter().find(|g| g.name.contains("Python")).unwrap();
        assert_eq!(python_group.conversations.len(), 1);
        
        // Check No Project group
        let no_project_group = organized.iter().find(|g| g.name == "No Project").unwrap();
        assert_eq!(no_project_group.conversations.len(), 1);
    }
} 