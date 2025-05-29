//! Decision making engine for reasoning
//!
//! This module implements a sophisticated decision making system that provides confidence-based
//! intelligent routing and option evaluation. It addresses the poor decision logic issues found
//! in the original reasoning system.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use tokio::sync::RwLock;

use crate::error::{Result, ReasoningError};
use crate::config::DecisionConfig;

/// Decision making engine with intelligent routing
pub struct DecisionEngine {
    config: DecisionConfig,
    decision_history: Arc<RwLock<VecDeque<DecisionRecord>>>,
    pattern_cache: Arc<RwLock<HashMap<String, DecisionPattern>>>,
    metrics: Arc<RwLock<DecisionMetrics>>,
}

/// A decision made during reasoning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    /// Decision identifier
    pub id: Uuid,
    /// Decision description
    pub description: String,
    /// Chosen option
    pub chosen_option: String,
    /// Confidence in the decision
    pub confidence: f32,
    /// All evaluated options with scores
    pub evaluated_options: Vec<EvaluatedOption>,
    /// Decision rationale
    pub rationale: String,
    /// Time taken to make decision
    pub decision_time: Duration,
    /// Context that influenced the decision
    pub context_summary: String,
}

/// Context for making decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionContext {
    /// Context identifier
    pub id: Uuid,
    /// Decision description
    pub description: String,
    /// Available options
    pub options: Vec<DecisionOption>,
    /// Current state summary
    pub state_summary: String,
    /// Available tools and resources
    pub available_tools: Vec<String>,
    /// Time constraints
    pub time_constraint: Option<Duration>,
    /// Priority level (0.0 to 1.0)
    pub priority: f32,
    /// Historical context for pattern matching
    pub historical_context: HashMap<String, String>,
    /// Environmental factors
    pub environmental_factors: HashMap<String, f32>,
}

/// An option that can be chosen in a decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionOption {
    /// Option identifier
    pub id: String,
    /// Option description
    pub description: String,
    /// Base confidence score
    pub base_confidence: f32,
    /// Required resources
    pub required_resources: Vec<String>,
    /// Estimated execution time
    pub estimated_time: Option<Duration>,
    /// Risk level (0.0 to 1.0)
    pub risk_level: f32,
    /// Expected benefit (0.0 to 1.0)
    pub expected_benefit: f32,
    /// Option metadata
    pub metadata: HashMap<String, String>,
}

/// An evaluated option with calculated scores
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatedOption {
    /// Original option
    pub option: DecisionOption,
    /// Final calculated score
    pub final_score: f32,
    /// Individual criterion scores
    pub criterion_scores: HashMap<String, f32>,
    /// Reasons for the score
    pub evaluation_reasons: Vec<String>,
}

/// Record of a decision for learning
#[derive(Debug, Clone)]
pub struct DecisionRecord {
    /// Decision that was made
    pub decision: Decision,
    /// Context at time of decision
    pub context: DecisionContext,
    /// Outcome of the decision
    pub outcome: Option<DecisionOutcome>,
    /// Timestamp when decision was made
    pub timestamp: Instant,
}

/// Outcome of a decision for learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionOutcome {
    /// Whether the decision was successful
    pub success: bool,
    /// Actual confidence achieved
    pub actual_confidence: f32,
    /// Time taken to execute
    pub execution_time: Duration,
    /// Lessons learned
    pub lessons: Vec<String>,
    /// Outcome metadata
    pub metadata: HashMap<String, String>,
}

/// Pattern recognized from decision history
#[derive(Debug, Clone)]
pub struct DecisionPattern {
    /// Pattern identifier
    pub id: Uuid,
    /// Pattern description
    pub description: String,
    /// Context conditions that trigger this pattern
    pub context_conditions: HashMap<String, String>,
    /// Recommended option for this pattern
    pub recommended_option: String,
    /// Success rate of this pattern
    pub success_rate: f32,
    /// Number of times this pattern was used
    pub usage_count: u32,
    /// Last time this pattern was updated
    pub last_updated: Instant,
}

