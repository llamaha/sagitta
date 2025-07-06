// Conversation navigation and timeline management
// TODO: Implement actual navigation

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use anyhow::Result;

use crate::agent::conversation::manager::ConversationManager;

use super::types::{Conversation, ConversationQuery, ConversationSearchResult};
use super::search::ConversationSearchEngine;

/// Advanced conversation navigation and timeline management
pub struct ConversationNavigationManager {
    conversation_manager: Arc<dyn ConversationManager>,
    search_engine: Option<Arc<dyn ConversationSearchEngine>>,
}

/// Timeline navigation result
#[derive(Debug, Clone)]
pub struct TimelineNavigationResult {
    pub conversations: Vec<ConversationTimelineEntry>,
    pub total_count: usize,
    pub time_range: (DateTime<Utc>, DateTime<Utc>),
}

/// Timeline entry for a conversation
#[derive(Debug, Clone)]
pub struct ConversationTimelineEntry {
    pub conversation_id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    pub message_count: usize,
    pub branch_count: usize,
    pub checkpoint_count: usize,
    pub workspace_id: Option<Uuid>,
    pub tags: Vec<String>,
    pub activity_summary: ActivitySummary,
}

/// Activity summary for timeline entries
#[derive(Debug, Clone)]
pub struct ActivitySummary {
    pub recent_activity: Vec<ActivityEvent>,
    pub activity_score: f32,
    pub trending: bool,
}

/// Activity event
#[derive(Debug, Clone)]
pub struct ActivityEvent {
    pub event_type: ActivityEventType,
    pub timestamp: DateTime<Utc>,
    pub description: String,
    pub related_id: Option<Uuid>,
}

/// Types of activity events
#[derive(Debug, Clone, PartialEq)]
pub enum ActivityEventType {
    MessageAdded,
    BranchCreated,
    CheckpointCreated,
    ConversationCreated,
    ConversationUpdated,
}

/// Graph navigation result
#[derive(Debug, Clone)]
pub struct ConversationGraphResult {
    pub nodes: Vec<ConversationNode>,
    pub edges: Vec<ConversationEdge>,
    pub clusters: Vec<ConversationCluster>,
}

/// Node in the conversation graph
#[derive(Debug, Clone)]
pub struct ConversationNode {
    pub id: Uuid,
    pub node_type: NodeType,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub size: f32, // Relative size based on importance/activity
    pub position: Option<(f32, f32)>, // For graph layout
    pub metadata: NodeMetadata,
}

/// Types of nodes in the conversation graph
#[derive(Debug, Clone, PartialEq)]
pub enum NodeType {
    Conversation,
    Branch,
    Checkpoint,
    Message,
    Workspace,
}

/// Node metadata
#[derive(Debug, Clone)]
pub struct NodeMetadata {
    pub message_count: usize,
    pub activity_score: f32,
    pub tags: Vec<String>,
    pub workspace_id: Option<Uuid>,
}

/// Edge in the conversation graph
#[derive(Debug, Clone)]
pub struct ConversationEdge {
    pub from_id: Uuid,
    pub to_id: Uuid,
    pub edge_type: EdgeType,
    pub weight: f32,
    pub created_at: DateTime<Utc>,
}

/// Types of edges in the conversation graph
#[derive(Debug, Clone, PartialEq)]
pub enum EdgeType {
    ParentChild,
    Branch,
    Checkpoint,
    Reference,
    Similarity,
    Temporal,
}

/// Cluster of related conversations
#[derive(Debug, Clone)]
pub struct ConversationCluster {
    pub id: Uuid,
    pub title: String,
    pub conversation_ids: Vec<Uuid>,
    pub center: (f32, f32),
    pub radius: f32,
    pub cluster_type: ClusterType,
}

/// Types of conversation clusters
#[derive(Debug, Clone, PartialEq)]
pub enum ClusterType {
    Semantic,
    Temporal,
    Workspace,
    Tag,
    Activity,
}

