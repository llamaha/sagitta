use super::{TagSuggestion, TagSource};
use crate::agent::conversation::types::{Conversation, ConversationSummary};
use crate::agent::message::types::AgentMessage;
use crate::llm::fast_model::FastModelOperations;
use sagitta_embed::{EmbeddingPool, EmbeddingConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Configuration for the tag suggester
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagSuggesterConfig {
    /// Minimum similarity threshold for tag suggestions
    pub similarity_threshold: f32,
    /// Maximum number of suggestions per conversation
    pub max_suggestions: usize,
    /// Minimum confidence for auto-applying tags
    pub auto_apply_threshold: f32,
    /// Whether to use content analysis for tag generation
    pub enable_content_analysis: bool,
    /// Whether to use historical tag patterns
    pub enable_pattern_learning: bool,
}

impl Default for TagSuggesterConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.4,
            max_suggestions: 5,
            auto_apply_threshold: 0.8,
            enable_content_analysis: true,
            enable_pattern_learning: true,
        }
    }
}

/// Tag corpus entry for similarity matching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagCorpusEntry {
    pub tag: String,
    pub embedding: Vec<f32>,
    pub usage_count: usize,
    pub success_rate: f32,
    pub last_used: DateTime<Utc>,
    pub example_conversations: Vec<Uuid>,
}

/// Embedding-based tag suggester
pub struct TagSuggester {
    config: TagSuggesterConfig,
    embedding_pool: Option<EmbeddingPool>,
    fast_model_provider: Option<Arc<dyn FastModelOperations>>,
    tag_corpus: HashMap<String, TagCorpusEntry>,
    conversation_embeddings: HashMap<Uuid, Vec<f32>>,
}

impl TagSuggester {
    /// Create a new tag suggester
    pub fn new(config: TagSuggesterConfig) -> Self {
        Self {
            config,
            embedding_pool: None,
            fast_model_provider: None,
            tag_corpus: HashMap::new(),
            conversation_embeddings: HashMap::new(),
        }
    }

    /// Initialize with embedding pool for semantic analysis
    pub fn with_embedding_pool(mut self, embedding_pool: EmbeddingPool) -> Self {
        self.embedding_pool = Some(embedding_pool);
        self
    }
    
    /// Set the fast model provider for tag generation
    pub fn with_fast_model_provider(mut self, provider: Arc<dyn FastModelOperations>) -> Self {
        self.fast_model_provider = Some(provider);
        self
    }

    /// Initialize from ONNX model files
    pub fn from_onnx_model(
        config: TagSuggesterConfig,
        model_path: PathBuf,
        tokenizer_path: PathBuf,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let embedding_config = EmbeddingConfig::new_onnx(model_path, tokenizer_path);
        let embedding_pool = EmbeddingPool::with_configured_sessions(embedding_config)?;
        
        Ok(Self {
            config,
            embedding_pool: Some(embedding_pool),
            fast_model_provider: None,
            tag_corpus: HashMap::new(),
            conversation_embeddings: HashMap::new(),
        })
    }

    /// Suggest tags for a conversation
    pub async fn suggest_tags(&self, conversation: &Conversation) -> Result<Vec<TagSuggestion>, Box<dyn std::error::Error>> {
        let mut suggestions = Vec::new();

        // Try fast model first if available
        if let Some(ref fast_provider) = self.fast_model_provider {
            match fast_provider.suggest_tags(conversation).await {
                Ok(fast_tags) => {
                    for (tag, confidence) in fast_tags {
                        suggestions.push(TagSuggestion::new(
                            tag,
                            confidence,
                            "Suggested by fast model".to_string(),
                            TagSource::Rule { rule_name: "fast_model".to_string() },
                        ));
                    }
                    log::debug!("Got {} tag suggestions from fast model", suggestions.len());
                }
                Err(e) => {
                    log::debug!("Fast model tag suggestion failed: {e}, falling back");
                }
            }
        }

        // Generate embedding-based suggestions if fast model not available or as supplement
        if suggestions.is_empty() {
            if let Some(embedding_suggestions) = self.suggest_from_embeddings(conversation).await? {
                suggestions.extend(embedding_suggestions);
            }
        }

        // Generate content-based suggestions
        if self.config.enable_content_analysis {
            let content_suggestions = self.suggest_from_content(conversation);
            suggestions.extend(content_suggestions);
        }

        // Generate pattern-based suggestions
        if self.config.enable_pattern_learning {
            let pattern_suggestions = self.suggest_from_patterns(conversation);
            suggestions.extend(pattern_suggestions);
        }

        // Sort by confidence and limit results
        suggestions.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
        suggestions.truncate(self.config.max_suggestions);

        // Filter by threshold
        suggestions.retain(|s| s.confidence >= self.config.similarity_threshold);

        Ok(suggestions)
    }

