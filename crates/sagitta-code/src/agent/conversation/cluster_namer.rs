use anyhow::Result;
use std::sync::Arc;
use std::collections::HashMap;

use crate::agent::conversation::clustering::ConversationCluster;
use crate::agent::conversation::types::{ConversationSummary, ProjectType};
use crate::llm::client::{LlmClient, Role, Message, MessagePart};
use sagitta_embed::EmbeddingPool;

/// Configuration for cluster naming
#[derive(Debug, Clone)]
pub struct ClusterNamerConfig {
    /// Maximum length for generated cluster names
    pub max_name_length: usize,
    
    /// Whether to use embeddings for context analysis
    pub use_embeddings: bool,
    
    /// Fallback name prefix when generation fails
    pub fallback_prefix: String,
    
    /// Whether to include project type in naming
    pub include_project_type: bool,
    
    /// Whether to include time context in naming
    pub include_time_context: bool,
}

impl Default for ClusterNamerConfig {
    fn default() -> Self {
        Self {
            max_name_length: 40,
            use_embeddings: true,
            fallback_prefix: "Cluster".to_string(),
            include_project_type: true,
            include_time_context: false,
        }
    }
}

/// Generator for cluster names using LLM and rule-based approaches
pub struct ClusterNamer {
    config: ClusterNamerConfig,
    llm_client: Option<Arc<dyn LlmClient>>,
    embedding_pool: Option<Arc<EmbeddingPool>>,
    should_fail: bool, // For testing failure scenarios
}

impl ClusterNamer {
    /// Create a new cluster namer
    pub fn new(config: ClusterNamerConfig) -> Self {
        Self {
            config,
            llm_client: None,
            embedding_pool: None,
            should_fail: false,
        }
    }
    
    /// Set the LLM client for name generation
    pub fn with_llm_client(mut self, client: Arc<dyn LlmClient>) -> Self {
        self.llm_client = Some(client);
        self
    }
    
    /// Set the embedding pool for context analysis
    pub fn with_embedding_pool(mut self, pool: Arc<EmbeddingPool>) -> Self {
        self.embedding_pool = Some(pool);
        self
    }
    
    /// Create a failing cluster namer for testing
    pub fn failing() -> Self {
        Self {
            config: ClusterNamerConfig::default(),
            llm_client: None,
            embedding_pool: None,
            should_fail: true,
        }
    }
    
    /// Generate a descriptive name for a cluster
    pub async fn generate_cluster_name(
        &self,
        cluster: &ConversationCluster,
        conversations: &[ConversationSummary],
    ) -> Result<String> {
        // Handle test failure scenario
        if self.should_fail {
            return Ok(self.generate_fallback_name(cluster, conversations));
        }
        
        // Try LLM generation first
        if let Some(ref llm_client) = self.llm_client {
            match self.generate_with_llm(cluster, conversations, llm_client).await {
                Ok(name) => {
                    let truncated = self.truncate_name(name);
                    if !truncated.is_empty() && truncated.len() > 3 {
                        return Ok(truncated);
                    }
                }
                Err(e) => {
                    eprintln!("LLM cluster naming failed: {e}");
                }
            }
        }
        
        // Fall back to rule-based generation
        self.generate_rule_based_name(cluster, conversations)
    }
    