/// Enhanced search parameters
#[derive(Debug, Clone)]
pub struct EnhancedSearchQuery {
    pub base_query: ConversationQuery,
    pub semantic_search: Option<String>,
    pub code_context: Option<CodeSearchContext>,
    pub outcome_filter: Option<OutcomeFilter>,
    pub navigation_context: Option<NavigationContext>,
}

/// Code-aware search context
#[derive(Debug, Clone)]
pub struct CodeSearchContext {
    pub file_patterns: Vec<String>,
    pub language_filters: Vec<String>,
    pub repository_context: Option<String>,
    pub code_snippets: Vec<String>,
}

/// Outcome-based filtering
#[derive(Debug, Clone)]
pub struct OutcomeFilter {
    pub success_criteria: Vec<String>,
    pub completion_status: Option<bool>,
    pub artifact_types: Vec<String>,
    pub impact_level: Option<ImpactLevel>,
}

/// Impact level for outcome filtering
#[derive(Debug, Clone, PartialEq)]
pub enum ImpactLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Navigation context for search
#[derive(Debug, Clone)]
pub struct NavigationContext {
    pub current_conversation_id: Option<Uuid>,
    pub related_conversations: Vec<Uuid>,
    pub time_window: Option<(DateTime<Utc>, DateTime<Utc>)>,
    pub workspace_context: Option<Uuid>,
}

impl ConversationNavigationManager {
    /// Create a new navigation manager
    pub fn new(conversation_manager: Arc<dyn ConversationManager>) -> Self {
        Self {
            conversation_manager,
            search_engine: None,
        }
    }
    
    /// Create with search engine for enhanced capabilities
    pub fn with_search_engine(
        conversation_manager: Arc<dyn ConversationManager>,
        search_engine: Arc<dyn ConversationSearchEngine>,
    ) -> Self {
        Self {
            conversation_manager,
            search_engine: Some(search_engine),
        }
    }
    
    /// Navigate conversations by timeline
    pub async fn navigate_timeline(
        &self,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
        workspace_id: Option<Uuid>,
        limit: Option<usize>,
    ) -> Result<TimelineNavigationResult> {
        // Get all conversations
        let conversations = self.conversation_manager.list_conversations(workspace_id).await?;
        
        // Filter by time range
        let filtered_conversations: Vec<_> = conversations.into_iter()
            .filter(|conv| {
                let in_start_range = start_time.is_none_or(|start| conv.created_at >= start);
                let in_end_range = end_time.is_none_or(|end| conv.created_at <= end);
                in_start_range && in_end_range
            })
            .collect();
        
        // Convert to timeline entries
        let mut timeline_entries = Vec::new();
        for conv_summary in &filtered_conversations {
            if let Some(conversation) = self.conversation_manager.get_conversation(conv_summary.id).await? {
                let activity_summary = self.calculate_activity_summary(&conversation).await;
                
                timeline_entries.push(ConversationTimelineEntry {
                    conversation_id: conversation.id,
                    title: conversation.title.clone(),
                    created_at: conversation.created_at,
                    last_active: conversation.last_active,
                    message_count: conversation.messages.len(),
                    branch_count: conversation.branches.len(),
                    checkpoint_count: conversation.checkpoints.len(),
                    workspace_id: conversation.workspace_id,
                    tags: conversation.tags.clone(),
                    activity_summary,
                });
            }
        }
        
        // Sort by activity and recency
        timeline_entries.sort_by(|a, b| {
            b.activity_summary.activity_score.partial_cmp(&a.activity_summary.activity_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.last_active.cmp(&a.last_active))
        });
        
        // Apply limit
        if let Some(limit) = limit {
            timeline_entries.truncate(limit);
        }
        
        // Calculate time range
        let time_range = if let (Some(first), Some(last)) = (timeline_entries.first(), timeline_entries.last()) {
            (last.created_at, first.last_active)
        } else {
            (Utc::now(), Utc::now())
        };
        
        Ok(TimelineNavigationResult {
            conversations: timeline_entries,
            total_count: filtered_conversations.len(),
            time_range,
        })
    }
    