    /// Suggest tags for a conversation summary (lighter version)
    pub async fn suggest_tags_for_summary(&self, summary: &ConversationSummary) -> Result<Vec<TagSuggestion>, Box<dyn std::error::Error>> {
        let mut suggestions = Vec::new();

        // Use existing tags for similarity if available
        if !summary.tags.is_empty() {
            let similar_tags = self.find_similar_tags(&summary.tags).await?;
            suggestions.extend(similar_tags);
        }

        // Generate suggestions based on title and project
        let title_suggestions = self.suggest_from_title(&summary.title);
        suggestions.extend(title_suggestions);

        if let Some(project_name) = &summary.project_name {
            let project_suggestions = self.suggest_from_project(project_name);
            suggestions.extend(project_suggestions);
        }

        // Sort and limit
        suggestions.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
        suggestions.truncate(self.config.max_suggestions);
        suggestions.retain(|s| s.confidence >= self.config.similarity_threshold);

        Ok(suggestions)
    }

    /// Generate embedding-based suggestions
    async fn suggest_from_embeddings(&self, conversation: &Conversation) -> Result<Option<Vec<TagSuggestion>>, Box<dyn std::error::Error>> {
        let embedding_pool = match &self.embedding_pool {
            Some(pool) => pool,
            None => return Ok(None),
        };

        // Create conversation text for embedding
        let conversation_text = self.create_conversation_text(conversation);
        
        // Generate embedding
        let embeddings = embedding_pool.embed_texts_async(&[&conversation_text]).await?;
        let conversation_embedding = embeddings.into_iter().next()
            .ok_or("Failed to generate conversation embedding")?;

        // Find similar tags in corpus
        let mut suggestions = Vec::new();
        for (tag, corpus_entry) in &self.tag_corpus {
            let similarity = self.cosine_similarity(&conversation_embedding, &corpus_entry.embedding);
            
            if similarity >= self.config.similarity_threshold {
                let confidence = similarity * corpus_entry.success_rate;
                let reasoning = format!(
                    "Similar to {} conversations with this tag (similarity: {:.2}, success rate: {:.2})",
                    corpus_entry.usage_count, similarity, corpus_entry.success_rate
                );

                suggestions.push(TagSuggestion::new(
                    tag.clone(),
                    confidence,
                    reasoning,
                    TagSource::Embedding { similarity_score: similarity },
                ));
            }
        }

        Ok(Some(suggestions))
    }

    /// Generate content-based suggestions
    fn suggest_from_content(&self, conversation: &Conversation) -> Vec<TagSuggestion> {
        let mut suggestions = Vec::new();
        let conversation_text = self.create_conversation_text(conversation).to_lowercase();

        // Programming language detection
        let language_keywords = vec![
            ("rust", vec!["cargo", "rustc", "trait", "impl", "fn", "let mut", "match", "Result", "Option"]),
            ("python", vec!["def", "import", "class", "self", "pip", "python", "django", "flask", "pandas"]),
            ("javascript", vec!["function", "const", "let", "var", "npm", "node", "react", "vue", "angular"]),
            ("go", vec!["func", "package", "import", "go mod", "goroutine", "channel", "interface"]),
            ("java", vec!["public class", "private", "static", "void", "import java", "maven", "gradle"]),
        ];

        for (language, keywords) in language_keywords {
            let matches = keywords.iter().filter(|&keyword| conversation_text.contains(keyword)).count();
            if matches > 0 {
                let confidence = (matches as f32 / keywords.len() as f32).min(1.0) * 0.8;
                let reasoning = format!("Detected {matches} programming language keywords");
                
                suggestions.push(TagSuggestion::new(
                    language.to_string(),
                    confidence,
                    reasoning,
                    TagSource::Content { keywords: keywords.iter().map(|s| s.to_string()).collect() },
                ));
            }
        }

        // Topic detection
        let topic_keywords = vec![
            ("debugging", vec!["error", "bug", "fix", "debug", "issue", "problem", "crash"]),
            ("performance", vec!["slow", "fast", "optimize", "performance", "speed", "memory", "cpu"]),
            ("testing", vec!["test", "unit test", "integration", "mock", "assert", "spec"]),
            ("deployment", vec!["deploy", "production", "server", "docker", "kubernetes", "ci/cd"]),
            ("database", vec!["sql", "database", "query", "table", "index", "migration"]),
            ("api", vec!["api", "rest", "graphql", "endpoint", "request", "response"]),
            ("frontend", vec!["ui", "frontend", "css", "html", "component", "styling"]),
            ("backend", vec!["backend", "server", "service", "microservice", "architecture"]),
        ];

        for (topic, keywords) in topic_keywords {
            let matches = keywords.iter().filter(|&keyword| conversation_text.contains(keyword)).count();
            if matches > 0 {
                let confidence = (matches as f32 / keywords.len() as f32).min(1.0) * 0.6;
                let reasoning = format!("Detected {matches} topic-related keywords");
                
                suggestions.push(TagSuggestion::new(
                    topic.to_string(),
                    confidence,
                    reasoning,
                    TagSource::Content { keywords: keywords.iter().map(|s| s.to_string()).collect() },
                ));
            }
        }

        suggestions
    }

