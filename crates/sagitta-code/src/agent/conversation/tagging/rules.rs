use super::{TagSuggestion, TagSource};
use crate::agent::conversation::types::{Conversation, ConversationSummary, ProjectType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use regex::Regex;

/// Type of tag rule
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TagRuleType {
    /// Match keywords in conversation content
    KeywordMatch { keywords: Vec<String>, case_sensitive: bool },
    /// Match regex patterns
    RegexMatch { pattern: String },
    /// Match based on project type
    ProjectType { project_types: Vec<ProjectType> },
    /// Match based on message count
    MessageCount { min: Option<usize>, max: Option<usize> },
    /// Match based on conversation title
    TitlePattern { patterns: Vec<String> },
    /// Match based on file extensions mentioned
    FileExtension { extensions: Vec<String> },
    /// Match based on conversation duration
    Duration { min_hours: Option<f32>, max_hours: Option<f32> },
    /// Match based on branch count
    BranchCount { min: Option<usize>, max: Option<usize> },
    /// Custom rule with closure (not serializable)
    Custom,
}

/// A rule for generating tags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagRule {
    pub name: String,
    pub tag: String,
    pub confidence: f32,
    pub rule_type: TagRuleType,
    pub description: String,
    pub enabled: bool,
    pub priority: u32,
}

impl TagRule {
    pub fn new(name: String, tag: String, confidence: f32, rule_type: TagRuleType, description: String) -> Self {
        Self {
            name,
            tag,
            confidence,
            rule_type,
            description,
            enabled: true,
            priority: 100,
        }
    }

    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Check if this rule matches the given conversation
    pub fn matches(&self, conversation: &Conversation) -> bool {
        if !self.enabled {
            return false;
        }

        match &self.rule_type {
            TagRuleType::KeywordMatch { keywords, case_sensitive } => {
                let text = self.get_conversation_text(conversation);
                let text = if *case_sensitive { text } else { text.to_lowercase() };
                
                keywords.iter().any(|keyword| {
                    let keyword = if *case_sensitive { keyword.clone() } else { keyword.to_lowercase() };
                    text.contains(&keyword)
                })
            },
            TagRuleType::RegexMatch { pattern } => {
                if let Ok(regex) = Regex::new(pattern) {
                    let text = self.get_conversation_text(conversation);
                    regex.is_match(&text)
                } else {
                    false
                }
            },
            TagRuleType::ProjectType { project_types } => {
                if let Some(project_context) = &conversation.project_context {
                    project_types.contains(&project_context.project_type)
                } else {
                    false
                }
            },
            TagRuleType::MessageCount { min, max } => {
                let count = conversation.messages.len();
                let min_ok = min.map_or(true, |m| count >= m);
                let max_ok = max.map_or(true, |m| count <= m);
                min_ok && max_ok
            },
            TagRuleType::TitlePattern { patterns } => {
                let title = conversation.title.to_lowercase();
                patterns.iter().any(|pattern| title.contains(&pattern.to_lowercase()))
            },
            TagRuleType::FileExtension { extensions } => {
                let text = self.get_conversation_text(conversation).to_lowercase();
                extensions.iter().any(|ext| {
                    text.contains(&format!(".{}", ext.to_lowercase()))
                })
            },
            TagRuleType::Duration { min_hours, max_hours } => {
                let duration = conversation.last_active.signed_duration_since(conversation.created_at);
                let hours = duration.num_minutes() as f32 / 60.0;
                
                let min_ok = min_hours.map_or(true, |m| hours >= m);
                let max_ok = max_hours.map_or(true, |m| hours <= m);
                min_ok && max_ok
            },
            TagRuleType::BranchCount { min, max } => {
                let count = conversation.branches.len();
                let min_ok = min.map_or(true, |m| count >= m);
                let max_ok = max.map_or(true, |m| count <= m);
                min_ok && max_ok
            },
            TagRuleType::Custom => false, // Custom rules need special handling
        }
    }