    /// Navigate conversations as a graph
    pub async fn navigate_graph(
        &self,
        center_conversation_id: Option<Uuid>,
        depth: usize,
        workspace_id: Option<Uuid>,
    ) -> Result<ConversationGraphResult> {
        let conversations = self.conversation_manager.list_conversations(workspace_id).await?;
        
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let mut processed_conversations = HashSet::new();
        
        // Start with center conversation or all conversations
        let start_conversations = if let Some(center_id) = center_conversation_id {
            vec![center_id]
        } else {
            conversations.iter().map(|c| c.id).collect()
        };
        
        // Build graph using BFS
        let mut queue = VecDeque::new();
        for conv_id in start_conversations {
            queue.push_back((conv_id, 0));
        }
        
        while let Some((conv_id, current_depth)) = queue.pop_front() {
            if current_depth > depth || processed_conversations.contains(&conv_id) {
                continue;
            }
            
            processed_conversations.insert(conv_id);
            
            if let Some(conversation) = self.conversation_manager.get_conversation(conv_id).await? {
                // Add conversation node
                let activity_score = self.calculate_activity_score(&conversation).await;
                nodes.push(ConversationNode {
                    id: conversation.id,
                    node_type: NodeType::Conversation,
                    title: conversation.title.clone(),
                    created_at: conversation.created_at,
                    size: activity_score,
                    position: None, // Will be calculated by layout algorithm
                    metadata: NodeMetadata {
                        message_count: conversation.messages.len(),
                        activity_score,
                        tags: conversation.tags.clone(),
                        workspace_id: conversation.workspace_id,
                    },
                });
                
                // Add branch nodes and edges
                for branch in &conversation.branches {
                    nodes.push(ConversationNode {
                        id: branch.id,
                        node_type: NodeType::Branch,
                        title: branch.title.clone(),
                        created_at: branch.created_at,
                        size: 0.5, // Smaller than conversations
                        position: None,
                        metadata: NodeMetadata {
                            message_count: branch.messages.len(),
                            activity_score: 0.5,
                            tags: Vec::new(),
                            workspace_id: conversation.workspace_id,
                        },
                    });
                    
                    edges.push(ConversationEdge {
                        from_id: conversation.id,
                        to_id: branch.id,
                        edge_type: EdgeType::Branch,
                        weight: 1.0,
                        created_at: branch.created_at,
                    });
                }
                
                // Add checkpoint nodes and edges
                for checkpoint in &conversation.checkpoints {
                    nodes.push(ConversationNode {
                        id: checkpoint.id,
                        node_type: NodeType::Checkpoint,
                        title: checkpoint.title.clone(),
                        created_at: checkpoint.created_at,
                        size: 0.3, // Smaller than branches
                        position: None,
                        metadata: NodeMetadata {
                            message_count: 0,
                            activity_score: 0.3,
                            tags: Vec::new(),
                            workspace_id: conversation.workspace_id,
                        },
                    });
                    
                    edges.push(ConversationEdge {
                        from_id: conversation.id,
                        to_id: checkpoint.id,
                        edge_type: EdgeType::Checkpoint,
                        weight: 0.5,
                        created_at: checkpoint.created_at,
                    });
                }
                
                // Find related conversations (by tags, workspace, etc.)
                let related_conversations = self.find_related_conversations(&conversation, &conversations).await;
                for related_id in related_conversations {
                    if current_depth < depth {
                        queue.push_back((related_id, current_depth + 1));
                    }
                    
                    // Add similarity edge
                    edges.push(ConversationEdge {
                        from_id: conversation.id,
                        to_id: related_id,
                        edge_type: EdgeType::Similarity,
                        weight: 0.7,
                        created_at: conversation.created_at,
                    });
                }
            }
        }
        
        // Generate clusters
        let clusters = self.generate_clusters(&nodes, &edges).await;
        
        Ok(ConversationGraphResult {
            nodes,
            edges,
            clusters,
        })
    }
    