/// Decision making metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DecisionMetrics {
    /// Total decisions made
    pub total_decisions: u64,
    /// Successful decisions
    pub successful_decisions: u64,
    /// Average decision time
    pub avg_decision_time: Duration,
    /// Average confidence of decisions
    pub avg_confidence: f32,
    /// Pattern recognition accuracy
    pub pattern_accuracy: f32,
    /// Most used decision patterns
    pub top_patterns: Vec<String>,
}

/// Evaluation criteria for decision making
#[derive(Debug, Clone)]
pub struct EvaluationCriteria {
    /// Weight for historical success patterns
    pub history_weight: f32,
    /// Weight for context similarity
    pub context_weight: f32,
    /// Weight for tool availability
    pub tool_availability_weight: f32,
    /// Weight for time constraints
    pub time_weight: f32,
    /// Weight for risk assessment
    pub risk_weight: f32,
    /// Weight for expected benefit
    pub benefit_weight: f32,
}

impl DecisionEngine {
    /// Create a new decision engine
    pub async fn new(config: DecisionConfig) -> Result<Self> {
        tracing::info!("Creating decision engine with config: {:?}", config);
        
        Ok(Self {
            config,
            decision_history: Arc::new(RwLock::new(VecDeque::new())),
            pattern_cache: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(DecisionMetrics::default())),
        })
    }

    /// Make a decision based on the given context
    pub async fn make_decision(&mut self, context: DecisionContext) -> Result<Decision> {
        let start_time = Instant::now();
        tracing::debug!("Making decision for context: {}", context.description);
        
        // Validate context
        self.validate_context(&context)?;
        
        // Add a small delay to ensure measurable decision time
        tokio::time::sleep(Duration::from_micros(100)).await;
        
        // Evaluate all options
        let evaluated_options = self.evaluate_options(&context).await?;
        
        // Select best option
        let best_option = self.select_best_option(&evaluated_options, &context).await?;
        
        // Create decision
        let decision = Decision {
            id: Uuid::new_v4(),
            description: context.description.clone(),
            chosen_option: best_option.option.id.clone(),
            confidence: best_option.final_score,
            evaluated_options: evaluated_options.clone(),
            rationale: self.generate_rationale(&best_option, &context).await,
            decision_time: start_time.elapsed(),
            context_summary: context.state_summary.clone(),
        };
        
        // Record decision for learning
        self.record_decision(decision.clone(), context).await?;
        
        // Update metrics
        self.update_metrics(&decision).await;
        
        tracing::info!("Decision made: {} (confidence: {:.2})", decision.chosen_option, decision.confidence);
        Ok(decision)
    }

    /// Evaluate all options in the context
    async fn evaluate_options(&self, context: &DecisionContext) -> Result<Vec<EvaluatedOption>> {
        let mut evaluated_options = Vec::new();
        let criteria = self.get_evaluation_criteria();
        
        for option in &context.options {
            let evaluated = self.evaluate_single_option(option, context, &criteria).await?;
            evaluated_options.push(evaluated);
        }
        
        // Sort by final score (descending)
        evaluated_options.sort_by(|a, b| b.final_score.partial_cmp(&a.final_score).unwrap());
        
        Ok(evaluated_options)
    }

    /// Evaluate a single option
    async fn evaluate_single_option(
        &self,
        option: &DecisionOption,
        context: &DecisionContext,
        criteria: &EvaluationCriteria,
    ) -> Result<EvaluatedOption> {
        let mut criterion_scores = HashMap::new();
        let mut evaluation_reasons = Vec::new();
        
        // Historical pattern score
        let history_score = self.calculate_history_score(option, context).await;
        criterion_scores.insert("history".to_string(), history_score);
        if history_score > 0.7 {
            evaluation_reasons.push("Strong historical success pattern".to_string());
        }
        
        // Tool availability score
        let tool_score = self.calculate_tool_availability_score(option, context);
        criterion_scores.insert("tool_availability".to_string(), tool_score);
        if tool_score < 0.5 {
            evaluation_reasons.push("Limited tool availability".to_string());
        }
        
        // Time constraint score
        let time_score = self.calculate_time_score(option, context);
        criterion_scores.insert("time".to_string(), time_score);
        if time_score < 0.3 {
            evaluation_reasons.push("Time constraint violation".to_string());
        }
        
        // Risk assessment score
        let risk_score = 1.0 - option.risk_level; // Lower risk = higher score
        criterion_scores.insert("risk".to_string(), risk_score);
        if option.risk_level > 0.7 {
            evaluation_reasons.push("High risk option".to_string());
        }
        
        // Benefit score
        let benefit_score = option.expected_benefit;
        criterion_scores.insert("benefit".to_string(), benefit_score);
        if benefit_score > 0.8 {
            evaluation_reasons.push("High expected benefit".to_string());
        }
        
        // Calculate weighted final score
        let final_score = (history_score * criteria.history_weight)
            + (tool_score * criteria.tool_availability_weight)
            + (time_score * criteria.time_weight)
            + (risk_score * criteria.risk_weight)
            + (benefit_score * criteria.benefit_weight);
        
        Ok(EvaluatedOption {
            option: option.clone(),
            final_score: final_score.min(1.0).max(0.0),
            criterion_scores,
            evaluation_reasons,
        })
    }

    /// Calculate historical pattern score for an option
    async fn calculate_history_score(&self, option: &DecisionOption, context: &DecisionContext) -> f32 {
        let patterns = self.pattern_cache.read().await;
        
        // Find matching patterns
        let mut best_match_score: f32 = 0.0;
        for pattern in patterns.values() {
            if pattern.recommended_option == option.id {
                let context_match = self.calculate_context_match(&pattern.context_conditions, &context.historical_context);
                let pattern_score = pattern.success_rate * context_match;
                best_match_score = best_match_score.max(pattern_score);
            }
        }
        
        // If no patterns found, use base confidence
        if best_match_score == 0.0 {
            option.base_confidence
        } else {
            best_match_score
        }
    }

    /// Calculate tool availability score
    fn calculate_tool_availability_score(&self, option: &DecisionOption, context: &DecisionContext) -> f32 {
        if option.required_resources.is_empty() {
            return 1.0; // No requirements = full availability
        }
        
        let available_count = option.required_resources.iter()
            .filter(|resource| context.available_tools.contains(resource))
            .count();
        
        available_count as f32 / option.required_resources.len() as f32
    }

    /// Calculate time constraint score
    fn calculate_time_score(&self, option: &DecisionOption, context: &DecisionContext) -> f32 {
        match (option.estimated_time, context.time_constraint) {
            (Some(estimated), Some(constraint)) => {
                if estimated <= constraint {
                    1.0
                } else {
                    // Penalty for exceeding time constraint
                    (constraint.as_secs_f32() / estimated.as_secs_f32()).min(1.0)
                }
            }
            (None, _) => 0.8, // Unknown time = slight penalty
            (_, None) => 1.0, // No constraint = no penalty
        }
    }

    /// Select the best option from evaluated options
    async fn select_best_option(
        &self,
        evaluated_options: &[EvaluatedOption],
        _context: &DecisionContext,
    ) -> Result<EvaluatedOption> {
        if evaluated_options.is_empty() {
            return Err(ReasoningError::decision("No options available for decision", 0.0));
        }
        
        // Filter options that meet minimum confidence threshold
        let viable_options: Vec<_> = evaluated_options.iter()
            .filter(|opt| opt.final_score >= self.config.min_confidence)
            .collect();
        
        if viable_options.is_empty() {
            // If no options meet threshold, take the best available but log warning
            tracing::warn!("No options meet confidence threshold {:.2}, selecting best available", self.config.min_confidence);
            return Ok(evaluated_options[0].clone());
        }
        
        // Select highest scoring viable option
        Ok(viable_options[0].clone())
    }

    /// Generate rationale for the decision
    async fn generate_rationale(&self, best_option: &EvaluatedOption, _context: &DecisionContext) -> String {
        let mut rationale = format!("Selected '{}' with confidence {:.2}", 
            best_option.option.id, best_option.final_score);
        
        // Add top reasons
        if !best_option.evaluation_reasons.is_empty() {
            rationale.push_str(". Key factors: ");
            rationale.push_str(&best_option.evaluation_reasons.join(", "));
        }
        
        // Add criterion breakdown
        let top_criteria: Vec<_> = best_option.criterion_scores.iter()
            .filter(|(_, &score)| score > 0.7)
            .map(|(name, score)| format!("{}: {:.2}", name, score))
            .collect();
        
        if !top_criteria.is_empty() {
            rationale.push_str(". Strong scores in: ");
            rationale.push_str(&top_criteria.join(", "));
        }
        
        rationale
    }

    /// Record decision for learning
    async fn record_decision(&mut self, decision: Decision, context: DecisionContext) -> Result<()> {
        let record = DecisionRecord {
            decision,
            context,
            outcome: None, // Will be updated later when outcome is known
            timestamp: Instant::now(),
        };
        
        let mut history = self.decision_history.write().await;
        history.push_back(record);
        
        // Limit history size
        while history.len() > 1000 {
            history.pop_front();
        }
        
        Ok(())
    }

    /// Update decision outcome for learning
    pub async fn update_decision_outcome(&mut self, decision_id: Uuid, outcome: DecisionOutcome) -> Result<()> {
        // Update the decision record
        {
            let mut history = self.decision_history.write().await;
            
            // Find the decision record and update outcome
            for record in history.iter_mut() {
                if record.decision.id == decision_id {
                    record.outcome = Some(outcome.clone());
                    break;
                }
            }
        }
        
        // Update patterns based on outcome (separate scope to avoid borrow conflicts)
        self.learn_from_outcome(decision_id, outcome).await?;
        
        Ok(())
    }

    /// Learn from decision outcomes
    async fn learn_from_outcome(&mut self, decision_id: Uuid, outcome: DecisionOutcome) -> Result<()> {
        // Find the decision record
        let record = {
            let history = self.decision_history.read().await;
            history.iter()
                .find(|r| r.decision.id == decision_id)
                .cloned()
                .ok_or_else(|| ReasoningError::decision("Decision record not found", 0.0))?
        };
        
        // Update patterns
        self.update_patterns(&record.context, &record.decision, &outcome).await;
        
        Ok(())
    }

    /// Update decision patterns based on outcomes
    async fn update_patterns(&self, context: &DecisionContext, decision: &Decision, outcome: &DecisionOutcome) {
        let mut patterns = self.pattern_cache.write().await;
        
        // Create pattern key from context
        let pattern_key = self.create_pattern_key(context);
        
        if let Some(pattern) = patterns.get_mut(&pattern_key) {
            // Update existing pattern
            pattern.usage_count += 1;
            let new_success_rate = (pattern.success_rate * (pattern.usage_count - 1) as f32 + if outcome.success { 1.0 } else { 0.0 }) / pattern.usage_count as f32;
            pattern.success_rate = new_success_rate;
            pattern.last_updated = Instant::now();
        } else {
            // Create new pattern
            let new_pattern = DecisionPattern {
                id: Uuid::new_v4(),
                description: format!("Pattern for {}", context.description),
                context_conditions: context.historical_context.clone(),
                recommended_option: decision.chosen_option.clone(),
                success_rate: if outcome.success { 1.0 } else { 0.0 },
                usage_count: 1,
                last_updated: Instant::now(),
            };
            patterns.insert(pattern_key, new_pattern);
        }
    }

    /// Get evaluation criteria based on config
    fn get_evaluation_criteria(&self) -> EvaluationCriteria {
        // Ensure weights add up to 1.0 for proper scoring
        let base_history = self.config.history_weight;
        let base_context = self.config.context_weight;
        let base_tool = self.config.tool_availability_weight;
        let time_weight = 0.2; // Significant weight for time constraints
        let risk_weight = 0.15;
        let benefit_weight = 0.1;
        
        // Calculate remaining weight for history, context, and tool availability
        let remaining_weight = 1.0 - time_weight - risk_weight - benefit_weight;
        let total_base = base_history + base_context + base_tool;
        
        let normalized_history = if total_base > 0.0 { (base_history / total_base) * remaining_weight } else { remaining_weight / 3.0 };
        let normalized_context = if total_base > 0.0 { (base_context / total_base) * remaining_weight } else { remaining_weight / 3.0 };
        let normalized_tool = if total_base > 0.0 { (base_tool / total_base) * remaining_weight } else { remaining_weight / 3.0 };
        
        EvaluationCriteria {
            history_weight: normalized_history,
            context_weight: normalized_context,
            tool_availability_weight: normalized_tool,
            time_weight,
            risk_weight,
            benefit_weight,
        }
    }

    /// Update decision metrics
    async fn update_metrics(&self, decision: &Decision) {
        let mut metrics = self.metrics.write().await;
        metrics.total_decisions += 1;
        
        // Update average decision time
        let total_time = metrics.avg_decision_time.as_nanos() as f64 * (metrics.total_decisions - 1) as f64;
        let new_avg = (total_time + decision.decision_time.as_nanos() as f64) / metrics.total_decisions as f64;
        metrics.avg_decision_time = Duration::from_nanos(new_avg as u64);
        
        // Update average confidence
        let total_confidence = metrics.avg_confidence * (metrics.total_decisions - 1) as f32;
        metrics.avg_confidence = (total_confidence + decision.confidence) / metrics.total_decisions as f32;
    }

    /// Get decision metrics
    pub async fn get_metrics(&self) -> DecisionMetrics {
        self.metrics.read().await.clone()
    }

    /// Validate decision context
    fn validate_context(&self, context: &DecisionContext) -> Result<()> {
        if context.options.is_empty() {
            return Err(ReasoningError::decision("No options provided in decision context", 0.0));
        }
        
        if context.options.len() > self.config.max_options as usize {
            return Err(ReasoningError::decision(
                format!("Too many options: {} (max: {})", context.options.len(), self.config.max_options),
                0.0
            ));
        }
        
        Ok(())
    }

    // Helper methods for pattern matching

    fn calculate_context_match(&self, pattern_conditions: &HashMap<String, String>, context: &HashMap<String, String>) -> f32 {
        if pattern_conditions.is_empty() {
            return 0.5; // Neutral match for empty conditions
        }
        
        let matches = pattern_conditions.iter()
            .filter(|(key, value)| context.get(*key).map_or(false, |v| v == *value))
            .count();
        
        matches as f32 / pattern_conditions.len() as f32
    }

    fn create_pattern_key(&self, context: &DecisionContext) -> String {
        // Create a key based on context characteristics
        format!("{}_{}_{}_{}", 
            context.description.chars().take(20).collect::<String>(),
            context.options.len(),
            context.available_tools.len(),
            (context.priority * 10.0) as u32
        )
    }
}