    /// Check if this rule matches the given conversation summary
    pub fn matches_summary(&self, summary: &ConversationSummary) -> bool {
        if !self.enabled {
            return false;
        }

        match &self.rule_type {
            TagRuleType::MessageCount { min, max } => {
                let count = summary.message_count;
                let min_ok = min.map_or(true, |m| count >= m);
                let max_ok = max.map_or(true, |m| count <= m);
                min_ok && max_ok
            },
            TagRuleType::TitlePattern { patterns } => {
                let title = summary.title.to_lowercase();
                patterns.iter().any(|pattern| title.contains(&pattern.to_lowercase()))
            },
            TagRuleType::BranchCount { min, max } => {
                let has_branches = summary.has_branches;
                let count = if has_branches { 1 } else { 0 }; // Simplified for summary
                let min_ok = min.map_or(true, |m| count >= m);
                let max_ok = max.map_or(true, |m| count <= m);
                min_ok && max_ok
            },
            _ => false, // Other rules need full conversation data
        }
    }

    fn get_conversation_text(&self, conversation: &Conversation) -> String {
        let mut parts = Vec::new();
        parts.push(conversation.title.clone());
        
        for message in &conversation.messages {
            parts.push(message.content.clone());
        }
        
        parts.join(" ")
    }
}

/// Rule-based tag suggester
pub struct RuleBasedTagger {
    rules: Vec<TagRule>,
    custom_rules: HashMap<String, Box<dyn Fn(&Conversation) -> bool + Send + Sync>>,
}