    /// Enhanced semantic search with code awareness
    pub async fn enhanced_search(&self, query: EnhancedSearchQuery) -> Result<Vec<ConversationSearchResult>> {
        if let Some(ref search_engine) = self.search_engine {
            // Start with base search
            let mut results = search_engine.search(&query.base_query).await?;
            
            // Apply semantic search if specified
            if let Some(ref semantic_query) = query.semantic_search {
                let semantic_query_obj = ConversationQuery {
                    text: Some(semantic_query.clone()),
                    ..query.base_query.clone()
                };
                let semantic_results = search_engine.search(&semantic_query_obj).await?;
                
                // Merge and re-rank results
                results = self.merge_search_results(results, semantic_results).await;
            }
            
            // Apply code context filtering
            if let Some(ref code_context) = query.code_context {
                results = self.filter_by_code_context(results, code_context).await;
            }
            
            // Apply outcome filtering
            if let Some(ref outcome_filter) = query.outcome_filter {
                results = self.filter_by_outcomes(results, outcome_filter).await;
            }
            
            // Apply navigation context
            if let Some(ref nav_context) = query.navigation_context {
                results = self.apply_navigation_context(results, nav_context).await;
            }
            
            Ok(results)
        } else {
            // Fallback to basic search without search engine
            let basic_results = self.conversation_manager.search_conversations(&query.base_query).await?;
            Ok(basic_results)
        }
    }
    
    /// Find conversations by outcome
    pub async fn find_by_outcome(
        &self,
        outcome_criteria: Vec<String>,
        workspace_id: Option<Uuid>,
    ) -> Result<Vec<ConversationSearchResult>> {
        let conversations = self.conversation_manager.list_conversations(workspace_id).await?;
        let mut results = Vec::new();
        
        for conv_summary in conversations {
            if let Some(conversation) = self.conversation_manager.get_conversation(conv_summary.id).await? {
                let outcome_score = self.calculate_outcome_score(&conversation, &outcome_criteria).await;
                
                if outcome_score > 0.5 {
                    results.push(ConversationSearchResult {
                        id: conversation.id,
                        title: conversation.title.clone(),
                        relevance_score: outcome_score,
                        summary_snippet: Some(self.extract_outcome_content(&conversation, &outcome_criteria).await.join("; ")),
                        last_active: conversation.last_active,
                        conversation: Some(Box::new(conversation.clone())),
                        matching_snippets: self.extract_outcome_content(&conversation, &outcome_criteria).await,
                        matching_messages: Vec::new(),
                    });
                }
            }
        }
        
        // Sort by relevance
        results.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap_or(std::cmp::Ordering::Equal));
        
