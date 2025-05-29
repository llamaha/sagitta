// Conversation branching logic and management
// TODO: Implement actual branching algorithms

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::agent::conversation::types::{Conversation, ConversationBranch, BranchStatus};
use crate::agent::message::types::AgentMessage;
use crate::llm::client::Role;

/// Context-aware conversation branching manager
pub struct ConversationBranchingManager {
    /// Configuration for branching behavior
    config: BranchingConfig,
    
    /// Branch success prediction model
    success_predictor: BranchSuccessPredictor,
    
    /// Branch point detection engine
    branch_detector: BranchPointDetector,
}

/// Configuration for conversation branching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchingConfig {
    /// Minimum confidence threshold for automatic branching suggestions
    pub auto_branch_threshold: f32,
    
    /// Maximum number of active branches per conversation
    pub max_active_branches: usize,
    
    /// Whether to enable automatic branch point detection
    pub enable_auto_detection: bool,
    
    /// Whether to enable branch success prediction
    pub enable_success_prediction: bool,
    
    /// Minimum message count before considering branching
    pub min_messages_for_branching: usize,
    
    /// Context window size for branch analysis
    pub context_window_size: usize,
}

impl Default for BranchingConfig {
    fn default() -> Self {
        Self {
            auto_branch_threshold: 0.7,
            max_active_branches: 5,
            enable_auto_detection: true,
            enable_success_prediction: true,
            min_messages_for_branching: 3,
            context_window_size: 10,
        }
    }
}

/// Branch point suggestion with context analysis
#[derive(Debug, Clone)]
pub struct BranchSuggestion {
    /// Suggested branch point (message ID)
    pub message_id: Uuid,
    
    /// Confidence score for this suggestion (0.0-1.0)
    pub confidence: f32,
    
    /// Reason for suggesting this branch point
    pub reason: BranchReason,
    
    /// Suggested branch title
    pub suggested_title: String,
    
    /// Predicted success probability
    pub success_probability: Option<f32>,
    
    /// Context that led to this suggestion
    pub context: BranchContext,
}

/// Reasons for suggesting a branch point
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchReason {
    /// Multiple solution approaches detected
    MultipleSolutions,
    
    /// Error or failure detected, alternative approach needed
    ErrorRecovery,
    
    /// User expressed uncertainty or asked for alternatives
    UserUncertainty,
    
    /// Complex problem that could benefit from parallel exploration
    ComplexProblem,
    
    /// Different tool or approach could be more effective
    AlternativeApproach,
    
    /// Experimental or risky approach suggested
    ExperimentalApproach,
    
    /// User explicitly requested branching
    UserRequested,
}

/// Context information for branch suggestions
#[derive(Debug, Clone)]
pub struct BranchContext {
    /// Recent messages that influenced the suggestion
    pub relevant_messages: Vec<Uuid>,
    
    /// Keywords or phrases that triggered the suggestion
    pub trigger_keywords: Vec<String>,
    
    /// Current conversation state
    pub conversation_state: ConversationState,
    
    /// Project context if available
    pub project_context: Option<String>,
    
    /// Tools or approaches mentioned
    pub mentioned_tools: Vec<String>,
}

/// Current state of the conversation for branching analysis
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConversationState {
    /// Initial problem exploration
    ProblemExploration,
    
    /// Solution development in progress
    SolutionDevelopment,
    
    /// Error or issue encountered
    ErrorState,
    
    /// Multiple options being considered
    OptionEvaluation,
    
    /// Implementation phase
    Implementation,
    
    /// Testing or validation
    Testing,
    
    /// Completion or wrap-up
    Completion,
}

/// Branch success prediction engine
pub struct BranchSuccessPredictor {
    /// Historical success patterns
    success_patterns: HashMap<String, f32>,
    
    /// Configuration for prediction
    config: PredictionConfig,
}

/// Configuration for branch success prediction
#[derive(Debug, Clone)]
pub struct PredictionConfig {
    /// Weight for historical patterns
    pub pattern_weight: f32,
    
    /// Weight for context similarity
    pub context_weight: f32,
    
    /// Weight for user behavior patterns
    pub behavior_weight: f32,
    
    /// Minimum data points required for prediction
    pub min_data_points: usize,
}