    /// Generate cluster name using LLM
    async fn generate_with_llm(
        &self,
        cluster: &ConversationCluster,
        conversations: &[ConversationSummary],
        llm_client: &Arc<dyn LlmClient>,
    ) -> Result<String> {
        // Get conversation summaries for the cluster
        let cluster_conversations: Vec<&ConversationSummary> = conversations
            .iter()
            .filter(|conv| cluster.conversation_ids.contains(&conv.id))
            .collect();
        
        if cluster_conversations.is_empty() {
            return Err(anyhow::anyhow!("No conversations found for cluster"));
        }
        
        // Build context for LLM
        let context = self.build_cluster_context(cluster, &cluster_conversations);
        
        let prompt = format!(
            "Generate a concise, descriptive name for a cluster of related conversations. The name should be under {} characters and capture the main theme or topic. Do not include quotes or extra formatting.\n\nCluster Information:\n{}\n\nCluster Name:",
            self.config.max_name_length,
            context
        );
        
        // Convert to LlmClient message format
        let llm_messages = vec![
            Message {
                id: uuid::Uuid::new_v4(),
                role: Role::User,
                parts: vec![MessagePart::Text { text: prompt }],
                metadata: HashMap::new(),
            }
        ];
        
        let response = llm_client.generate(&llm_messages, &[]).await
            .map_err(|e| anyhow::anyhow!("LLM generation failed: {}", e))?;
        
        // Extract text from response
        let name = response.message.parts.iter()
            .find_map(|part| match part {
                MessagePart::Text { text } => Some(text.clone()),
                _ => None,
            })
            .unwrap_or_default()
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();
        
        Ok(name)
    }
    
    /// Build context string for LLM prompt
    fn build_cluster_context(
        &self,
        cluster: &ConversationCluster,
        conversations: &[&ConversationSummary],
    ) -> String {
        let mut context_parts = Vec::new();
        
        // Add conversation titles
        context_parts.push("Conversation Titles:".to_string());
        for conv in conversations.iter().take(5) { // Limit to avoid token limits
            context_parts.push(format!("- {}", conv.title));
        }
        
        // Add common tags
        if !cluster.common_tags.is_empty() {
            context_parts.push(format!("Common Tags: {}", cluster.common_tags.join(", ")));
        }
        
        // Add project type if configured
        if self.config.include_project_type {
            if let Some(ref project_type) = cluster.dominant_project_type {
                context_parts.push(format!("Project Type: {project_type:?}"));
            }
        }
        
        // Add cohesion score
        context_parts.push(format!("Cohesion Score: {:.2}", cluster.cohesion_score));
        
        // Add time context if configured
        if self.config.include_time_context {
            let duration = cluster.time_range.1.signed_duration_since(cluster.time_range.0);
            if duration.num_days() > 0 {
                context_parts.push(format!("Time Span: {} days", duration.num_days()));
            } else if duration.num_hours() > 0 {
                context_parts.push(format!("Time Span: {} hours", duration.num_hours()));
            }
        }
        
        context_parts.join("\n")
    }
    
    /// Generate cluster name using rule-based approach
    fn generate_rule_based_name(
        &self,
        cluster: &ConversationCluster,
        conversations: &[ConversationSummary],
    ) -> Result<String> {
        // Get conversations in this cluster
        let cluster_conversations: Vec<&ConversationSummary> = conversations
            .iter()
            .filter(|conv| cluster.conversation_ids.contains(&conv.id))
            .collect();
        
        if cluster_conversations.is_empty() {
            return Ok(self.generate_fallback_name(cluster, conversations));
        }
        
        // Try common tags first
        if !cluster.common_tags.is_empty() {
            let name = self.generate_name_from_tags(&cluster.common_tags, cluster_conversations.len());
            if !name.is_empty() {
                return Ok(self.truncate_name(name));
            }
        }
        
        // Try project type
        if let Some(ref project_type) = cluster.dominant_project_type {
            let name = self.generate_name_from_project_type(project_type, &cluster.common_tags);
            if !name.is_empty() {
                return Ok(self.truncate_name(name));
            }
        }
        
        // Try common words from titles
        let titles: Vec<&str> = cluster_conversations
            .iter()
            .map(|conv| conv.title.as_str())
            .collect();
        
        let common_words = self.find_common_words(&titles);
        if !common_words.is_empty() {
            let name = format!("{} Discussions", common_words.join(" "));
            return Ok(self.truncate_name(name));
        }
        
        // Try thematic analysis
        let thematic_name = self.generate_thematic_name(&cluster_conversations);
        if !thematic_name.is_empty() {
            return Ok(self.truncate_name(thematic_name));
        }
        
        // Final fallback
        Ok(self.generate_fallback_name(cluster, conversations))
    }
    
