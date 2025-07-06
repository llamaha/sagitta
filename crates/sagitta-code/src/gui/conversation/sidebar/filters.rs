use crate::agent::conversation::types::ConversationSummary;
use super::types::ConversationSidebar;

impl ConversationSidebar {
    /// Apply filters to conversations (no filtering after removal of filters feature)
    pub fn apply_filters(&self, conversations: &[ConversationSummary]) -> Vec<ConversationSummary> {
        conversations.to_vec()
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


    /// Set search query
    pub fn set_search_query(&mut self, query: Option<String>) {
        self.search_query = query;
        self.invalidate_cache();
    }
} 