impl Default for PredictionConfig {
    fn default() -> Self {
        Self {
            pattern_weight: 0.4,
            context_weight: 0.3,
            behavior_weight: 0.3,
            min_data_points: 5,
        }
    }
}

/// Branch point detection engine
pub struct BranchPointDetector {
    /// Keywords that suggest branching opportunities
    branch_keywords: Vec<String>,
    
    /// Patterns that indicate uncertainty or alternatives
    uncertainty_patterns: Vec<String>,
    
    /// Error patterns that suggest recovery branching
    error_patterns: Vec<String>,
}

impl Default for BranchPointDetector {
    fn default() -> Self {
        Self {
            branch_keywords: vec![
                "alternative".to_string(),
                "different approach".to_string(),
                "another way".to_string(),
                "try something else".to_string(),
                "what if".to_string(),
                "maybe we could".to_string(),
                "alternatively".to_string(),
                "or we could".to_string(),
                "let's try".to_string(),
                "experiment".to_string(),
            ],
            uncertainty_patterns: vec![
                "not sure".to_string(),
                "uncertain".to_string(),
                "maybe".to_string(),
                "perhaps".to_string(),
                "might work".to_string(),
                "could try".to_string(),
                "not confident".to_string(),
                "unsure".to_string(),
            ],
            error_patterns: vec![
                "error".to_string(),
                "failed".to_string(),
                "doesn't work".to_string(),
                "not working".to_string(),
                "issue".to_string(),
                "problem".to_string(),
                "bug".to_string(),
                "exception".to_string(),
            ],
        }
    }
}

impl ConversationBranchingManager {
    /// Create a new branching manager
    pub fn new(config: BranchingConfig) -> Self {
        Self {
            config,
            success_predictor: BranchSuccessPredictor::new(PredictionConfig::default()),
            branch_detector: BranchPointDetector::default(),
        }
    }
    
    /// Create branching manager with default configuration
    pub fn with_default_config() -> Self {
        Self::new(BranchingConfig::default())
    }
    
    /// Analyze conversation for potential branch points
    pub fn analyze_branch_opportunities(&self, conversation: &Conversation) -> Result<Vec<BranchSuggestion>> {
        if !self.config.enable_auto_detection {
            return Ok(Vec::new());
        }
        
        if conversation.messages.len() < self.config.min_messages_for_branching {
            return Ok(Vec::new());
        }
        
        let mut suggestions = Vec::new();
        
        // Analyze recent messages for branch opportunities
        let recent_messages = self.get_recent_messages(conversation);
        
        for (i, message) in recent_messages.iter().enumerate() {
            if let Some(suggestion) = self.analyze_message_for_branching(message, &recent_messages, i, conversation)? {
                if suggestion.confidence >= self.config.auto_branch_threshold {
                    suggestions.push(suggestion);
                }
            }
        }
        
        // Sort by confidence (highest first)
        suggestions.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
        
        // Limit to max active branches
        suggestions.truncate(self.config.max_active_branches);
        
        Ok(suggestions)
    }
    