    /// Generate name from common tags
    fn generate_name_from_tags(&self, tags: &[String], conversation_count: usize) -> String {
        if tags.is_empty() {
            return String::new();
        }
        
        // Prioritize meaningful tags
        let meaningful_tags: Vec<&String> = tags
            .iter()
            .filter(|tag| tag.len() > 2 && !tag.chars().all(|c| c.is_numeric()))
            .collect();
        
        if meaningful_tags.is_empty() {
            return String::new();
        }
        
        // Use up to 2 most relevant tags
        let selected_tags: Vec<String> = meaningful_tags
            .iter()
            .take(2)
            .map(|tag| self.capitalize_first_letter(tag))
            .collect();
        
        if conversation_count == 1 {
            format!("{} Discussion", selected_tags.join(" "))
        } else {
            format!("{} Conversations", selected_tags.join(" "))
        }
    }
    
    /// Generate name from project type
    fn generate_name_from_project_type(
        &self,
        project_type: &ProjectType,
        tags: &[String],
    ) -> String {
        let base_name = match project_type {
            ProjectType::Rust => "Rust Development",
            ProjectType::Python => "Python Development", 
            ProjectType::JavaScript => "JavaScript Development",
            ProjectType::TypeScript => "TypeScript Development",
            ProjectType::Go => "Go Development",
            ProjectType::Ruby => "Ruby Development",
            ProjectType::Markdown => "Documentation",
            ProjectType::Yaml => "Configuration",
            ProjectType::Html => "Web Development",
            ProjectType::Unknown => "Development",
        };
        
        // Add context from tags if relevant
        if let Some(context_tag) = tags.iter().find(|tag| {
            let tag_lower = tag.to_lowercase();
            tag_lower.contains("error") || tag_lower.contains("debug") || 
            tag_lower.contains("help") || tag_lower.contains("question")
        }) {
            format!("{} {}", base_name, self.capitalize_first_letter(context_tag))
        } else {
            base_name.to_string()
        }
    }
    
    /// Generate thematic name based on conversation content analysis
    fn generate_thematic_name(&self, conversations: &[&ConversationSummary]) -> String {
        let mut theme_scores: HashMap<String, usize> = HashMap::new();
        
        // Analyze titles for themes
        for conv in conversations {
            let title_lower = conv.title.to_lowercase();
            
            // Machine Learning and AI themes
            if title_lower.contains("machine learning") || title_lower.contains("ml") ||
               title_lower.contains("neural network") || title_lower.contains("neural") ||
               title_lower.contains("model training") || title_lower.contains("training") ||
               title_lower.contains("deep learning") || title_lower.contains("ai") ||
               title_lower.contains("artificial intelligence") || title_lower.contains("algorithm") {
                *theme_scores.entry("Machine Learning".to_string()).or_insert(0) += 3;
            }
            
            // Programming themes
            if title_lower.contains("error") || title_lower.contains("debug") || title_lower.contains("fix") {
                *theme_scores.entry("Error Resolution".to_string()).or_insert(0) += 2;
            }
            if title_lower.contains("how") && title_lower.contains("?") {
                *theme_scores.entry("How-To Questions".to_string()).or_insert(0) += 2;
            }
            if title_lower.contains("api") || title_lower.contains("endpoint") {
                *theme_scores.entry("API Development".to_string()).or_insert(0) += 2;
            }
            if title_lower.contains("database") || title_lower.contains("sql") {
                *theme_scores.entry("Database Queries".to_string()).or_insert(0) += 2;
            }
            if title_lower.contains("test") || title_lower.contains("testing") {
                *theme_scores.entry("Testing & QA".to_string()).or_insert(0) += 2;
            }
            if title_lower.contains("deploy") || title_lower.contains("deployment") {
                *theme_scores.entry("Deployment".to_string()).or_insert(0) += 2;
            }
            if title_lower.contains("performance") || title_lower.contains("optimization") {
                *theme_scores.entry("Performance".to_string()).or_insert(0) += 2;
            }
            if title_lower.contains("security") || title_lower.contains("auth") {
                *theme_scores.entry("Security".to_string()).or_insert(0) += 2;
            }
            
            // General themes
            if title_lower.contains("help") || title_lower.contains("assist") {
                *theme_scores.entry("Help & Support".to_string()).or_insert(0) += 1;
            }
            if title_lower.contains("review") || title_lower.contains("feedback") {
                *theme_scores.entry("Code Review".to_string()).or_insert(0) += 1;
            }
            if title_lower.contains("learn") || title_lower.contains("tutorial") {
                *theme_scores.entry("Learning".to_string()).or_insert(0) += 1;
            }
        }
        
        // Find the highest scoring theme
        if let Some((theme, _)) = theme_scores.iter().max_by_key(|(_, score)| *score) {
            theme.clone()
        } else {
            String::new()
        }
    }
    