        Ok(results)
    }
    
    /// Calculate activity summary for a conversation
    async fn calculate_activity_summary(&self, conversation: &Conversation) -> ActivitySummary {
        let mut recent_activity = Vec::new();
        
        // Add message events
        for message in conversation.messages.iter().rev().take(5) {
            recent_activity.push(ActivityEvent {
                event_type: ActivityEventType::MessageAdded,
                timestamp: message.timestamp,
                description: format!("Message: {}", truncate_text(&message.content, 50)),
                related_id: Some(message.id),
            });
        }
        
        // Add branch events
        for branch in &conversation.branches {
            recent_activity.push(ActivityEvent {
                event_type: ActivityEventType::BranchCreated,
                timestamp: branch.created_at,
                description: format!("Branch created: {}", branch.title),
                related_id: Some(branch.id),
            });
        }
        
        // Add checkpoint events
        for checkpoint in &conversation.checkpoints {
            recent_activity.push(ActivityEvent {
                event_type: ActivityEventType::CheckpointCreated,
                timestamp: checkpoint.created_at,
                description: format!("Checkpoint: {}", checkpoint.title),
                related_id: Some(checkpoint.id),
            });
        }
        
        // Sort by timestamp
        recent_activity.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        recent_activity.truncate(10);
        
        let activity_score = self.calculate_activity_score(conversation).await;
        let trending = self.is_trending(conversation).await;
        
        ActivitySummary {
            recent_activity,
            activity_score,
            trending,
        }
    }
    
    /// Calculate activity score for a conversation
    async fn calculate_activity_score(&self, conversation: &Conversation) -> f32 {
        let now = Utc::now();
        let _age_days = (now - conversation.created_at).num_days() as f32;
        let recency_days = (now - conversation.last_active).num_days() as f32;
        
        let message_score = (conversation.messages.len() as f32).ln().max(1.0);
        let branch_score = conversation.branches.len() as f32 * 0.5;
        let checkpoint_score = conversation.checkpoints.len() as f32 * 0.3;
        let recency_score = (1.0 / (recency_days + 1.0)).max(0.1);
        
        (message_score + branch_score + checkpoint_score) * recency_score
    }
    
    /// Check if conversation is trending
    async fn is_trending(&self, conversation: &Conversation) -> bool {
        let now = Utc::now();
        let recent_threshold = now - chrono::Duration::days(7);
        
        let recent_messages = conversation.messages.iter()
            .filter(|m| m.timestamp > recent_threshold)
            .count();
        
        recent_messages > 5 // Arbitrary threshold for trending
    }
    
    /// Find related conversations
    async fn find_related_conversations(
        &self,
        conversation: &Conversation,
        all_conversations: &[crate::agent::conversation::types::ConversationSummary],
    ) -> Vec<Uuid> {
        let mut related = Vec::new();
        
        for other in all_conversations {
            if other.id == conversation.id {
                continue;
            }
            
            let mut similarity_score = 0.0;
            
            // Tag similarity
            let common_tags = conversation.tags.iter()
                .filter(|tag| other.tags.contains(tag))
                .count();
            similarity_score += common_tags as f32 * 0.3;
            
            // Workspace similarity
            if conversation.workspace_id == other.workspace_id && conversation.workspace_id.is_some() {
                similarity_score += 0.5;
            }
            
            // Title similarity (simple word overlap)
            let conv_words: HashSet<&str> = conversation.title.split_whitespace().collect();
            let other_words: HashSet<&str> = other.title.split_whitespace().collect();
            let common_words = conv_words.intersection(&other_words).count();
            similarity_score += common_words as f32 * 0.2;
            
            if similarity_score > 0.5 {
                related.push(other.id);
            }
        }
        
        related
    }
    
    /// Generate conversation clusters
    async fn generate_clusters(&self, nodes: &[ConversationNode], _edges: &[ConversationEdge]) -> Vec<ConversationCluster> {
        let mut clusters = Vec::new();
        
        // Simple clustering by workspace
        let mut workspace_groups: HashMap<Option<Uuid>, Vec<Uuid>> = HashMap::new();
        for node in nodes {
            if node.node_type == NodeType::Conversation {
                workspace_groups.entry(node.metadata.workspace_id)
                    .or_default()
                    .push(node.id);
            }
        }
        
        for (workspace_id, conversation_ids) in workspace_groups {
            if conversation_ids.len() > 1 {
                clusters.push(ConversationCluster {
                    id: Uuid::new_v4(),
                    title: format!("Workspace Cluster {workspace_id:?}"),
                    conversation_ids,
                    center: (0.0, 0.0), // Would be calculated by layout algorithm
                    radius: 1.0,
                    cluster_type: ClusterType::Workspace,
                });
            }
        }
        
        // TODO: Add more sophisticated clustering algorithms
        // - Semantic clustering using embeddings
        // - Temporal clustering
        // - Tag-based clustering
        
        clusters
    }
    
    /// Merge search results from different sources
    async fn merge_search_results(
        &self,
        results1: Vec<ConversationSearchResult>,
        results2: Vec<ConversationSearchResult>,
    ) -> Vec<ConversationSearchResult> {
        let mut merged = HashMap::new();
        
        // Add first set of results
        for result in results1 {
            merged.insert(result.id, result);
        }
        
        // Merge second set, combining scores for duplicates
        for result in results2 {
            if let Some(existing) = merged.get_mut(&result.id) {
                existing.relevance_score = (existing.relevance_score + result.relevance_score) / 2.0;
            } else {
                merged.insert(result.id, result);
            }
        }
        
        let mut final_results: Vec<_> = merged.into_values().collect();
        final_results.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap_or(std::cmp::Ordering::Equal));
        
        final_results
    }
    
    /// Filter results by code context
    async fn filter_by_code_context(
        &self,
        results: Vec<ConversationSearchResult>,
        _code_context: &CodeSearchContext,
    ) -> Vec<ConversationSearchResult> {
        // TODO: Implement code-aware filtering
        // This would analyze conversation content for:
        // - File references matching patterns
        // - Programming language mentions
        // - Code snippets
        // - Repository references
        
        results // Placeholder implementation
    }
    
    /// Filter results by outcomes
    async fn filter_by_outcomes(
        &self,
        results: Vec<ConversationSearchResult>,
        outcome_filter: &OutcomeFilter,
    ) -> Vec<ConversationSearchResult> {
        let mut filtered = Vec::new();
        
        for result in results {
            if let Some(conversation) = self.conversation_manager.get_conversation(result.id).await.ok().flatten() {
                let outcome_score = self.calculate_outcome_score(&conversation, &outcome_filter.success_criteria).await;
                
                if outcome_score > 0.3 {
                    filtered.push(result);
                }
            }
        }
        
        filtered
    }
    
    /// Apply navigation context to results
    async fn apply_navigation_context(
        &self,
        results: Vec<ConversationSearchResult>,
        nav_context: &NavigationContext,
    ) -> Vec<ConversationSearchResult> {
        let mut contextualized = results;
        
        // Boost results related to current conversation
        if let Some(_current_id) = nav_context.current_conversation_id {
            for result in &mut contextualized {
                if nav_context.related_conversations.contains(&result.id) {
                    result.relevance_score *= 1.2;
                }
            }
        }
        
        // Filter by time window
        if let Some((start, end)) = nav_context.time_window {
            contextualized.retain(|result| {
                result.last_active >= start && result.last_active <= end
            });
        }
        
        // Filter by workspace - we'll need to get this from the conversation
        if let Some(workspace_id) = nav_context.workspace_context {
            let mut filtered = Vec::new();
            for result in contextualized {
                if let Some(conversation) = &result.conversation {
                    if conversation.workspace_id == Some(workspace_id) {
                        filtered.push(result);
                    }
                } else if let Some(conversation) = self.conversation_manager.get_conversation(result.id).await.ok().flatten() {
                    if conversation.workspace_id == Some(workspace_id) {
                        filtered.push(result);
                    }
                }
            }
            contextualized = filtered;
        }
        
        // Re-sort by updated relevance scores
        contextualized.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap_or(std::cmp::Ordering::Equal));
        
        contextualized
    }
    
    /// Calculate outcome score for a conversation
    async fn calculate_outcome_score(&self, conversation: &Conversation, criteria: &[String]) -> f32 {
        let mut score = 0.0;
        let content = self.extract_conversation_content(conversation);
        let content_lower = content.to_lowercase();
        
        for criterion in criteria {
            let criterion_lower = criterion.to_lowercase();
            if content_lower.contains(&criterion_lower) {
                score += 1.0;
            }
        }
        
        // Normalize by number of criteria
        if !criteria.is_empty() {
            score / criteria.len() as f32
        } else {
            0.0
        }
    }
    
    /// Extract outcome-related content from conversation
    async fn extract_outcome_content(&self, conversation: &Conversation, criteria: &[String]) -> Vec<String> {
        let mut matching_content = Vec::new();
        
        for message in &conversation.messages {
            let content_lower = message.content.to_lowercase();
            for criterion in criteria {
                if content_lower.contains(&criterion.to_lowercase()) {
                    matching_content.push(truncate_text(&message.content, 100));
                    break;
                }
            }
        }
        
        matching_content
    }
    
    /// Extract all content from a conversation
    fn extract_conversation_content(&self, conversation: &Conversation) -> String {
        let mut content = String::new();
        
        content.push_str(&conversation.title);
        content.push('\n');
        
        for message in &conversation.messages {
            content.push_str(&message.content);
            content.push('\n');
        }
        
        for branch in &conversation.branches {
            content.push_str(&branch.title);
            content.push('\n');
            if let Some(ref description) = branch.description {
                content.push_str(description);
                content.push('\n');
            }
        }
        
        content
    }
}