    /// Generate pattern-based suggestions
    fn suggest_from_patterns(&self, conversation: &Conversation) -> Vec<TagSuggestion> {
        let mut suggestions = Vec::new();

        // Project type patterns
        if let Some(project_context) = &conversation.project_context {
            let project_tag = format!("{:?}", project_context.project_type).to_lowercase();
            suggestions.push(TagSuggestion::new(
                project_tag,
                0.7,
                "Based on detected project type".to_string(),
                TagSource::Rule { rule_name: "project_type".to_string() },
            ));
        }

        // Message count patterns
        let message_count = conversation.messages.len();
        if message_count > 20 {
            suggestions.push(TagSuggestion::new(
                "long-conversation".to_string(),
                0.5,
                format!("Conversation has {message_count} messages"),
                TagSource::Rule { rule_name: "message_count".to_string() },
            ));
        }

        // Branch patterns
        if !conversation.branches.is_empty() {
            suggestions.push(TagSuggestion::new(
                "branched".to_string(),
                0.6,
                format!("Conversation has {} branches", conversation.branches.len()),
                TagSource::Rule { rule_name: "has_branches".to_string() },
            ));
        }

        suggestions
    }

    /// Find similar tags based on existing tags
    async fn find_similar_tags(&self, existing_tags: &[String]) -> Result<Vec<TagSuggestion>, Box<dyn std::error::Error>> {
        let mut suggestions = Vec::new();

        if let Some(_embedding_pool) = &self.embedding_pool {
            for tag in existing_tags {
                if let Some(corpus_entry) = self.tag_corpus.get(tag) {
                    // Find similar tags in corpus
                    for (other_tag, other_entry) in &self.tag_corpus {
                        if other_tag != tag {
                            let similarity = self.cosine_similarity(&corpus_entry.embedding, &other_entry.embedding);
                            if similarity >= self.config.similarity_threshold {
                                let confidence = similarity * 0.7; // Lower confidence for indirect similarity
                                let reasoning = format!("Similar to existing tag '{tag}' (similarity: {similarity:.2})");
                                
                                suggestions.push(TagSuggestion::new(
                                    other_tag.clone(),
                                    confidence,
                                    reasoning,
                                    TagSource::Embedding { similarity_score: similarity },
                                ));
                            }
                        }
                    }
                }
            }
        }

        Ok(suggestions)
    }

    /// Generate suggestions from title
    fn suggest_from_title(&self, title: &str) -> Vec<TagSuggestion> {
        let mut suggestions = Vec::new();
        let title_lower = title.to_lowercase();

        // Simple keyword matching in title
        let title_keywords = vec![
            ("question", vec!["how", "what", "why", "when", "where", "?"]),
            ("help", vec!["help", "assist", "support", "stuck"]),
            ("error", vec!["error", "fail", "broken", "issue", "problem"]),
            ("feature", vec!["add", "implement", "create", "build", "new"]),
            ("review", vec!["review", "check", "look", "feedback"]),
        ];

        for (tag, keywords) in title_keywords {
            if keywords.iter().any(|keyword| title_lower.contains(keyword)) {
                suggestions.push(TagSuggestion::new(
                    tag.to_string(),
                    0.5,
                    format!("Detected in title: '{title}'"),
                    TagSource::Content { keywords: keywords.iter().map(|s| s.to_string()).collect() },
                ));
            }
        }

        suggestions
    }

    /// Generate suggestions from project name
    fn suggest_from_project(&self, project_name: &str) -> Vec<TagSuggestion> {
        vec![TagSuggestion::new(
            project_name.to_lowercase(),
            0.6,
            format!("Based on project name: {project_name}"),
            TagSource::Rule { rule_name: "project_name".to_string() },
        )]
    }