    /// Find common words across conversation titles
    fn find_common_words(&self, titles: &[&str]) -> Vec<String> {
        let mut word_counts: HashMap<String, usize> = HashMap::new();
        
        for title in titles {
            let words: std::collections::HashSet<String> = title
                .to_lowercase()
                .split_whitespace()
                .filter(|word| word.len() > 3) // Only consider longer words
                .filter(|word| !self.is_stop_word(word)) // Filter out stop words
                .map(|word| word.to_string())
                .collect();
            
            for word in words {
                *word_counts.entry(word).or_insert(0) += 1;
            }
        }
        
        let mut common_words: Vec<(String, usize)> = word_counts
            .into_iter()
            .filter(|(_, count)| *count >= titles.len().div_ceil(2)) // At least half the titles
            .collect();
        
        // Sort by count (descending) then alphabetically for deterministic behavior
        common_words.sort_by(|a, b| {
            b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0))
        });
        
        common_words
            .into_iter()
            .map(|(word, _)| self.capitalize_first_letter(&word))
            .take(2) // Limit to 2 words
            .collect()
    }
    
    /// Check if a word is a stop word
    fn is_stop_word(&self, word: &str) -> bool {
        matches!(word, 
            "the" | "and" | "or" | "but" | "in" | "on" | "at" | "to" | "for" | 
            "of" | "with" | "by" | "from" | "up" | "about" | "into" | "through" |
            "during" | "before" | "after" | "above" | "below" | "between" |
            "this" | "that" | "these" | "those" | "a" | "an" | "is" | "are" |
            "was" | "were" | "be" | "been" | "being" | "have" | "has" | "had" |
            "do" | "does" | "did" | "will" | "would" | "could" | "should" |
            "may" | "might" | "must" | "can" | "help" | "how" | "what" | "when" |
            "where" | "why" | "which" | "who" | "whom" | "whose"
        )
    }
    
    /// Generate a fallback name when other methods fail
    fn generate_fallback_name(
        &self,
        cluster: &ConversationCluster,
        conversations: &[ConversationSummary],
    ) -> String {
        let cluster_conversations: Vec<&ConversationSummary> = conversations
            .iter()
            .filter(|conv| cluster.conversation_ids.contains(&conv.id))
            .collect();
        
        let count = cluster_conversations.len();
        
        // Try to use the first conversation's theme
        if let Some(first_conv) = cluster_conversations.first() {
            let title_words: Vec<&str> = first_conv.title
                .split_whitespace()
                .filter(|word| word.len() > 3 && !self.is_stop_word(&word.to_lowercase()))
                .take(2)
                .collect();
            
            if !title_words.is_empty() {
                return format!("{} Discussions", title_words.join(" "));
            }
        }
        
        // Use project type if available
        if let Some(ref project_type) = cluster.dominant_project_type {
            return format!("{project_type:?} Cluster");
        }
        
        // Final fallback with count
        if count == 1 {
            format!("{} Discussion", self.config.fallback_prefix)
        } else {
            format!("{} of {} Conversations", self.config.fallback_prefix, count)
        }
    }
    
    /// Capitalize the first letter of a string
    fn capitalize_first_letter(&self, s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }
    
    /// Truncate name to maximum length
    fn truncate_name(&self, name: String) -> String {
        if name.len() <= self.config.max_name_length {
            name
        } else {
            let truncated = name.chars().take(self.config.max_name_length - 3).collect::<String>();
            format!("{truncated}...")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    use chrono::Utc;
    use crate::ConversationStatus;
    
    fn create_test_conversation(title: &str, tags: Vec<String>) -> ConversationSummary {
        ConversationSummary {
            id: Uuid::new_v4(),
            title: title.to_string(),
            created_at: Utc::now() - chrono::Duration::hours(1),
            last_active: Utc::now(),
            message_count: 5,
            status: ConversationStatus::Active,
            tags,
            workspace_id: None,
            has_branches: false,
            has_checkpoints: false,
            project_name: None,
        }
    }
    
    #[tokio::test]
    async fn test_rule_based_naming_from_tags() {
        let namer = ClusterNamer::new(ClusterNamerConfig::default());
        
        let conversations = vec![
            create_test_conversation("Rust Error Help", vec!["rust".to_string(), "error".to_string()]),
            create_test_conversation("Rust Panic Issues", vec!["rust".to_string(), "panic".to_string()]),
        ];
        
        let cluster = ConversationCluster {
            id: Uuid::new_v4(),
            title: "Test".to_string(),
            conversation_ids: conversations.iter().map(|c| c.id).collect(),
            centroid: vec![0.1, 0.2, 0.3],
            cohesion_score: 0.85,
            common_tags: vec!["rust".to_string(), "error".to_string()],
            dominant_project_type: Some(ProjectType::Rust),
            time_range: (Utc::now() - chrono::Duration::days(1), Utc::now()),
        };
        
        let name = namer.generate_cluster_name(&cluster, &conversations).await.unwrap();
        
        assert!(!name.is_empty());
        assert!(name.len() > 3);
        assert!(name.to_lowercase().contains("rust") || name.to_lowercase().contains("error"));
    }
    
    #[tokio::test]
    async fn test_rule_based_naming_from_project_type() {
        let namer = ClusterNamer::new(ClusterNamerConfig::default());
        
        let conversations = vec![
            create_test_conversation("Python Data Analysis", vec!["python".to_string()]),
            create_test_conversation("Python Pandas Help", vec!["python".to_string()]),
        ];
        
        let cluster = ConversationCluster {
            id: Uuid::new_v4(),
            title: "Test".to_string(),
            conversation_ids: conversations.iter().map(|c| c.id).collect(),
            centroid: vec![0.4, 0.5, 0.6],
            cohesion_score: 0.78,
            common_tags: vec![],
            dominant_project_type: Some(ProjectType::Python),
            time_range: (Utc::now() - chrono::Duration::days(1), Utc::now()),
        };
        
        let name = namer.generate_cluster_name(&cluster, &conversations).await.unwrap();
        
        assert!(!name.is_empty());
        assert!(name.len() > 3);
        assert!(name.to_lowercase().contains("python") || name.to_lowercase().contains("development"));
    }
    
    #[tokio::test]
    async fn test_thematic_naming() {
        let namer = ClusterNamer::new(ClusterNamerConfig::default());
        
        let conversations = vec![
            create_test_conversation("How to fix database error?", vec![]),
            create_test_conversation("Debug SQL query issue", vec![]),
            create_test_conversation("Error in database connection", vec![]),
        ];
        
        let cluster = ConversationCluster {
            id: Uuid::new_v4(),
            title: "Test".to_string(),
            conversation_ids: conversations.iter().map(|c| c.id).collect(),
            centroid: vec![0.7, 0.8, 0.9],
            cohesion_score: 0.82,
            common_tags: vec![],
            dominant_project_type: None,
            time_range: (Utc::now() - chrono::Duration::days(1), Utc::now()),
        };
        
        let name = namer.generate_cluster_name(&cluster, &conversations).await.unwrap();
        
        assert!(!name.is_empty());
        assert!(name.len() > 3);
        // Should identify error/database theme
        assert!(
            name.to_lowercase().contains("error") ||
            name.to_lowercase().contains("database") ||
            name.to_lowercase().contains("resolution")
        );
    }
    
    #[tokio::test]
    async fn test_fallback_naming() {
        let namer = ClusterNamer::failing();
        
        let conversations = vec![
            create_test_conversation("Random conversation", vec![]),
        ];
        
        let cluster = ConversationCluster {
            id: Uuid::new_v4(),
            title: "Test".to_string(),
            conversation_ids: vec![conversations[0].id],
            centroid: vec![0.1, 0.1, 0.1],
            cohesion_score: 1.0,
            common_tags: vec![],
            dominant_project_type: None,
            time_range: (Utc::now() - chrono::Duration::hours(1), Utc::now()),
        };
        
        let name = namer.generate_cluster_name(&cluster, &conversations).await.unwrap();
        
        assert!(!name.is_empty());
        assert!(name.len() > 3);
        assert!(name.starts_with("Cluster") || name.contains("Discussion"));
    }
    
    #[test]
    fn test_name_truncation() {
        let namer = ClusterNamer::new(ClusterNamerConfig {
            max_name_length: 20,
            ..Default::default()
        });
        
        let long_name = "This is a very long cluster name that should be truncated".to_string();
        let truncated = namer.truncate_name(long_name);
        
        assert!(truncated.len() <= 20);
        assert!(truncated.ends_with("..."));
    }
    
    #[test]
    fn test_common_words_extraction() {
        let namer = ClusterNamer::new(ClusterNamerConfig::default());
        
        let titles = vec![
            "Rust error handling help",
            "Rust error recovery methods", 
            "Rust error propagation guide",
        ];
        
        let common_words = namer.find_common_words(&titles);
        
        assert!(!common_words.is_empty());
        assert!(common_words.iter().any(|word| word.to_lowercase().contains("rust") || word.to_lowercase().contains("error")));
    }
    
    #[tokio::test]
    async fn test_machine_learning_thematic_naming() {
        let namer = ClusterNamer::new(ClusterNamerConfig::default());
        
        let conversations = vec![
            create_test_conversation("Machine Learning Model Training", vec!["ml".to_string(), "training".to_string()]),
            create_test_conversation("Neural Network Architecture", vec!["ml".to_string(), "neural".to_string()]),
        ];
        
        let cluster = ConversationCluster {
            id: Uuid::new_v4(),
            title: "Test".to_string(),
            conversation_ids: conversations.iter().map(|c| c.id).collect(),
            centroid: vec![0.6, 0.7, 0.8],
            cohesion_score: 0.88,
            common_tags: vec!["ml".to_string()],
            dominant_project_type: None,
            time_range: (Utc::now() - chrono::Duration::days(1), Utc::now()),
        };
        
        let name = namer.generate_cluster_name(&cluster, &conversations).await.unwrap();
        
        assert!(!name.is_empty());
        assert!(name.len() > 3);
        // Should identify machine learning theme
        assert!(
            name.to_lowercase().contains("machine learning") ||
            name.to_lowercase().contains("ml") ||
            name.to_lowercase().contains("learning"),
            "Should identify ML theme, got: '{name}'"
        );
    }
} 