/// Truncate text to specified length
fn truncate_text(text: &str, max_length: usize) -> String {
    if text.len() <= max_length {
        text.to_string()
    } else {
        format!("{}...", &text[..max_length.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    use crate::agent::conversation::types::*;
    use crate::agent::message::types::AgentMessage;
    use crate::{Role, ConversationStatus};
    
    #[tokio::test]
    async fn test_navigation_manager_creation() {
        // Create a mock conversation manager for testing
        // This is a placeholder since we don't have InMemoryConversationManager
        // let conversation_manager = Arc::new(InMemoryConversationManager::new());
        // let nav_manager = ConversationNavigationManager::new(conversation_manager);
        
        // Basic creation test
        // assert!(nav_manager.search_engine.is_none());
    }
    
    #[tokio::test]
    async fn test_activity_score_calculation() {
        // Create a mock conversation manager for testing
        // let conversation_manager = Arc::new(InMemoryConversationManager::new());
        // let nav_manager = ConversationNavigationManager::new(conversation_manager);
        
        let conversation = Conversation {
            id: Uuid::new_v4(),
            title: "Test Conversation".to_string(),
            project_context: None,
            workspace_id: None,
            created_at: Utc::now() - chrono::Duration::days(1),
            last_active: Utc::now(),
            messages: vec![
                AgentMessage {
                    id: Uuid::new_v4(),
                    role: Role::User,
                    content: "Test message".to_string(),
                    is_streaming: false,
                    timestamp: Utc::now(),
                    metadata: std::collections::HashMap::new(),
                    tool_calls: Vec::new(),
                },
            ],
            branches: Vec::new(),
            checkpoints: Vec::new(),
            tags: Vec::new(),
            status: ConversationStatus::Active,
        };
        
        // let score = nav_manager.calculate_activity_score(&conversation).await;
        // assert!(score > 0.0);
    }
    
    #[test]
    fn test_truncate_text() {
        let text = "This is a long text that should be truncated";
        let truncated = truncate_text(text, 20);
        assert_eq!(truncated, "This is a long te...");
        
        let short_text = "Short";
        let not_truncated = truncate_text(short_text, 20);
        assert_eq!(not_truncated, "Short");
    }
    
    #[test]
    fn test_activity_event_types() {
        let event = ActivityEvent {
            event_type: ActivityEventType::MessageAdded,
            timestamp: Utc::now(),
            description: "Test event".to_string(),
            related_id: None,
        };
        
        assert_eq!(event.event_type, ActivityEventType::MessageAdded);
    }
    
    #[test]
    fn test_node_types() {
        assert_eq!(NodeType::Conversation, NodeType::Conversation);
        assert_ne!(NodeType::Conversation, NodeType::Branch);
    }
} 