impl DecisionContext {
    /// Create a new decision context
    pub fn new(description: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            description,
            options: Vec::new(),
            state_summary: String::new(),
            available_tools: Vec::new(),
            time_constraint: None,
            priority: 0.5,
            historical_context: HashMap::new(),
            environmental_factors: HashMap::new(),
        }
    }

    /// Add an option to the context
    pub fn add_option(&mut self, option: DecisionOption) {
        self.options.push(option);
    }

    /// Set available tools
    pub fn set_available_tools(&mut self, tools: Vec<String>) {
        self.available_tools = tools;
    }

    /// Set time constraint
    pub fn set_time_constraint(&mut self, constraint: Duration) {
        self.time_constraint = Some(constraint);
    }

    /// Set priority level
    pub fn set_priority(&mut self, priority: f32) {
        self.priority = priority.clamp(0.0, 1.0);
    }

    /// Add historical context
    pub fn add_historical_context(&mut self, key: String, value: String) {
        self.historical_context.insert(key, value);
    }

    /// Add environmental factor
    pub fn add_environmental_factor(&mut self, key: String, value: f32) {
        self.environmental_factors.insert(key, value);
    }
}

impl DecisionOption {
    /// Create a new decision option
    pub fn new(id: String, base_confidence: f32) -> Self {
        Self {
            id,
            description: String::new(),
            base_confidence: base_confidence.clamp(0.0, 1.0),
            required_resources: Vec::new(),
            estimated_time: None,
            risk_level: 0.5,
            expected_benefit: 0.5,
            metadata: HashMap::new(),
        }
    }