    /// Get recent messages within the context window
    fn get_recent_messages<'a>(&self, conversation: &'a Conversation) -> Vec<&'a AgentMessage> {
        let start_index = conversation.messages.len().saturating_sub(self.config.context_window_size);
        conversation.messages[start_index..].iter().collect()
    }
    
    /// Analyze a single message for branching opportunities
    fn analyze_message_for_branching(
        &self,
        message: &AgentMessage,
        context_messages: &[&AgentMessage],
        message_index: usize,
        conversation: &Conversation,
    ) -> Result<Option<BranchSuggestion>> {
        let content = &message.content.to_lowercase();
        
        // Check for branch keywords
        let mut confidence: f32 = 0.0;
        let mut reasons = Vec::new();
        let mut trigger_keywords = Vec::new();
        
        // Detect uncertainty patterns
        for pattern in &self.branch_detector.uncertainty_patterns {
            if content.contains(pattern) {
                confidence += 0.3;
                reasons.push(BranchReason::UserUncertainty);
                trigger_keywords.push(pattern.clone());
            }
        }
        
        // Detect error patterns
        for pattern in &self.branch_detector.error_patterns {
            if content.contains(pattern) {
                confidence += 0.4;
                reasons.push(BranchReason::ErrorRecovery);
                trigger_keywords.push(pattern.clone());
            }
        }
        
        // Detect alternative approach keywords
        for keyword in &self.branch_detector.branch_keywords {
            if content.contains(keyword) {
                confidence += 0.2;
                reasons.push(BranchReason::AlternativeApproach);
                trigger_keywords.push(keyword.clone());
            }
        }
        
        // Analyze conversation state
        let conversation_state = self.analyze_conversation_state(context_messages);
        
        // Adjust confidence based on conversation state
        match conversation_state {
            ConversationState::ErrorState => confidence += 0.3,
            ConversationState::OptionEvaluation => confidence += 0.2,
            ConversationState::ProblemExploration => confidence += 0.1,
            _ => {}
        }
        
        // Check if user is asking questions (indicates uncertainty)
        if message.role == Role::User && content.contains('?') {
            confidence += 0.1;
            reasons.push(BranchReason::UserUncertainty);
        }
        
        // Check for complex problems (long messages, multiple tools mentioned)
        if content.len() > 500 {
            confidence += 0.1;
            reasons.push(BranchReason::ComplexProblem);
        }
        
        if confidence < 0.3 {
            return Ok(None);
        }
        
        // Determine primary reason
        let primary_reason = reasons.into_iter().next().unwrap_or(BranchReason::AlternativeApproach);
        
        // Generate suggested title
        let suggested_title = self.generate_branch_title(&primary_reason, &trigger_keywords);
        
        // Predict success probability
        let success_probability = if self.config.enable_success_prediction {
            Some(self.success_predictor.predict_success(message, context_messages, &primary_reason)?)
        } else {
            None
        };
        
        // Build context
        let context = BranchContext {
            relevant_messages: context_messages.iter().map(|m| m.id).collect(),
            trigger_keywords,
            conversation_state,
            project_context: conversation.project_context.as_ref().map(|pc| pc.name.clone()),
            mentioned_tools: self.extract_mentioned_tools(content),
        };
        
        Ok(Some(BranchSuggestion {
            message_id: message.id,
            confidence: confidence.min(1.0),
            reason: primary_reason,
            suggested_title,
            success_probability,
            context,
        }))
    }
    
    /// Analyze the current state of the conversation
    fn analyze_conversation_state(&self, messages: &[&AgentMessage]) -> ConversationState {
        if messages.is_empty() {
            return ConversationState::ProblemExploration;
        }
        
        let recent_content: String = messages.iter()
            .rev()
            .take(3)
            .map(|m| m.content.to_lowercase())
            .collect::<Vec<_>>()
            .join(" ");
        
        // Check for error indicators
        if recent_content.contains("error") || recent_content.contains("failed") || recent_content.contains("issue") {
            return ConversationState::ErrorState;
        }
        
        // Check for option evaluation
        if recent_content.contains("option") || recent_content.contains("choice") || recent_content.contains("alternative") {
            return ConversationState::OptionEvaluation;
        }
        
        // Check for implementation phase
        if recent_content.contains("implement") || recent_content.contains("code") || recent_content.contains("build") {
            return ConversationState::Implementation;
        }
        
        // Check for testing phase
        if recent_content.contains("test") || recent_content.contains("verify") || recent_content.contains("check") {
            return ConversationState::Testing;
        }
        
        // Check for completion
        if recent_content.contains("done") || recent_content.contains("complete") || recent_content.contains("finished") {
            return ConversationState::Completion;
        }
        
        // Default to solution development
        ConversationState::SolutionDevelopment
    }
    
    /// Generate a suggested branch title based on the reason and context
    fn generate_branch_title(&self, reason: &BranchReason, keywords: &[String]) -> String {
        match reason {
            BranchReason::MultipleSolutions => "Alternative Solution".to_string(),
            BranchReason::ErrorRecovery => "Error Recovery Approach".to_string(),
            BranchReason::UserUncertainty => "Alternative Exploration".to_string(),
            BranchReason::ComplexProblem => "Complex Problem Branch".to_string(),
            BranchReason::AlternativeApproach => {
                if let Some(keyword) = keywords.first() {
                    format!("Alternative: {}", keyword.to_title_case())
                } else {
                    "Alternative Approach".to_string()
                }
            },
            BranchReason::ExperimentalApproach => "Experimental Approach".to_string(),
            BranchReason::UserRequested => "User Requested Branch".to_string(),
        }
    }
    
    /// Extract mentioned tools from message content
    fn extract_mentioned_tools(&self, content: &str) -> Vec<String> {
        let tool_keywords = vec![
            "git", "cargo", "npm", "pip", "docker", "kubernetes", "terraform",
            "grep", "sed", "awk", "curl", "wget", "ssh", "scp", "rsync",
            "vim", "emacs", "vscode", "intellij", "eclipse",
            "rust", "python", "javascript", "typescript", "go", "java", "c++",
        ];
        
        tool_keywords.iter()
            .filter(|tool| content.to_lowercase().contains(*tool))
            .map(|tool| tool.to_string())
            .collect()
    }
    
    /// Create a context-aware branch with intelligent defaults
    pub fn create_context_aware_branch(
        &self,
        conversation: &mut Conversation,
        suggestion: &BranchSuggestion,
    ) -> Result<Uuid> {
        let branch = ConversationBranch::new(
            suggestion.suggested_title.clone(),
            Some(suggestion.message_id),
        );
        
        let branch_id = branch.id;
        conversation.branches.push(branch);
        conversation.last_active = Utc::now();
        
        Ok(branch_id)
    }
    
    /// Evaluate branch success and update predictions
    pub fn evaluate_branch_success(&mut self, branch: &ConversationBranch, success_score: f32) -> Result<()> {
        self.success_predictor.update_success_pattern(branch, success_score)?;
        Ok(())
    }
    
    /// Get configuration
    pub fn get_config(&self) -> &BranchingConfig {
        &self.config
    }
    
    /// Update configuration
    pub fn update_config(&mut self, config: BranchingConfig) {
        self.config = config;
    }
}