    /// Create text representation of conversation for embedding
    fn create_conversation_text(&self, conversation: &Conversation) -> String {
        let mut parts = Vec::new();
        
        // Add title
        parts.push(conversation.title.clone());
        
        // Add first few messages (to avoid token limits)
        for message in conversation.messages.iter().take(5) {
            parts.push(message.content.clone());
        }
        
        // Add existing tags
        if !conversation.tags.is_empty() {
            parts.push(format!("Tags: {}", conversation.tags.join(", ")));
        }
        
        parts.join(" ")
    }

    /// Calculate cosine similarity between two vectors
    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }

        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot_product / (norm_a * norm_b)
        }
    }

    /// Add a tag to the corpus with its embedding
    pub async fn add_tag_to_corpus(&mut self, tag: String, example_conversations: Vec<Uuid>) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(embedding_pool) = &self.embedding_pool {
            // Generate embedding for the tag
            let embeddings = embedding_pool.embed_texts_async(&[&tag]).await?;
            let embedding = embeddings.into_iter().next()
                .ok_or("Failed to generate tag embedding")?;

            let corpus_entry = TagCorpusEntry {
                tag: tag.clone(),
                embedding,
                usage_count: example_conversations.len(),
                success_rate: 1.0, // Start with perfect success rate
                last_used: Utc::now(),
                example_conversations,
            };

            self.tag_corpus.insert(tag, corpus_entry);
        }

        Ok(())
    }

    /// Update tag success rate based on user feedback
    pub fn update_tag_success_rate(&mut self, tag: &str, accepted: bool) {
        if let Some(corpus_entry) = self.tag_corpus.get_mut(tag) {
            let current_total = corpus_entry.usage_count as f32;
            let current_successes = current_total * corpus_entry.success_rate;
            
            let new_successes = if accepted {
                current_successes + 1.0
            } else {
                current_successes
            };
            
            corpus_entry.usage_count += 1;
            corpus_entry.success_rate = new_successes / corpus_entry.usage_count as f32;
            corpus_entry.last_used = Utc::now();
        }
    }

    /// Get high-confidence auto-apply suggestions
    pub async fn get_auto_apply_suggestions(&self, conversation: &Conversation) -> Result<Vec<TagSuggestion>, Box<dyn std::error::Error>> {
        let all_suggestions = self.suggest_tags(conversation).await?;
        Ok(all_suggestions.into_iter()
            .filter(|s| s.confidence >= self.config.auto_apply_threshold)
            .collect())
    }

    /// Get corpus statistics
    pub fn get_corpus_stats(&self) -> HashMap<String, usize> {
        let mut stats = HashMap::new();
        stats.insert("total_tags".to_string(), self.tag_corpus.len());
        stats.insert("total_conversations".to_string(), self.conversation_embeddings.len());
        
        let avg_success_rate = if !self.tag_corpus.is_empty() {
            self.tag_corpus.values().map(|e| e.success_rate).sum::<f32>() / self.tag_corpus.len() as f32
        } else {
            0.0
        };
        stats.insert("avg_success_rate_percent".to_string(), (avg_success_rate * 100.0) as usize);
        
        stats
    }

    async fn analyze_content(&self, messages: &[AgentMessage]) -> Vec<String> {
        let mut content_tags = Vec::new();
        
        // Analyze programming languages mentioned
        let mut all_content = String::new();
        for message in messages {
            all_content.push_str(&message.content);
            all_content.push(' ');
        }
        
        // Add basic content analysis here
        let content_lower = all_content.to_lowercase();
        
        // Programming language detection
        if content_lower.contains("rust") || content_lower.contains("cargo") {
            content_tags.push("rust".to_string());
        }
        if content_lower.contains("python") || content_lower.contains("pip") {
            content_tags.push("python".to_string());
        }
        if content_lower.contains("javascript") || content_lower.contains("npm") {
            content_tags.push("javascript".to_string());
        }
        
        // Topic detection
        if content_lower.contains("error") || content_lower.contains("bug") {
            content_tags.push("debugging".to_string());
        }
        if content_lower.contains("test") || content_lower.contains("testing") {
            content_tags.push("testing".to_string());
        }

        content_tags
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::conversation::types::{Conversation, ProjectContext, ProjectType};
    use crate::agent::message::types::AgentMessage;
    use crate::agent::state::types::ConversationStatus;

    fn create_test_conversation() -> Conversation {
        let mut conversation = Conversation::new("Test Rust debugging".to_string(), None);
        conversation.project_context = Some(ProjectContext {
            name: "test-project".to_string(),
            project_type: ProjectType::Rust,
            root_path: None,
            description: None,
            repositories: vec![],
            settings: std::collections::HashMap::new(),
        });
        
        // Add a test message
        let message = AgentMessage {
            id: uuid::Uuid::new_v4(),
            content: "I'm having trouble with a Rust error in my cargo build".to_string(),
            role: crate::llm::client::Role::User,
            is_streaming: false,
            timestamp: chrono::Utc::now(),
            metadata: std::collections::HashMap::new(),
            tool_calls: vec![],
        };
        conversation.add_message(message);
        
        conversation
    }

    #[test]
    fn test_tag_suggester_creation() {
        let config = TagSuggesterConfig::default();
        let suggester = TagSuggester::new(config);
        
        assert!(suggester.embedding_pool.is_none());
        assert!(suggester.tag_corpus.is_empty());
    }

    #[test]
    fn test_content_based_suggestions() {
        let config = TagSuggesterConfig::default();
        let suggester = TagSuggester::new(config);
        let conversation = create_test_conversation();
        
        let suggestions = suggester.suggest_from_content(&conversation);
        
        // Should detect Rust and debugging
        assert!(suggestions.iter().any(|s| s.tag == "rust"));
        assert!(suggestions.iter().any(|s| s.tag == "debugging"));
    }

    #[test]
    fn test_pattern_based_suggestions() {
        let config = TagSuggesterConfig::default();
        let suggester = TagSuggester::new(config);
        let conversation = create_test_conversation();
        
        let suggestions = suggester.suggest_from_patterns(&conversation);
        
        // Should detect project type
        assert!(suggestions.iter().any(|s| s.tag == "rust"));
    }

    #[test]
    fn test_title_based_suggestions() {
        let config = TagSuggesterConfig::default();
        let suggester = TagSuggester::new(config);
        
        let suggestions = suggester.suggest_from_title("How to fix this error?");
        
        assert!(suggestions.iter().any(|s| s.tag == "question"));
        assert!(suggestions.iter().any(|s| s.tag == "error"));
    }

    #[test]
    fn test_cosine_similarity() {
        let config = TagSuggesterConfig::default();
        let suggester = TagSuggester::new(config);
        
        let vec_a = vec![1.0, 0.0, 0.0];
        let vec_b = vec![1.0, 0.0, 0.0];
        let vec_c = vec![0.0, 1.0, 0.0];
        
        assert!((suggester.cosine_similarity(&vec_a, &vec_b) - 1.0).abs() < 0.001);
        assert!((suggester.cosine_similarity(&vec_a, &vec_c) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_conversation_text_creation() {
        let config = TagSuggesterConfig::default();
        let suggester = TagSuggester::new(config);
        let conversation = create_test_conversation();
        
        let text = suggester.create_conversation_text(&conversation);
        
        assert!(text.contains("Test Rust debugging"));
        assert!(text.contains("Rust error"));
    }

    #[test]
    fn test_tag_success_rate_update() {
        let config = TagSuggesterConfig::default();
        let mut suggester = TagSuggester::new(config);
        
        // Add a tag to corpus
        let corpus_entry = TagCorpusEntry {
            tag: "rust".to_string(),
            embedding: vec![1.0, 0.0, 0.0],
            usage_count: 1,
            success_rate: 1.0,
            last_used: Utc::now(),
            example_conversations: vec![],
        };
        suggester.tag_corpus.insert("rust".to_string(), corpus_entry);
        
        // Update with acceptance
        suggester.update_tag_success_rate("rust", true);
        assert_eq!(suggester.tag_corpus.get("rust").unwrap().usage_count, 2);
        assert_eq!(suggester.tag_corpus.get("rust").unwrap().success_rate, 1.0);
        
        // Update with rejection
        suggester.update_tag_success_rate("rust", false);
        assert_eq!(suggester.tag_corpus.get("rust").unwrap().usage_count, 3);
        assert!((suggester.tag_corpus.get("rust").unwrap().success_rate - 0.666).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_suggest_tags_for_summary() {
        let config = TagSuggesterConfig::default();
        let suggester = TagSuggester::new(config);
        
        let summary = ConversationSummary {
            id: uuid::Uuid::new_v4(),
            title: "How to debug Rust code?".to_string(),
            created_at: Utc::now(),
            last_active: Utc::now(),
            message_count: 5,
            status: ConversationStatus::default(),
            tags: vec!["rust".to_string()],
            workspace_id: None,
            has_branches: false,
            has_checkpoints: false,
            project_name: Some("my-rust-project".to_string()),
        };
        
        let suggestions = suggester.suggest_tags_for_summary(&summary).await.unwrap();
        
        // Should have suggestions from title and project
        assert!(!suggestions.is_empty());
        assert!(suggestions.iter().any(|s| s.tag == "question"));
    }
} 