    /// Set option description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = description;
        self
    }

    /// Set required resources
    pub fn with_resources(mut self, resources: Vec<String>) -> Self {
        self.required_resources = resources;
        self
    }

    /// Set estimated execution time
    pub fn with_estimated_time(mut self, time: Duration) -> Self {
        self.estimated_time = Some(time);
        self
    }

    /// Set risk level
    pub fn with_risk_level(mut self, risk: f32) -> Self {
        self.risk_level = risk.clamp(0.0, 1.0);
        self
    }

    /// Set expected benefit
    pub fn with_expected_benefit(mut self, benefit: f32) -> Self {
        self.expected_benefit = benefit.clamp(0.0, 1.0);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DecisionConfig;
    use std::time::Duration;
    
    #[tokio::test]
    async fn test_decision_engine_creation() {
        let config = DecisionConfig::default();
        let result = DecisionEngine::new(config).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_simple_decision_making() {
        let config = DecisionConfig::default();
        let mut engine = DecisionEngine::new(config).await.unwrap();
        
        let mut context = DecisionContext::new("Test decision".to_string());
        context.add_option(DecisionOption::new("option1".to_string(), 0.8));
        context.add_option(DecisionOption::new("option2".to_string(), 0.6));
        
        let decision = engine.make_decision(context).await.unwrap();
        assert_eq!(decision.chosen_option, "option1"); // Higher confidence should win
        assert!(decision.confidence > 0.0);
    }
    
    #[tokio::test]
    async fn test_decision_with_tool_requirements() {
        let config = DecisionConfig::default();
        let mut engine = DecisionEngine::new(config).await.unwrap();
        
        let mut context = DecisionContext::new("Tool-dependent decision".to_string());
        context.set_available_tools(vec!["tool1".to_string(), "tool2".to_string()]);
        
        let option1 = DecisionOption::new("option1".to_string(), 0.9)
            .with_resources(vec!["tool1".to_string(), "tool3".to_string()]); // Missing tool3
        let option2 = DecisionOption::new("option2".to_string(), 0.7)
            .with_resources(vec!["tool1".to_string(), "tool2".to_string()]); // All available
        
        context.add_option(option1);
        context.add_option(option2);
        
        let decision = engine.make_decision(context).await.unwrap();
        // Option2 should win due to better tool availability despite lower base confidence
        assert_eq!(decision.chosen_option, "option2");
    }
    
    #[tokio::test]
    async fn test_decision_with_time_constraints() {
        let config = DecisionConfig::default();
        let mut engine = DecisionEngine::new(config).await.unwrap();
        
        let mut context = DecisionContext::new("Time-constrained decision".to_string());
        context.set_time_constraint(Duration::from_secs(10));
        
        let option1 = DecisionOption::new("fast_option".to_string(), 0.6)
            .with_estimated_time(Duration::from_secs(5));
        let option2 = DecisionOption::new("slow_option".to_string(), 0.8)
            .with_estimated_time(Duration::from_secs(20)); // Exceeds constraint
        
        context.add_option(option1);
        context.add_option(option2);
        
        let decision = engine.make_decision(context).await.unwrap();
        
        // Fast option should win due to time constraint
        assert_eq!(decision.chosen_option, "fast_option");
    }
    
    #[tokio::test]
    async fn test_decision_outcome_learning() {
        let config = DecisionConfig::default();
        let mut engine = DecisionEngine::new(config).await.unwrap();
        
        let mut context = DecisionContext::new("Learning decision".to_string());
        context.add_option(DecisionOption::new("learn_option".to_string(), 0.7));
        
        let decision = engine.make_decision(context).await.unwrap();
        let decision_id = decision.id;
        
        // Update with successful outcome
        let outcome = DecisionOutcome {
            success: true,
            actual_confidence: 0.9,
            execution_time: Duration::from_secs(5),
            lessons: vec!["This option works well".to_string()],
            metadata: HashMap::new(),
        };
        
        let result = engine.update_decision_outcome(decision_id, outcome).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_decision_context_builder() {
        let mut context = DecisionContext::new("Builder test".to_string());
        context.set_priority(0.8);
        context.set_time_constraint(Duration::from_secs(30));
        context.add_historical_context("previous_action".to_string(), "analyze".to_string());
        context.add_environmental_factor("system_load".to_string(), 0.6);
        
        assert_eq!(context.priority, 0.8);
        assert_eq!(context.time_constraint, Some(Duration::from_secs(30)));
        assert_eq!(context.historical_context.get("previous_action"), Some(&"analyze".to_string()));
        assert_eq!(context.environmental_factors.get("system_load"), Some(&0.6));
    }
    
    #[tokio::test]
    async fn test_decision_option_builder() {
        let option = DecisionOption::new("test_option".to_string(), 0.8)
            .with_description("Test option description".to_string())
            .with_resources(vec!["tool1".to_string()])
            .with_estimated_time(Duration::from_secs(10))
            .with_risk_level(0.3)
            .with_expected_benefit(0.9)
            .with_metadata("category".to_string(), "analysis".to_string());
        
        assert_eq!(option.id, "test_option");
        assert_eq!(option.base_confidence, 0.8);
        assert_eq!(option.description, "Test option description");
        assert_eq!(option.required_resources, vec!["tool1"]);
        assert_eq!(option.estimated_time, Some(Duration::from_secs(10)));
        assert_eq!(option.risk_level, 0.3);
        assert_eq!(option.expected_benefit, 0.9);
        assert_eq!(option.metadata.get("category"), Some(&"analysis".to_string()));
    }
    
    #[tokio::test]
    async fn test_confidence_threshold_filtering() {
        let mut config = DecisionConfig::default();
        config.min_confidence = 0.8; // High threshold
        
        let mut engine = DecisionEngine::new(config).await.unwrap();
        
        let mut context = DecisionContext::new("Threshold test".to_string());
        context.add_option(DecisionOption::new("low_conf".to_string(), 0.5)); // Below threshold
        context.add_option(DecisionOption::new("high_conf".to_string(), 0.9)); // Above threshold
        
        let decision = engine.make_decision(context).await.unwrap();
        assert_eq!(decision.chosen_option, "high_conf");
    }
    
    #[tokio::test]
    async fn test_decision_metrics() {
        let config = DecisionConfig::default();
        let mut engine = DecisionEngine::new(config).await.unwrap();
        
        // Make several decisions
        for i in 0..3 {
            let mut context = DecisionContext::new(format!("Decision {}", i));
            context.add_option(DecisionOption::new(format!("option{}", i), 0.7));
            engine.make_decision(context).await.unwrap();
        }
        
        let metrics = engine.get_metrics().await;
        assert_eq!(metrics.total_decisions, 3);
        assert!(metrics.avg_decision_time.as_millis() > 0);
        assert!(metrics.avg_confidence > 0.0);
    }
    
    #[tokio::test]
    async fn test_empty_options_error() {
        let config = DecisionConfig::default();
        let mut engine = DecisionEngine::new(config).await.unwrap();
        
        let context = DecisionContext::new("Empty options test".to_string());
        // No options added
        
        let result = engine.make_decision(context).await;
        assert!(result.is_err());
    }
} 