impl BranchSuccessPredictor {
    /// Create a new success predictor
    pub fn new(config: PredictionConfig) -> Self {
        Self {
            success_patterns: HashMap::new(),
            config,
        }
    }
    
    /// Predict success probability for a potential branch
    pub fn predict_success(
        &self,
        message: &AgentMessage,
        context: &[&AgentMessage],
        reason: &BranchReason,
    ) -> Result<f32> {
        let mut prediction = 0.5; // Base probability
        
        // Adjust based on branch reason
        match reason {
            BranchReason::ErrorRecovery => prediction += 0.2, // Error recovery often successful
            BranchReason::UserRequested => prediction += 0.3, // User-requested branches often successful
            BranchReason::AlternativeApproach => prediction += 0.1,
            BranchReason::ExperimentalApproach => prediction -= 0.1, // Experimental approaches riskier
            _ => {}
        }
        
        // Adjust based on message characteristics
        if message.role == Role::User {
            prediction += 0.1; // User-initiated branches often more successful
        }
        
        // Adjust based on context length (more context = better success)
        let context_factor = (context.len() as f32 / 10.0).min(1.0) * 0.1;
        prediction += context_factor;
        
        // Look up historical patterns
        let pattern_key = format!("{:?}", reason);
        if let Some(&historical_success) = self.success_patterns.get(&pattern_key) {
            prediction = prediction * (1.0 - self.config.pattern_weight) + 
                        historical_success * self.config.pattern_weight;
        }
        
        Ok(prediction.clamp(0.0, 1.0))
    }
    
    /// Update success patterns based on actual outcomes
    pub fn update_success_pattern(&mut self, branch: &ConversationBranch, success_score: f32) -> Result<()> {
        // For now, we'll use a simple pattern based on branch title
        // In a real implementation, this would be more sophisticated
        let pattern_key = if branch.title.contains("Error") {
            "ErrorRecovery".to_string()
        } else if branch.title.contains("Alternative") {
            "AlternativeApproach".to_string()
        } else if branch.title.contains("Experimental") {
            "ExperimentalApproach".to_string()
        } else {
            "General".to_string()
        };
        
        // Update running average
        let current_avg = self.success_patterns.get(&pattern_key).unwrap_or(&0.5);
        let new_avg = (current_avg + success_score) / 2.0;
        self.success_patterns.insert(pattern_key, new_avg);
        
        Ok(())
    }
}

// Helper trait for string formatting
trait StringExt {
    fn to_title_case(&self) -> String;
}

