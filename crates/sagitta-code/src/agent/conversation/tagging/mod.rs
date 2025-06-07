pub mod suggester;
pub mod rules;
pub mod ui;

// Core types
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// A tag suggestion with confidence and reasoning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagSuggestion {
    pub tag: String,
    pub confidence: f32,
    pub reasoning: String,
    pub source: TagSource,
    pub timestamp: DateTime<Utc>,
}

impl TagSuggestion {
    pub fn new(tag: String, confidence: f32, reasoning: String, source: TagSource) -> Self {
        Self {
            tag,
            confidence,
            reasoning,
            source,
            timestamp: Utc::now(),
        }
    }

    pub fn confidence_level(&self) -> SuggestionConfidence {
        SuggestionConfidence::from_score(self.confidence)
    }

    pub fn is_high_confidence(&self) -> bool {
        self.confidence >= 0.6
    }
}

/// Source of a tag suggestion
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TagSource {
    /// Generated from embedding similarity
    Embedding { similarity_score: f32 },
    /// Generated from rule-based matching
    Rule { rule_name: String },
    /// Manually added by user
    Manual,
    /// Generated from content analysis
    Content { keywords: Vec<String> },
}

/// Confidence level for tag suggestions
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SuggestionConfidence {
    Low,
    Medium,
    High,
    VeryHigh,
}

impl SuggestionConfidence {
    pub fn from_score(score: f32) -> Self {
        if score >= 0.8 {
            Self::VeryHigh
        } else if score >= 0.6 {
            Self::High
        } else if score >= 0.4 {
            Self::Medium
        } else {
            Self::Low
        }
    }

    pub fn to_score(self) -> f32 {
        match self {
            Self::Low => 0.3,
            Self::Medium => 0.5,
            Self::High => 0.7,
            Self::VeryHigh => 0.9,
        }
    }
}

/// Action taken on a tag suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TagAction {
    /// Accept the suggestion
    Accept,
    /// Reject the suggestion
    Reject,
    /// Modify the suggestion
    Modify { new_tag: String },
    /// Pending user decision
    Pending,
}

/// Tag suggestion with user action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagSuggestionWithAction {
    pub suggestion: TagSuggestion,
    pub action: TagAction,
    pub action_timestamp: Option<DateTime<Utc>>,
}

impl TagSuggestionWithAction {
    pub fn new(suggestion: TagSuggestion) -> Self {
        Self {
            suggestion,
            action: TagAction::Pending,
            action_timestamp: None,
        }
    }

    pub fn accept(mut self) -> Self {
        self.action = TagAction::Accept;
        self.action_timestamp = Some(Utc::now());
        self
    }

    pub fn reject(mut self) -> Self {
        self.action = TagAction::Reject;
        self.action_timestamp = Some(Utc::now());
        self
    }

    pub fn modify(mut self, new_tag: String) -> Self {
        self.action = TagAction::Modify { new_tag };
        self.action_timestamp = Some(Utc::now());
        self
    }
}

// Re-exports
pub use suggester::{TagSuggester, TagSuggesterConfig, TagCorpusEntry};
pub use rules::{RuleBasedTagger, TagRule, TagRuleType};
pub use ui::{TagManagementUI, TagUIState, TagUIAction};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_suggestion_confidence_from_score() {
        assert_eq!(SuggestionConfidence::from_score(0.9), SuggestionConfidence::VeryHigh);
        assert_eq!(SuggestionConfidence::from_score(0.7), SuggestionConfidence::High);
        assert_eq!(SuggestionConfidence::from_score(0.5), SuggestionConfidence::Medium);
        assert_eq!(SuggestionConfidence::from_score(0.2), SuggestionConfidence::Low);
    }

    #[test]
    fn test_suggestion_confidence_to_score() {
        assert_eq!(SuggestionConfidence::VeryHigh.to_score(), 0.9);
        assert_eq!(SuggestionConfidence::High.to_score(), 0.7);
        assert_eq!(SuggestionConfidence::Medium.to_score(), 0.5);
        assert_eq!(SuggestionConfidence::Low.to_score(), 0.3);
    }

    #[test]
    fn test_tag_suggestion_creation() {
        let suggestion = TagSuggestion::new(
            "rust".to_string(),
            0.8,
            "High similarity to Rust-related conversations".to_string(),
            TagSource::Embedding { similarity_score: 0.8 },
        );

        assert_eq!(suggestion.tag, "rust");
        assert_eq!(suggestion.confidence, 0.8);
        assert_eq!(SuggestionConfidence::from_score(suggestion.confidence), SuggestionConfidence::VeryHigh);
        assert!(suggestion.confidence >= 0.6);
    }

    #[test]
    fn test_tag_source_variants() {
        let embedding_source = TagSource::Embedding { similarity_score: 0.7 };
        let rule_source = TagSource::Rule { rule_name: "project_type".to_string() };
        let manual_source = TagSource::Manual;
        let content_source = TagSource::Content { keywords: vec!["async".to_string(), "await".to_string()] };

        // Test serialization/deserialization
        let sources = vec![embedding_source, rule_source, manual_source, content_source];
        for source in sources {
            let serialized = serde_json::to_string(&source).unwrap();
            let deserialized: TagSource = serde_json::from_str(&serialized).unwrap();
            assert_eq!(source, deserialized);
        }
    }
} 