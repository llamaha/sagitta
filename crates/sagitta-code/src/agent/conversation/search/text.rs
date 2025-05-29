// Text-based conversation search implementation
// TODO: Implement actual text search

use async_trait::async_trait;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use similar::{ChangeTag, TextDiff};
use std::collections::HashSet;

use super::ConversationSearchEngine;
use crate::agent::conversation::types::{
    Conversation, ConversationQuery, ConversationSearchResult, ConversationSummary
};

/// Text-based conversation search engine
pub struct TextConversationSearchEngine {
    /// In-memory search index
    index: Arc<RwLock<TextIndex>>,
}

/// Search index structure
#[derive(Debug, Default)]
struct TextIndex {
    /// Conversation summaries for quick access
    conversations: HashMap<Uuid, ConversationSummary>,
    
    /// Tag index (tag -> conversation_ids)
    tag_index: HashMap<String, HashSet<Uuid>>,
}

impl TextConversationSearchEngine {
    /// Create a new text search engine
    pub fn new() -> Self {
        Self {
            index: Arc::new(RwLock::new(TextIndex::default())),
        }
    }
    
    /// Extract searchable text from a conversation
    fn extract_searchable_text(conversation: &Conversation) -> String {
        let mut text_parts = Vec::new();
        
        // Add title
        text_parts.push(conversation.title.clone());
        
        // Add message content
        for message in &conversation.messages {
            text_parts.push(message.content.clone());
        }
        
        // Add branch content
        for branch in &conversation.branches {
            text_parts.push(branch.title.clone());
            if let Some(description) = &branch.description {
                text_parts.push(description.clone());
            }
            for message in &branch.messages {
                text_parts.push(message.content.clone());
            }
        }
        
        // Add checkpoint titles
        for checkpoint in &conversation.checkpoints {
            text_parts.push(checkpoint.title.clone());
            if let Some(description) = &checkpoint.description {
                text_parts.push(description.clone());
            }
        }
        
        // Add tags
        text_parts.extend(conversation.tags.clone());
        
        // Join all text with spaces
        text_parts.join(" ")
    }
    
    /// Calculate text similarity score
    fn calculate_similarity_score(query: &str, text: &str) -> f32 {
        let query_lower = query.to_lowercase();
        let text_lower = text.to_lowercase();
        
        // Exact match gets highest score
        if text_lower.contains(&query_lower) {
            return 1.0;
        }
        
        // Use text diff for similarity calculation
        let diff = TextDiff::from_words(&query_lower, &text_lower);
        let mut matches = 0;
        let mut total = 0;
        
        for change in diff.iter_all_changes() {
            total += 1;
            if change.tag() == ChangeTag::Equal {
                matches += 1;
            }
        }
        
        if total == 0 {
            0.0
        } else {
            matches as f32 / total as f32
        }
    }
    
    /// Find matching text snippets
    fn find_matching_snippets(query: &str, text: &str, max_snippets: usize) -> Vec<String> {
        let query_lower = query.to_lowercase();
        let text_lower = text.to_lowercase();
        let mut snippets = Vec::new();
        
        // Find all occurrences of query terms
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();
        
        for word in query_words {
            if let Some(pos) = text_lower.find(word) {
                // Extract snippet around the match
                let start = pos.saturating_sub(50);
                let end = (pos + word.len() + 50).min(text.len());
                
                if let Some(snippet) = text.get(start..end) {
                    let snippet = snippet.trim();
                    if !snippet.is_empty() && !snippets.contains(&snippet.to_string()) {
                        snippets.push(snippet.to_string());
                        if snippets.len() >= max_snippets {
                            break;
                        }
                    }
                }
            }
        }
        
        snippets
    }
    
    /// Update tag index
    async fn update_tag_index(&self, conversation_id: Uuid, tags: &[String]) {
        let mut index = self.index.write().await;
        
        // Remove from old tags
        for tag_conversations in index.tag_index.values_mut() {
            tag_conversations.retain(|&id| id != conversation_id);
        }
        
        // Add to new tags
        for tag in tags {
            index.tag_index
                .entry(tag.clone())
                .or_insert_with(HashSet::new)
                .insert(conversation_id);
        }
    }
    
    /// Update workspace index
    async fn update_workspace_index(&self, conversation_id: Uuid, workspace_id: Option<Uuid>) {
        let mut index = self.index.write().await;
        
        // Remove from old workspace
        for workspace_conversations in index.tag_index.values_mut() {
            workspace_conversations.retain(|&id| id != conversation_id);
        }
        
        // Add to new workspace
        if let Some(workspace_id) = workspace_id {
            index.tag_index.entry(workspace_id.to_string()).or_insert_with(HashSet::new).insert(conversation_id);
        }
    }

