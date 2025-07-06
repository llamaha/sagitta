use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use super::{TagSuggestion, TagSource};
use super::suggester::{TagSuggester, TagSuggesterConfig};
use super::rules::{RuleBasedTagger, TagRule};
use super::ui::{TagManagementUI, TagUIAction};
use crate::agent::conversation::types::{Conversation, ConversationSummary};
use crate::agent::conversation::manager::ConversationManager;

/// Configuration for the tagging pipeline
#[derive(Debug, Clone)]
pub struct TaggingPipelineConfig {
    /// Whether to auto-apply high-confidence tags
    pub auto_apply_enabled: bool,
    
    /// Minimum confidence threshold for auto-applying tags
    pub auto_apply_threshold: f32,
    
    /// Maximum number of tags to apply per conversation
    pub max_tags_per_conversation: usize,
    
    /// Whether to run tagging on conversation creation
    pub tag_on_creation: bool,
    
    /// Whether to run tagging on conversation update
    pub tag_on_update: bool,
    
    /// Minimum number of messages before running tagging
    pub min_messages_for_tagging: usize,
    
    /// Whether to preserve manual tags
    pub preserve_manual_tags: bool,
}

impl Default for TaggingPipelineConfig {
    fn default() -> Self {
        Self {
            auto_apply_enabled: true,
            auto_apply_threshold: 0.7,
            max_tags_per_conversation: 10,
            tag_on_creation: false, // Don't tag empty conversations
            tag_on_update: true,
            min_messages_for_tagging: 3,
            preserve_manual_tags: true,
        }
    }
}

/// Metadata about applied tags
#[derive(Debug, Clone)]
pub struct TagMetadata {
    pub tag: String,
    pub source: TagSource,
    pub confidence: f32,
    pub applied_at: DateTime<Utc>,
    pub auto_applied: bool,
    pub user_accepted: Option<bool>,
}

/// Result of tagging pipeline execution
#[derive(Debug, Clone)]
pub struct TaggingResult {
    pub conversation_id: Uuid,
    pub suggestions_generated: Vec<TagSuggestion>,
    pub tags_applied: Vec<String>,
    pub tags_rejected: Vec<String>,
    pub metadata: Vec<TagMetadata>,
}

/// Main tagging pipeline that coordinates all tagging components
pub struct TaggingPipeline {
    config: TaggingPipelineConfig,
    tag_suggester: Option<TagSuggester>,
    rule_tagger: RuleBasedTagger,
    conversation_manager: Arc<dyn ConversationManager>,
    tag_metadata: Arc<RwLock<HashMap<Uuid, Vec<TagMetadata>>>>,
    ui_states: Arc<RwLock<HashMap<Uuid, TagManagementUI>>>,
}

