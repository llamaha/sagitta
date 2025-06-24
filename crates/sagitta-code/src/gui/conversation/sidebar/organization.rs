use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use uuid::Uuid;

use crate::agent::conversation::types::ConversationSummary;
use crate::agent::conversation::clustering::ConversationCluster;
use crate::agent::state::types::ConversationStatus;

use super::types::{
    ConversationSidebar, OrganizedConversations, ConversationGroup, OrganizationMode,
    ConversationItem, ConversationDisplay, StatusIndicator, VisualIndicator, IndicatorType,
    GroupMetadata, GroupStatistics
};

impl ConversationSidebar {
    /// Organize conversations for display
    pub fn organize_conversations(
        &self,
        conversations: &[ConversationSummary],
        clusters: Option<&[ConversationCluster]>,
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

    /// Organize conversations by recency
    pub fn organize_by_recency(&self, conversations: &[ConversationSummary]) -> Result<Vec<ConversationGroup>> {
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
    ) -> Result<Vec<ConversationGroup>> {
        let mut groups: HashMap<Option<String>, Vec<ConversationSummary>> = HashMap::new();

        for conv in conversations {
            // Group by project name if available
            let project_key = conv.project_name.clone();
            groups.entry(project_key).or_default().push(conv.clone());
        }

        let mut conversation_groups = Vec::new();
        for (project_name, convs) in groups {
            let (group_name, group_id, priority) = match &project_name {
                Some(name) => (name.clone(), name.clone(), 0),
                None => ("No Project".to_string(), "no-project".to_string(), 1),
            };

            let group = self.create_group(
                &group_id,
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
    pub fn organize_by_status(&self, conversations: &[ConversationSummary]) -> Result<Vec<ConversationGroup>> {
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
    pub(super) fn create_conversation_item(&self, summary: ConversationSummary) -> ConversationItem {
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
        
        // Add branch and checkpoint indicators if available
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
        
        // Add tag indicators - tags is Vec<String> not Option<Vec<String>>
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
    pub fn format_relative_time(&self, time: DateTime<Utc>) -> String {
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
} 