impl StringExt for str {
    fn to_title_case(&self) -> String {
        self.split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::message::types::AgentMessage;

    #[test]
    fn test_branching_manager_creation() {
        let manager = ConversationBranchingManager::with_default_config();
        assert!(manager.config.enable_auto_detection);
        assert_eq!(manager.config.max_active_branches, 5);
    }

    #[test]
    fn test_branch_suggestion_confidence() {
        // Create manager with lower threshold for testing
        let mut config = BranchingConfig::default();
        config.auto_branch_threshold = 0.3; // Lower threshold for testing
        config.min_messages_for_branching = 2; // Lower minimum for testing
        let manager = ConversationBranchingManager::new(config);
        let mut conversation = Conversation::new("Test".to_string(), None);
        
        // Add messages that should trigger branching suggestions with high confidence
        conversation.add_message(AgentMessage::user("I'm not sure about this approach, maybe we could try something else"));
        conversation.add_message(AgentMessage::assistant("Let me help you explore alternatives"));
        conversation.add_message(AgentMessage::user("What if we try a different method? This error is confusing"));
        
        let suggestions = manager.analyze_branch_opportunities(&conversation).unwrap();
        
        // Should have at least one suggestion due to uncertainty keywords
        assert!(!suggestions.is_empty(), "Expected at least one branching suggestion");
        
        // Check that confidence is reasonable
        for suggestion in &suggestions {
            assert!(suggestion.confidence >= 0.0 && suggestion.confidence <= 1.0);
            assert!(suggestion.confidence >= 0.3, "Confidence should be at least 0.3, got {}", suggestion.confidence);
        }
    }

    #[test]
    fn test_conversation_state_analysis() {
        let manager = ConversationBranchingManager::with_default_config();
        
        let error_msg = AgentMessage::user("This is giving me an error");
        let error_messages = vec![&error_msg];
        let state = manager.analyze_conversation_state(&error_messages);
        assert_eq!(state, ConversationState::ErrorState);
        
        let option_msg = AgentMessage::user("What are my options here?");
        let option_messages = vec![&option_msg];
        let state = manager.analyze_conversation_state(&option_messages);
        assert_eq!(state, ConversationState::OptionEvaluation);
    }

    #[test]
    fn test_branch_title_generation() {
        let manager = ConversationBranchingManager::with_default_config();
        
        let title = manager.generate_branch_title(&BranchReason::ErrorRecovery, &[]);
        assert_eq!(title, "Error Recovery Approach");
        
        let title = manager.generate_branch_title(
            &BranchReason::AlternativeApproach, 
            &["different approach".to_string()]
        );
        assert!(title.contains("Alternative"));
    }

    #[test]
    fn test_success_prediction() {
        let predictor = BranchSuccessPredictor::new(PredictionConfig::default());
        
        let message = AgentMessage::user("Let's try a different approach");
        let context = vec![];
        
        let prediction = predictor.predict_success(&message, &context, &BranchReason::UserRequested).unwrap();
        
        assert!(prediction >= 0.0 && prediction <= 1.0);
        assert!(prediction > 0.5); // User-requested should have higher success probability
    }

    #[test]
    fn test_tool_extraction() {
        let manager = ConversationBranchingManager::with_default_config();
        
        let content = "Let's use git to check the history and then run cargo test";
        let tools = manager.extract_mentioned_tools(content);
        
        assert!(tools.contains(&"git".to_string()));
        assert!(tools.contains(&"cargo".to_string()));
    }

    #[test]
    fn test_context_aware_branch_creation() {
        let manager = ConversationBranchingManager::with_default_config();
        let mut conversation = Conversation::new("Test".to_string(), None);
        
        let suggestion = BranchSuggestion {
            message_id: Uuid::new_v4(),
            confidence: 0.8,
            reason: BranchReason::AlternativeApproach,
            suggested_title: "Alternative Solution".to_string(),
            success_probability: Some(0.7),
            context: BranchContext {
                relevant_messages: vec![],
                trigger_keywords: vec!["alternative".to_string()],
                conversation_state: ConversationState::SolutionDevelopment,
                project_context: None,
                mentioned_tools: vec![],
            },
        };
        
        let branch_id = manager.create_context_aware_branch(&mut conversation, &suggestion).unwrap();
        
        assert_eq!(conversation.branches.len(), 1);
        assert_eq!(conversation.branches[0].id, branch_id);
        assert_eq!(conversation.branches[0].title, "Alternative Solution");
    }
} 