impl TaggingPipeline {
    /// Create a new tagging pipeline
    pub fn new(
        config: TaggingPipelineConfig,
        conversation_manager: Arc<dyn ConversationManager>,
    ) -> Self {
        let rule_tagger = RuleBasedTagger::with_default_rules();
        
        Self {
            config,
            tag_suggester: None,
            rule_tagger,
            conversation_manager,
            tag_metadata: Arc::new(RwLock::new(HashMap::new())),
            ui_states: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Create with tag suggester for embedding-based suggestions
    pub fn with_tag_suggester(mut self, suggester: TagSuggester) -> Self {
        self.tag_suggester = Some(suggester);
        self
    }
    
    /// Add a custom rule to the rule-based tagger
    pub fn add_rule(&mut self, rule: TagRule) {
        self.rule_tagger.add_rule(rule);
    }
    
    /// Process a conversation and apply tags
    pub async fn process_conversation(&self, conversation_id: Uuid) -> Result<TaggingResult> {
        let conversation = self.conversation_manager
            .get_conversation(conversation_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Conversation not found: {}", conversation_id))?;
        
        // Check if we should process this conversation
        if !self.should_process_conversation(&conversation) {
            return Ok(TaggingResult {
                conversation_id,
                suggestions_generated: Vec::new(),
                tags_applied: Vec::new(),
                tags_rejected: Vec::new(),
                metadata: Vec::new(),
            });
        }
        
        // Generate suggestions from all sources
        let mut all_suggestions = Vec::new();
        
        // Get suggestions from tag suggester (embedding-based)
        if let Some(ref suggester) = self.tag_suggester {
            match suggester.suggest_tags(&conversation).await {
                Ok(suggestions) => all_suggestions.extend(suggestions),
                Err(e) => {
                    log::warn!("Failed to get embedding-based tag suggestions: {e}");
                }
            }
        }
        
        // Get suggestions from rule-based tagger
        let rule_suggestions = self.rule_tagger.suggest_tags(&conversation);
        all_suggestions.extend(rule_suggestions);
        
        // Remove duplicates and sort by confidence
        all_suggestions = self.deduplicate_suggestions(all_suggestions);
        all_suggestions.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
        
        // Limit suggestions
        all_suggestions.truncate(self.config.max_tags_per_conversation);
        
        // Apply tags based on configuration
        let (applied_tags, rejected_tags, metadata) = self.apply_tags(&conversation, &all_suggestions).await?;
        
        // Update conversation with new tags
        if !applied_tags.is_empty() {
            let mut updated_conversation = conversation.clone();
            for tag in &applied_tags {
                if !updated_conversation.tags.contains(tag) {
                    updated_conversation.tags.push(tag.clone());
                }
            }
            self.conversation_manager.update_conversation(updated_conversation).await?;
        }
        
        // Store metadata
        {
            let mut metadata_map = self.tag_metadata.write().await;
            metadata_map.insert(conversation_id, metadata.clone());
        }
        
        Ok(TaggingResult {
            conversation_id,
            suggestions_generated: all_suggestions,
            tags_applied: applied_tags,
            tags_rejected: rejected_tags,
            metadata,
        })
    }
    
    /// Process a conversation summary (lighter version)
    pub async fn process_conversation_summary(&self, summary: &ConversationSummary) -> Result<Vec<TagSuggestion>> {
        let mut suggestions = Vec::new();
        
        // Get suggestions from tag suggester
        if let Some(ref suggester) = self.tag_suggester {
            match suggester.suggest_tags_for_summary(summary).await {
                Ok(summary_suggestions) => suggestions.extend(summary_suggestions),
                Err(e) => {
                    log::warn!("Failed to get embedding-based tag suggestions for summary: {e}");
                }
            }
        }
        
        // Get suggestions from rule-based tagger
        let rule_suggestions = self.rule_tagger.suggest_tags_for_summary(summary);
        suggestions.extend(rule_suggestions);
        
        // Remove duplicates and sort
        suggestions = self.deduplicate_suggestions(suggestions);
        suggestions.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
        
        Ok(suggestions)
    }
    
    /// Get or create UI state for a conversation
    pub async fn get_ui_state(&self, conversation_id: Uuid) -> TagManagementUI {
        let mut ui_states = self.ui_states.write().await;
        let ui = ui_states.entry(conversation_id)
            .or_insert_with(|| {
                let mut ui = TagManagementUI::new();
                ui.set_conversation(conversation_id);
                ui
            });
        
        // Create a new UI with the same state since we can't clone
        let mut new_ui = TagManagementUI::new();
        new_ui.set_conversation(conversation_id);
        new_ui.import_suggestion_history(ui.export_suggestion_history());
        new_ui
    }
    
    /// Handle UI action for tag management
    pub async fn handle_ui_action(&self, conversation_id: Uuid, action: TagUIAction) -> Result<Vec<String>> {
        let mut ui_states = self.ui_states.write().await;
        let ui = ui_states.entry(conversation_id)
            .or_insert_with(|| {
                let mut ui = TagManagementUI::new();
                ui.set_conversation(conversation_id);
                ui
            });
        
        let applied_tags = ui.handle_action(action);
        
        // If tags were applied, update the conversation
        if !applied_tags.is_empty() {
            if let Ok(Some(mut conversation)) = self.conversation_manager.get_conversation(conversation_id).await {
                for tag in &applied_tags {
                    if !conversation.tags.contains(tag) {
                        conversation.tags.push(tag.clone());
                    }
                }
                self.conversation_manager.update_conversation(conversation).await?;
            }
        }
        
        Ok(applied_tags)
    }
    
    /// Get tag metadata for a conversation
    pub async fn get_tag_metadata(&self, conversation_id: Uuid) -> Vec<TagMetadata> {
        let metadata_map = self.tag_metadata.read().await;
        metadata_map.get(&conversation_id).cloned().unwrap_or_default()
    }
    
    /// Check if a conversation should be processed for tagging
    fn should_process_conversation(&self, conversation: &Conversation) -> bool {
        // Don't process if not enough messages
        if conversation.messages.len() < self.config.min_messages_for_tagging {
            return false;
        }
        
        // Don't process if conversation already has many tags
        if conversation.tags.len() >= self.config.max_tags_per_conversation {
            return false;
        }
        
        true
    }
    
    /// Apply tags based on suggestions and configuration
    async fn apply_tags(
        &self,
        conversation: &Conversation,
        suggestions: &[TagSuggestion],
    ) -> Result<(Vec<String>, Vec<String>, Vec<TagMetadata>)> {
        let mut applied_tags = Vec::new();
        let mut rejected_tags = Vec::new();
        let mut metadata = Vec::new();
        
        let existing_tags = &conversation.tags;
        
        for suggestion in suggestions {
            // Skip if tag already exists
            if existing_tags.contains(&suggestion.tag) {
                continue;
            }
            
            // Check if we should auto-apply this tag
            let should_auto_apply = self.config.auto_apply_enabled && 
                                   suggestion.confidence >= self.config.auto_apply_threshold;
            
            if should_auto_apply {
                applied_tags.push(suggestion.tag.clone());
                metadata.push(TagMetadata {
                    tag: suggestion.tag.clone(),
                    source: suggestion.source.clone(),
                    confidence: suggestion.confidence,
                    applied_at: Utc::now(),
                    auto_applied: true,
                    user_accepted: None,
                });
            } else {
                // Tag requires user approval
                rejected_tags.push(suggestion.tag.clone());
                metadata.push(TagMetadata {
                    tag: suggestion.tag.clone(),
                    source: suggestion.source.clone(),
                    confidence: suggestion.confidence,
                    applied_at: Utc::now(),
                    auto_applied: false,
                    user_accepted: None,
                });
            }
        }
        
        Ok((applied_tags, rejected_tags, metadata))
    }
    
    /// Remove duplicate suggestions, keeping the highest confidence one
    fn deduplicate_suggestions(&self, suggestions: Vec<TagSuggestion>) -> Vec<TagSuggestion> {
        let mut seen_tags = HashMap::new();
        let mut result = Vec::new();
        
        for suggestion in suggestions {
            match seen_tags.get(&suggestion.tag) {
                Some(&existing_confidence) => {
                    if suggestion.confidence > existing_confidence {
                        // Replace with higher confidence suggestion
                        seen_tags.insert(suggestion.tag.clone(), suggestion.confidence);
                        // Remove the old one and add the new one
                        result.retain(|s: &TagSuggestion| s.tag != suggestion.tag);
                        result.push(suggestion);
                    }
                },
                None => {
                    seen_tags.insert(suggestion.tag.clone(), suggestion.confidence);
                    result.push(suggestion);
                }
            }
        }
        
        result
    }
}

/// Builder for creating a tagging pipeline with custom configuration
pub struct TaggingPipelineBuilder {
    config: TaggingPipelineConfig,
    tag_suggester_config: Option<TagSuggesterConfig>,
    custom_rules: Vec<TagRule>,
}

impl TaggingPipelineBuilder {
    pub fn new() -> Self {
        Self {
            config: TaggingPipelineConfig::default(),
            tag_suggester_config: None,
            custom_rules: Vec::new(),
        }
    }
    
    pub fn with_config(mut self, config: TaggingPipelineConfig) -> Self {
        self.config = config;
        self
    }
    
    pub fn with_tag_suggester_config(mut self, config: TagSuggesterConfig) -> Self {
        self.tag_suggester_config = Some(config);
        self
    }
    
    pub fn add_rule(mut self, rule: TagRule) -> Self {
        self.custom_rules.push(rule);
        self
    }
    
    pub fn build(self, conversation_manager: Arc<dyn ConversationManager>) -> TaggingPipeline {
        let mut pipeline = TaggingPipeline::new(self.config, conversation_manager);
        
        // Add tag suggester if configured
        if let Some(suggester_config) = self.tag_suggester_config {
            let suggester = TagSuggester::new(suggester_config);
            pipeline = pipeline.with_tag_suggester(suggester);
        }
        
        // Add custom rules
        for rule in self.custom_rules {
            pipeline.add_rule(rule);
        }
        
        pipeline
    }
}

impl Default for TaggingPipelineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::conversation::types::{Conversation, ProjectContext, ProjectType};
    use crate::agent::message::types::AgentMessage;
    use crate::conversation::TagRuleType;
    use std::collections::HashMap;
    use uuid::Uuid;
    
    // Mock conversation manager for testing
    struct MockConversationManager {
        conversations: Arc<RwLock<HashMap<Uuid, Conversation>>>,
    }
    
    impl MockConversationManager {
        fn new() -> Self {
            Self {
                conversations: Arc::new(RwLock::new(HashMap::new())),
            }
        }
        
        async fn add_conversation(&self, conversation: Conversation) {
            let mut conversations = self.conversations.write().await;
            conversations.insert(conversation.id, conversation);
        }
    }
    
    #[async_trait::async_trait]
    impl ConversationManager for MockConversationManager {
        async fn create_conversation(&self, title: String, workspace_id: Option<Uuid>) -> Result<Uuid> {
            let conversation = Conversation::new(title, workspace_id);
            let id = conversation.id;
            self.add_conversation(conversation).await;
            Ok(id)
        }
        
        async fn get_conversation(&self, id: Uuid) -> Result<Option<Conversation>> {
            let conversations = self.conversations.read().await;
            Ok(conversations.get(&id).cloned())
        }
        
        async fn update_conversation(&self, conversation: Conversation) -> Result<()> {
            let mut conversations = self.conversations.write().await;
            conversations.insert(conversation.id, conversation);
            Ok(())
        }
        
        async fn delete_conversation(&self, _id: Uuid) -> Result<()> {
            Ok(())
        }
        
        async fn list_conversations(&self, _workspace_id: Option<Uuid>) -> Result<Vec<crate::agent::conversation::types::ConversationSummary>> {
            Ok(Vec::new())
        }
        
        async fn search_conversations(&self, _query: &crate::agent::conversation::types::ConversationQuery) -> Result<Vec<crate::agent::conversation::types::ConversationSearchResult>> {
            Ok(Vec::new())
        }
        
        async fn create_branch(&self, _conversation_id: Uuid, _parent_message_id: Option<Uuid>, _title: String) -> Result<Uuid> {
            Ok(Uuid::new_v4())
        }
        
        async fn merge_branch(&self, _conversation_id: Uuid, _branch_id: Uuid) -> Result<()> {
            Ok(())
        }
        
        async fn create_checkpoint(&self, _conversation_id: Uuid, _message_id: Uuid, _title: String) -> Result<Uuid> {
            Ok(Uuid::new_v4())
        }
        
        async fn restore_checkpoint(&self, _conversation_id: Uuid, _checkpoint_id: Uuid) -> Result<()> {
            Ok(())
        }
        
        async fn get_statistics(&self) -> Result<crate::agent::conversation::manager::ConversationStatistics> {
            Ok(crate::agent::conversation::manager::ConversationStatistics {
                total_conversations: 0,
                active_conversations: 0,
                total_messages: 0,
                total_branches: 0,
                total_checkpoints: 0,
                conversations_by_workspace: HashMap::new(),
                average_messages_per_conversation: 0.0,
            })
        }
        
        async fn archive_conversations(&self, _criteria: crate::agent::conversation::manager::ArchiveCriteria) -> Result<usize> {
            Ok(0)
        }
        
        async fn get_tag_suggestions(&self, _conversation_id: Uuid) -> Result<Vec<TagSuggestion>> {
            Ok(Vec::new())
        }
        
        async fn get_tag_metadata(&self, _conversation_id: Uuid) -> Result<Vec<TagMetadata>> {
            Ok(Vec::new())
        }
        
        async fn retag_conversation(&self, conversation_id: Uuid) -> Result<TaggingResult> {
            Ok(TaggingResult {
                conversation_id,
                suggestions_generated: Vec::new(),
                tags_applied: Vec::new(),
                tags_rejected: Vec::new(),
                metadata: Vec::new(),
            })
        }
    }
    
    fn create_test_conversation() -> Conversation {
        let mut conversation = Conversation::new("Test Rust Conversation".to_string(), None);
        conversation.project_context = Some(ProjectContext {
            name: "test-project".to_string(),
            project_type: ProjectType::Rust,
            root_path: None,
            description: Some("Test project".to_string()),
            repositories: Vec::new(),
            settings: HashMap::new(),
        });
        
        conversation.messages = vec![
            AgentMessage::user("I'm having trouble with Rust"),
            AgentMessage::assistant("I can help with that"),
            AgentMessage::user("The error says 'cannot borrow as mutable'"),
            AgentMessage::assistant("This is a common ownership issue"),
        ];
        
        conversation
    }
    
    #[tokio::test]
    async fn test_pipeline_creation() {
        let manager = Arc::new(MockConversationManager::new());
        let pipeline = TaggingPipeline::new(TaggingPipelineConfig::default(), manager);
        
        // Should be created successfully
        assert!(pipeline.tag_suggester.is_none()); // No suggester by default
    }
    
    #[tokio::test]
    async fn test_process_conversation() {
        let manager = Arc::new(MockConversationManager::new());
        let conversation = create_test_conversation();
        let conversation_id = conversation.id;
        
        manager.add_conversation(conversation).await;
        
        let pipeline = TaggingPipeline::new(TaggingPipelineConfig::default(), manager);
        
        let result = pipeline.process_conversation(conversation_id).await.unwrap();
        
        // Should have generated some suggestions
        assert!(!result.suggestions_generated.is_empty());
        
        // Should have applied some tags (auto-apply enabled by default)
        assert!(!result.tags_applied.is_empty());
        
        // Should have rust tag from project type
        assert!(result.tags_applied.contains(&"rust".to_string()));
    }
    
    #[tokio::test]
    async fn test_deduplication() {
        let manager = Arc::new(MockConversationManager::new());
        let pipeline = TaggingPipeline::new(TaggingPipelineConfig::default(), manager);
        
        let suggestions = vec![
            TagSuggestion::new(
                "rust".to_string(),
                0.8,
                "High confidence".to_string(),
                TagSource::Content { keywords: vec![] }
            ),
            TagSuggestion::new(
                "rust".to_string(),
                0.9,
                "Higher confidence".to_string(),
                TagSource::Rule { rule_name: "test".to_string() }
            ),
        ];
        
        let deduplicated = pipeline.deduplicate_suggestions(suggestions);
        
        // Should have only one rust tag with higher confidence
        assert_eq!(deduplicated.len(), 1);
        assert_eq!(deduplicated[0].confidence, 0.9);
    }
    
    #[tokio::test]
    async fn test_builder_pattern() {
        let manager = Arc::new(MockConversationManager::new());
        
        let pipeline = TaggingPipelineBuilder::new()
            .with_config(TaggingPipelineConfig {
                auto_apply_threshold: 0.8,
                ..Default::default()
            })
            .add_rule(TagRule {
                name: "test_rule".to_string(),
                tag: "test".to_string(),
                description: "Test rule".to_string(),
                rule_type: TagRuleType::KeywordMatch {
                    keywords: vec!["test".to_string()],
                    case_sensitive: false,
                },
                confidence: 0.9,
                enabled: true,
                priority: 100,
            })
            .build(manager);
        
        // Should have the custom configuration
        assert_eq!(pipeline.config.auto_apply_threshold, 0.8);
    }
} 