use crate::agent::conversation::types::ConversationSummary;
use super::types::{ConversationSidebar, SidebarFilters};

impl ConversationSidebar {
    /// Apply filters to conversations
    pub fn apply_filters(&self, conversations: &[ConversationSummary]) -> Vec<ConversationSummary> {
        conversations
            .iter()
            .filter(|conv| {
                // Filter by project type - use project_name as proxy since project_context doesn't exist
                if !self.filters.project_types.is_empty() {
                    // For now, skip project type filtering since we don't have project_context
                    // This could be implemented by checking project_name or other available fields
                }

                // Filter by status
                if !self.filters.statuses.is_empty() {
                    if !self.filters.statuses.contains(&conv.status) {
                        return false;
                    }
                }

                // Filter by tags - tags is Vec<String>, not Option<Vec<String>>
                if !self.filters.tags.is_empty() {
                    if !self.filters.tags.iter().any(|tag| conv.tags.contains(tag)) {
                        return false;
                    }
                }

                // Filter by date range - use last_active since updated_at doesn't exist
                if let Some((start, end)) = self.filters.date_range {
                    if conv.last_active < start || conv.last_active > end {
                        return false;
                    }
                }

                // Filter by minimum message count
                if let Some(min_messages) = self.filters.min_messages {
                    if conv.message_count < min_messages {
                        return false;
                    }
                }

                // Skip success rate filter since success_rate field doesn't exist
                // Skip favorites filter since is_favorite field doesn't exist
                // Skip branches filter since branch_count field doesn't exist
                // Skip checkpoints filter since checkpoint_count field doesn't exist

                // Filter by branches - use has_branches field
                if self.filters.branches_only {
                    if !conv.has_branches {
                        return false;
                    }
                }

                // Filter by checkpoints - use has_checkpoints field
                if self.filters.checkpoints_only {
                    if !conv.has_checkpoints {
                        return false;
                    }
                }

                true
            })
            .cloned()
            .collect()
    }

    /// Apply search to conversations
    pub fn apply_search(&self, conversations: &[ConversationSummary], query: &str) -> Vec<ConversationSummary> {
        if query.trim().is_empty() {
            return conversations.to_vec();
        }

        let query_lower = query.to_lowercase();
        conversations
            .iter()
            .filter(|conv| {
                // Search in title
                if conv.title.to_lowercase().contains(&query_lower) {
                    return true;
                }

                // Search in tags - tags is Vec<String>, not Option<Vec<String>>
                if conv.tags.iter().any(|tag| tag.to_lowercase().contains(&query_lower)) {
                    return true;
                }

                // Search in project name
                if let Some(ref name) = conv.project_name {
                    if name.to_lowercase().contains(&query_lower) {
                        return true;
                    }
                }

                false
            })
            .cloned()
            .collect()
    }

    /// Update filters
    pub fn update_filters(&mut self, filters: SidebarFilters) {
        self.filters = filters;
        self.invalidate_cache();
    }

    /// Set search query
    pub fn set_search_query(&mut self, query: Option<String>) {
        self.search_query = query;
        self.invalidate_cache();
    }
} 