impl RuleBasedTagger {
    /// Create a new rule-based tagger
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            custom_rules: HashMap::new(),
        }
    }

    /// Create with default rules
    pub fn with_default_rules() -> Self {
        let mut tagger = Self::new();
        tagger.add_default_rules();
        tagger
    }

    /// Add a rule
    pub fn add_rule(&mut self, rule: TagRule) {
        self.rules.push(rule);
        // Sort by priority (higher priority first)
        self.rules.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Add multiple rules
    pub fn add_rules(&mut self, rules: Vec<TagRule>) {
        for rule in rules {
            self.add_rule(rule);
        }
    }

    /// Add a custom rule with a closure
    pub fn add_custom_rule<F>(&mut self, name: String, tag: String, confidence: f32, description: String, rule_fn: F)
    where
        F: Fn(&Conversation) -> bool + Send + Sync + 'static,
    {
        let rule = TagRule::new(
            name.clone(),
            tag,
            confidence,
            TagRuleType::Custom,
            description,
        );
        self.add_rule(rule);
        self.custom_rules.insert(name, Box::new(rule_fn));
    }

    /// Remove a rule by name
    pub fn remove_rule(&mut self, name: &str) {
        self.rules.retain(|r| r.name != name);
        self.custom_rules.remove(name);
    }

    /// Enable/disable a rule
    pub fn set_rule_enabled(&mut self, name: &str, enabled: bool) {
        if let Some(rule) = self.rules.iter_mut().find(|r| r.name == name) {
            rule.enabled = enabled;
        }
    }

    /// Suggest tags for a conversation
    pub fn suggest_tags(&self, conversation: &Conversation) -> Vec<TagSuggestion> {
        let mut suggestions = Vec::new();

        for rule in &self.rules {
            if rule.matches(conversation) {
                let reasoning = format!("Rule '{}': {}", rule.name, rule.description);
                suggestions.push(TagSuggestion::new(
                    rule.tag.clone(),
                    rule.confidence,
                    reasoning,
                    TagSource::Rule { rule_name: rule.name.clone() },
                ));
            }
        }

        // Check custom rules
        for (name, rule_fn) in &self.custom_rules {
            if let Some(rule) = self.rules.iter().find(|r| r.name.as_str() == name && r.rule_type == TagRuleType::Custom) {
                if rule_fn(conversation) {
                    let reasoning = format!("Custom rule '{}': {}", rule.name, rule.description);
                    suggestions.push(TagSuggestion::new(
                        rule.tag.clone(),
                        rule.confidence,
                        reasoning,
                        TagSource::Rule { rule_name: rule.name.clone() },
                    ));
                }
            }
        }

        suggestions
    }

    /// Suggest tags for a conversation summary (limited rules only)
    pub fn suggest_tags_for_summary(&self, summary: &ConversationSummary) -> Vec<TagSuggestion> {
        let mut suggestions = Vec::new();

        for rule in &self.rules {
            if rule.matches_summary(summary) {
                let reasoning = format!("Rule '{}': {}", rule.name, rule.description);
                suggestions.push(TagSuggestion::new(
                    rule.tag.clone(),
                    rule.confidence,
                    reasoning,
                    TagSource::Rule { rule_name: rule.name.clone() },
                ));
            }
        }

        suggestions
    }

    /// Add default rules for common scenarios
    fn add_default_rules(&mut self) {
        // Programming language rules
        self.add_rule(TagRule::new(
            "rust_keywords".to_string(),
            "rust".to_string(),
            0.8,
            TagRuleType::KeywordMatch {
                keywords: vec!["cargo".to_string(), "rustc".to_string(), "trait".to_string(), "impl".to_string()],
                case_sensitive: false,
            },
            "Detects Rust programming language keywords".to_string(),
        ).with_priority(200));

        self.add_rule(TagRule::new(
            "python_keywords".to_string(),
            "python".to_string(),
            0.8,
            TagRuleType::KeywordMatch {
                keywords: vec!["def ".to_string(), "import ".to_string(), "python".to_string(), "pip".to_string()],
                case_sensitive: false,
            },
            "Detects Python programming language keywords".to_string(),
        ).with_priority(200));

        self.add_rule(TagRule::new(
            "javascript_keywords".to_string(),
            "javascript".to_string(),
            0.8,
            TagRuleType::KeywordMatch {
                keywords: vec!["function".to_string(), "const ".to_string(), "npm".to_string(), "node".to_string()],
                case_sensitive: false,
            },
            "Detects JavaScript programming language keywords".to_string(),
        ).with_priority(200));

        // Project type rules
        self.add_rule(TagRule::new(
            "rust_project".to_string(),
            "rust".to_string(),
            0.9,
            TagRuleType::ProjectType {
                project_types: vec![ProjectType::Rust],
            },
            "Conversation in a Rust project".to_string(),
        ).with_priority(300));

        self.add_rule(TagRule::new(
            "python_project".to_string(),
            "python".to_string(),
            0.9,
            TagRuleType::ProjectType {
                project_types: vec![ProjectType::Python],
            },
            "Conversation in a Python project".to_string(),
        ).with_priority(300));

        // Topic rules
        self.add_rule(TagRule::new(
            "error_debugging".to_string(),
            "debugging".to_string(),
            0.7,
            TagRuleType::KeywordMatch {
                keywords: vec!["error".to_string(), "bug".to_string(), "debug".to_string(), "fix".to_string()],
                case_sensitive: false,
            },
            "Conversation about debugging or fixing errors".to_string(),
        ).with_priority(150));

        self.add_rule(TagRule::new(
            "performance_optimization".to_string(),
            "performance".to_string(),
            0.7,
            TagRuleType::KeywordMatch {
                keywords: vec!["slow".to_string(), "optimize".to_string(), "performance".to_string(), "speed".to_string()],
                case_sensitive: false,
            },
            "Conversation about performance optimization".to_string(),
        ).with_priority(150));

        // Question patterns
        self.add_rule(TagRule::new(
            "question_title".to_string(),
            "question".to_string(),
            0.6,
            TagRuleType::TitlePattern {
                patterns: vec!["how".to_string(), "what".to_string(), "why".to_string(), "?".to_string()],
            },
            "Title contains question words or question mark".to_string(),
        ).with_priority(100));

        // File extension rules
        self.add_rule(TagRule::new(
            "rust_files".to_string(),
            "rust".to_string(),
            0.6,
            TagRuleType::FileExtension {
                extensions: vec!["rs".to_string(), "toml".to_string()],
            },
            "Mentions Rust file extensions".to_string(),
        ).with_priority(120));

        self.add_rule(TagRule::new(
            "python_files".to_string(),
            "python".to_string(),
            0.6,
            TagRuleType::FileExtension {
                extensions: vec!["py".to_string(), "pyx".to_string(), "pyi".to_string()],
            },
            "Mentions Python file extensions".to_string(),
        ).with_priority(120));

        // Conversation characteristics
        self.add_rule(TagRule::new(
            "long_conversation".to_string(),
            "long-conversation".to_string(),
            0.5,
            TagRuleType::MessageCount {
                min: Some(20),
                max: None,
            },
            "Conversation with many messages".to_string(),
        ).with_priority(50));

        self.add_rule(TagRule::new(
            "branched_conversation".to_string(),
            "branched".to_string(),
            0.6,
            TagRuleType::BranchCount {
                min: Some(1),
                max: None,
            },
            "Conversation with branches".to_string(),
        ).with_priority(80));

        // Help and support
        self.add_rule(TagRule::new(
            "help_request".to_string(),
            "help".to_string(),
            0.6,
            TagRuleType::KeywordMatch {
                keywords: vec!["help".to_string(), "stuck".to_string(), "assist".to_string(), "support".to_string()],
                case_sensitive: false,
            },
            "Request for help or assistance".to_string(),
        ).with_priority(110));
    }

    /// Get all rules
    pub fn get_rules(&self) -> &[TagRule] {
        &self.rules
    }

    /// Get rule by name
    pub fn get_rule(&self, name: &str) -> Option<&TagRule> {
        self.rules.iter().find(|r| r.name == name)
    }

    /// Get enabled rules count
    pub fn enabled_rules_count(&self) -> usize {
        self.rules.iter().filter(|r| r.enabled).count()
    }

    /// Clear all rules
    pub fn clear_rules(&mut self) {
        self.rules.clear();
        self.custom_rules.clear();
    }

    fn apply_keyword_rule(&self, rule: &TagRule, conversation: &Conversation) -> Option<TagSuggestion> {
        if let TagRuleType::KeywordMatch { keywords, case_sensitive } = &rule.rule_type {
            let mut all_text = conversation.title.clone();
            
            for message in &conversation.messages {
                all_text.push(' ');
                all_text.push_str(&message.content);
            }
            
            let search_text = if *case_sensitive {
                all_text
            } else {
                all_text.to_lowercase()
            };
            
            let search_keywords: Vec<String> = if *case_sensitive {
                keywords.clone()
            } else {
                keywords.iter().map(|k| k.to_lowercase()).collect()
            };
            
            let matches = search_keywords.iter()
                .filter(|keyword| search_text.contains(*keyword))
                .count();
            
            if matches > 0 {
                let confidence = (matches as f32 / keywords.len() as f32).min(1.0) * 0.7;
                let reasoning = format!("Matched {} of {} keywords: {}", 
                    matches, keywords.len(), keywords.join(", "));
                
                Some(TagSuggestion::new(
                    rule.tag.clone(),
                    confidence,
                    reasoning,
                    TagSource::Rule { rule_name: rule.name.clone() },
                ))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Remove a custom rule by name
    pub fn remove_custom_rule(&mut self, name: &str) -> bool {
        if let Some(rule) = self.rules.iter().find(|r| r.name.as_str() == name && r.rule_type == TagRuleType::Custom) {
            let rule_name = rule.name.clone();
            self.rules.retain(|r| r.name != rule_name);
            true
        } else {
            false
        }
    }
}

impl Default for RuleBasedTagger {
    fn default() -> Self {
        Self::with_default_rules()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::conversation::types::{Conversation, ProjectContext};
    use crate::agent::message::types::AgentMessage;
    use crate::agent::state::types::ConversationStatus;

    fn create_test_conversation(title: &str, content: &str) -> Conversation {
        let mut conversation = Conversation::new(title.to_string(), None);
        
        let message = AgentMessage {
            id: uuid::Uuid::new_v4(),
            content: content.to_string(),
            role: crate::llm::client::Role::User,
            is_streaming: false,
            timestamp: chrono::Utc::now(),
            metadata: std::collections::HashMap::new(),
            tool_calls: vec![],
        };
        conversation.add_message(message);
        
        conversation
    }

    fn create_rust_conversation() -> Conversation {
        let mut conversation = create_test_conversation(
            "Rust cargo build error",
            "I'm getting a cargo build error with my Rust project. The trait implementation is failing."
        );
        
        conversation.project_context = Some(ProjectContext {
            name: "test-project".to_string(),
            project_type: ProjectType::Rust,
            root_path: None,
            description: None,
            repositories: vec![],
            settings: std::collections::HashMap::new(),
        });
        
        conversation
    }

    #[test]
    fn test_keyword_rule_matching() {
        let rule = TagRule::new(
            "rust_test".to_string(),
            "rust".to_string(),
            0.8,
            TagRuleType::KeywordMatch {
                keywords: vec!["cargo".to_string(), "trait".to_string()],
                case_sensitive: false,
            },
            "Test rule".to_string(),
        );

        let conversation = create_rust_conversation();
        assert!(rule.matches(&conversation));

        let non_rust_conversation = create_test_conversation("Python help", "I need help with Python");
        assert!(!rule.matches(&non_rust_conversation));
    }

    #[test]
    fn test_project_type_rule_matching() {
        let rule = TagRule::new(
            "rust_project_test".to_string(),
            "rust".to_string(),
            0.9,
            TagRuleType::ProjectType {
                project_types: vec![ProjectType::Rust],
            },
            "Test rule".to_string(),
        );

        let conversation = create_rust_conversation();
        assert!(rule.matches(&conversation));

        let conversation_no_project = create_test_conversation("General question", "How do I code?");
        assert!(!rule.matches(&conversation_no_project));
    }

    #[test]
    fn test_message_count_rule_matching() {
        let rule = TagRule::new(
            "long_conversation_test".to_string(),
            "long".to_string(),
            0.5,
            TagRuleType::MessageCount {
                min: Some(2),
                max: None,
            },
            "Test rule".to_string(),
        );

        let mut conversation = create_test_conversation("Test", "Message 1");
        assert!(!rule.matches(&conversation)); // Only 1 message

        // Add another message
        let message2 = AgentMessage {
            id: uuid::Uuid::new_v4(),
            content: "Message 2".to_string(),
            role: crate::llm::client::Role::Assistant,
            is_streaming: false,
            timestamp: chrono::Utc::now(),
            metadata: std::collections::HashMap::new(),
            tool_calls: vec![],
        };
        conversation.add_message(message2);
        assert!(rule.matches(&conversation)); // Now 2 messages
    }

    #[test]
    fn test_title_pattern_rule_matching() {
        let rule = TagRule::new(
            "question_test".to_string(),
            "question".to_string(),
            0.6,
            TagRuleType::TitlePattern {
                patterns: vec!["how".to_string(), "?".to_string()],
            },
            "Test rule".to_string(),
        );

        let question_conversation = create_test_conversation("How do I fix this?", "Help needed");
        assert!(rule.matches(&question_conversation));

        let statement_conversation = create_test_conversation("I fixed the bug", "It works now");
        assert!(!rule.matches(&statement_conversation));
    }

    #[test]
    fn test_rule_based_tagger() {
        let mut tagger = RuleBasedTagger::new();
        
        tagger.add_rule(TagRule::new(
            "rust_test".to_string(),
            "rust".to_string(),
            0.8,
            TagRuleType::KeywordMatch {
                keywords: vec!["cargo".to_string()],
                case_sensitive: false,
            },
            "Test rule".to_string(),
        ));

        let conversation = create_rust_conversation();
        let suggestions = tagger.suggest_tags(&conversation);
        
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].tag, "rust");
        assert_eq!(suggestions[0].confidence, 0.8);
    }

    #[test]
    fn test_default_rules() {
        let tagger = RuleBasedTagger::with_default_rules();
        assert!(tagger.enabled_rules_count() > 0);

        let conversation = create_rust_conversation();
        let suggestions = tagger.suggest_tags(&conversation);
        
        // Should detect multiple tags
        assert!(!suggestions.is_empty());
        assert!(suggestions.iter().any(|s| s.tag == "rust"));
        assert!(suggestions.iter().any(|s| s.tag == "debugging"));
    }

    #[test]
    fn test_rule_priority_sorting() {
        let mut tagger = RuleBasedTagger::new();
        
        tagger.add_rule(TagRule::new(
            "low_priority".to_string(),
            "low".to_string(),
            0.5,
            TagRuleType::KeywordMatch {
                keywords: vec!["test".to_string()],
                case_sensitive: false,
            },
            "Low priority rule".to_string(),
        ).with_priority(50));

        tagger.add_rule(TagRule::new(
            "high_priority".to_string(),
            "high".to_string(),
            0.8,
            TagRuleType::KeywordMatch {
                keywords: vec!["test".to_string()],
                case_sensitive: false,
            },
            "High priority rule".to_string(),
        ).with_priority(200));

        // Rules should be sorted by priority (high first)
        assert_eq!(tagger.rules[0].name, "high_priority");
        assert_eq!(tagger.rules[1].name, "low_priority");
    }

    #[test]
    fn test_rule_enable_disable() {
        let mut tagger = RuleBasedTagger::new();
        
        tagger.add_rule(TagRule::new(
            "test_rule".to_string(),
            "test".to_string(),
            0.8,
            TagRuleType::KeywordMatch {
                keywords: vec!["test".to_string()],
                case_sensitive: false,
            },
            "Test rule".to_string(),
        ));

        let conversation = create_test_conversation("Test title", "Test content");
        
        // Rule should match initially
        let suggestions = tagger.suggest_tags(&conversation);
        assert_eq!(suggestions.len(), 1);

        // Disable rule
        tagger.set_rule_enabled("test_rule", false);
        let suggestions = tagger.suggest_tags(&conversation);
        assert_eq!(suggestions.len(), 0);

        // Re-enable rule
        tagger.set_rule_enabled("test_rule", true);
        let suggestions = tagger.suggest_tags(&conversation);
        assert_eq!(suggestions.len(), 1);
    }

    #[test]
    fn test_custom_rule() {
        let mut tagger = RuleBasedTagger::new();
        
        tagger.add_custom_rule(
            "custom_test".to_string(),
            "custom".to_string(),
            0.7,
            "Custom rule for testing".to_string(),
            |conversation| conversation.title.to_lowercase().contains("custom"),
        );

        let custom_conversation = create_test_conversation("Custom title", "Content");
        let suggestions = tagger.suggest_tags(&custom_conversation);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].tag, "custom");

        let normal_conversation = create_test_conversation("Normal title", "Content");
        let suggestions = tagger.suggest_tags(&normal_conversation);
        assert_eq!(suggestions.len(), 0);
    }

    #[test]
    fn test_summary_matching() {
        let tagger = RuleBasedTagger::with_default_rules();
        
        let summary = ConversationSummary {
            id: uuid::Uuid::new_v4(),
            title: "How to fix this error?".to_string(),
            created_at: chrono::Utc::now(),
            last_active: chrono::Utc::now(),
            message_count: 25,
            status: ConversationStatus::default(),
            tags: vec![],
            workspace_id: None,
            has_branches: true,
            has_checkpoints: false,
            project_name: None,
        };

        let suggestions = tagger.suggest_tags_for_summary(&summary);
        
        // Should detect question and long conversation
        assert!(suggestions.iter().any(|s| s.tag == "question"));
        assert!(suggestions.iter().any(|s| s.tag == "long-conversation"));
        assert!(suggestions.iter().any(|s| s.tag == "branched"));
    }
} 