    fn perform_text_search(&self, pre_filtered_summaries: Vec<&ConversationSummary>, query: &ConversationQuery) -> Vec<ConversationSearchResult> {
        let mut results = Vec::new();
        
        if let Some(q_text) = query.text.as_ref().filter(|t| !t.is_empty()).map(|t| t.to_lowercase()) {
            // Text query is present, perform text search
            for conversation_summary in pre_filtered_summaries {
                if conversation_summary.title.to_lowercase().contains(&q_text) {
                    results.push(ConversationSearchResult {
                        id: conversation_summary.id,
                        title: conversation_summary.title.clone(),
                        relevance_score: 1.0, // Matched text
                        summary_snippet: Some(conversation_summary.title.clone()),
                        last_active: conversation_summary.last_active,
                        conversation: None,
                        matching_snippets: vec![conversation_summary.title.clone()],
                        matching_messages: Vec::new(),
                    });
                }
                // If text query is present but no match, this summary is skipped.
            }
        } else {
            // No text query, so all pre_filtered_summaries are considered matches from other filters.
            for conversation_summary in pre_filtered_summaries {
                results.push(ConversationSearchResult {
                    id: conversation_summary.id,
                    title: conversation_summary.title.clone(),
                    relevance_score: 0.5, // Neutral score for filter-only match
                    summary_snippet: Some(conversation_summary.title.chars().take(100).collect()),
                    last_active: conversation_summary.last_active,
                    conversation: None,
                    matching_snippets: vec![conversation_summary.title.chars().take(100).collect()], // Default snippet
                    matching_messages: Vec::new(),
                });
            }
        }
        results.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap_or(std::cmp::Ordering::Equal));
        results
    }
}

#[async_trait]
impl ConversationSearchEngine for TextConversationSearchEngine {
    async fn index_conversation(&self, conversation: &Conversation) -> Result<()> {
        let mut index = self.index.write().await;
        index.conversations.insert(conversation.id, conversation.to_summary()); // Store summary
        
        // Index words from title
        let title_words: HashSet<String> = conversation.title
            .to_lowercase()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        for word in title_words {
            index.tag_index.entry(word).or_default().insert(conversation.id);
        }

        // Index actual tags
        for tag in &conversation.tags {
            index.tag_index.entry(tag.to_lowercase()).or_default().insert(conversation.id);
        }

        Ok(())
    }
    
    async fn remove_conversation(&self, id: Uuid) -> Result<()> {
        let mut index = self.index.write().await;
        
        // Remove from all indices
        index.conversations.remove(&id);
        
        // Remove from tag index
        for tag_conversations in index.tag_index.values_mut() {
            tag_conversations.retain(|&conv_id| conv_id != id);
        }
        
        Ok(())
    }
    
    async fn search(&self, query: &ConversationQuery) -> Result<Vec<ConversationSearchResult>> {
        let index = self.index.read().await;
        let mut filtered_summaries: Vec<&ConversationSummary> = index.conversations.values().collect(); // Now collects &ConversationSummary

        // Filter by workspace_id if provided
        if let Some(workspace_id) = query.workspace_id {
            filtered_summaries.retain(|c| c.workspace_id == Some(workspace_id));
        }

        // Filter by status if provided
        if let Some(ref status) = query.status {
            filtered_summaries.retain(|c| &c.status == status);
        }

        // Filter by tags if provided
        if query.tags.as_ref().map_or(false, |t| !t.is_empty()) {
            if let Some(tags_to_match) = &query.tags {
                let mut matching_ids_from_tags: HashSet<Uuid> = HashSet::new();
                for (i, tag) in tags_to_match.iter().enumerate() {
                    if let Some(tag_conversations) = index.tag_index.get(&tag.to_lowercase()) {
                        if i == 0 {
                            matching_ids_from_tags.extend(tag_conversations);
                        } else {
                            matching_ids_from_tags.retain(|id| tag_conversations.contains(id));
                        }
                    }
                }
                filtered_summaries.retain(|c| matching_ids_from_tags.contains(&c.id));
            }
        }

        // Filter by date range if provided
        if let Some((start_date, end_date)) = query.date_range {
            filtered_summaries.retain(|c| c.created_at >= start_date && c.created_at <= end_date);
        }

        // Perform text search on remaining conversations
        let results = self.perform_text_search(filtered_summaries, query);
        
        // Limit results
        if let Some(limit) = query.limit {
            Ok(results.into_iter().take(limit).collect())
        } else {
            Ok(results)
        }
    }
    
    async fn clear_index(&self) -> Result<()> {
        let mut index = self.index.write().await;
        *index = TextIndex::default();
        Ok(())
    }
    
    async fn rebuild_index(&self, conversations: &[Conversation]) -> Result<()> {
        // Clear existing index
        self.clear_index().await?;
        
        // Re-index all conversations
        for conversation in conversations {
            self.index_conversation(&conversation).await?;
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::conversation::types::{Conversation, ConversationQuery};
    use crate::agent::message::types::AgentMessage;

    #[tokio::test]
    async fn test_text_search_engine_creation() {
        let engine = TextConversationSearchEngine::new();
        
        // Should start with empty index
        let query = ConversationQuery::default();
        let results = engine.search(&query).await.unwrap();
        assert!(results.is_empty());
    }
    
    #[tokio::test]
    async fn test_index_and_search_conversation() {
        let engine = TextConversationSearchEngine::new();
        
        let mut conversation = Conversation::new("Test Hello Conversation".to_string(), None);
        conversation.add_message(AgentMessage::user("Hello world"));
        conversation.tags.push("test".to_string());
        
        engine.index_conversation(&conversation).await.unwrap();
        
        let mut query = ConversationQuery::default();
        query.text = Some("Hello".to_string());
        
        let results = engine.search(&query).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, conversation.id);
        assert!(results[0].relevance_score > 0.0);
    }
    
    #[tokio::test]
    async fn test_search_by_tags() {
        let engine = TextConversationSearchEngine::new();
        
        let mut conv1 = Conversation::new("Conv 1".to_string(), None);
        conv1.tags.push("rust".to_string());
        
        let mut conv2 = Conversation::new("Conv 2".to_string(), None);
        conv2.tags.push("python".to_string());
        
        engine.index_conversation(&conv1).await.unwrap();
        engine.index_conversation(&conv2).await.unwrap();
        
        let mut query = ConversationQuery::default();
        query.tags = Some(vec!["rust".to_string()]);
        
        let results = engine.search(&query).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, conv1.id);
    }
    
    #[tokio::test]
    async fn test_search_by_workspace() {
        let engine = TextConversationSearchEngine::new();
        
        let workspace_id = Uuid::new_v4();
        let conv1 = Conversation::new("Conv 1".to_string(), Some(workspace_id));
        let conv2 = Conversation::new("Conv 2".to_string(), None);
        
        engine.index_conversation(&conv1).await.unwrap();
        engine.index_conversation(&conv2).await.unwrap();
        
        let mut query = ConversationQuery::default();
        query.workspace_id = Some(workspace_id);
        
        let results = engine.search(&query).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, conv1.id);
    }
    
    #[tokio::test]
    async fn test_remove_conversation() {
        let engine = TextConversationSearchEngine::new();
        
        let conversation = Conversation::new("Test Conversation".to_string(), None);
        let conversation_id = conversation.id;
        
        // Index and then remove
        engine.index_conversation(&conversation).await.unwrap();
        engine.remove_conversation(conversation_id).await.unwrap();
        
        // Should not be found in search
        let query = ConversationQuery::default();
        let results = engine.search(&query).await.unwrap();
        assert!(results.is_empty());
    }
    
    #[tokio::test]
    async fn test_similarity_scoring() {
        // Test exact match
        let score = TextConversationSearchEngine::calculate_similarity_score("hello", "hello world");
        assert!(score > 0.8);
        
        // Test partial match
        let score = TextConversationSearchEngine::calculate_similarity_score("hello", "hi there");
        assert!(score < 0.5);
        
        // Test no match
        let score = TextConversationSearchEngine::calculate_similarity_score("hello", "xyz");
        assert!(score < 0.2);
    }
    
    #[tokio::test]
    async fn test_snippet_extraction() {
        let text = "This is a long text with some important information about rust programming";
        let snippets = TextConversationSearchEngine::find_matching_snippets("rust", text, 2);
        
        assert!(!snippets.is_empty());
        assert!(snippets[0].contains("rust"));
    }
    
    #[tokio::test]
    async fn test_rebuild_index() {
        let engine = TextConversationSearchEngine::new();
        
        let conv1 = Conversation::new("Conv 1".to_string(), None);
        let conv2 = Conversation::new("Conv 2".to_string(), None);
        
        let conversations = vec![conv1.clone(), conv2.clone()];
        
        // Rebuild index
        engine.rebuild_index(&conversations).await.unwrap();
        
        // Should find both conversations
        let query = ConversationQuery::default();
        let results = engine.search(&query).await.unwrap();
        assert_eq!(results.len(), 2);
    }
} 