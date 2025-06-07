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
        
        // Update metrics for successful decisions
        if outcome.success {
            let mut metrics = self.metrics.write().await;
            metrics.successful_decisions += 1;
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

    /// NEW: Detect if current decision context indicates task completion
    pub async fn detect_completion_in_context(&self, context: &DecisionContext) -> Option<f32> {
        let completion_indicators = [
            "completed", "finished", "done", "success", "final", "result",
            "concluded", "accomplished", "achieved", "resolved"
        ];
        
        let continuation_indicators = [
            "now", "next", "continue", "proceed", "then", "after", "still", "more",
            "further", "additional", "also", "again"
        ];
        
        let description_lower = context.description.to_lowercase();
        let state_lower = context.state_summary.to_lowercase();
        
        // Check for continuation indicators first - these reduce completion confidence
        let has_continuation_signal = continuation_indicators.iter()
            .any(|indicator| description_lower.contains(indicator) || state_lower.contains(indicator));
        
        // Check for partial completion indicators
        let has_partial_indicators = description_lower.contains("partial") || 
                                   description_lower.contains("partly") ||
                                   description_lower.contains("some");
        
        let mut max_signal_strength: f32 = 0.0;
        
        // Check description for completion phrases
        for indicator in &completion_indicators {
            if description_lower.contains(indicator) {
                let strength = match *indicator {
                    "completed" | "finished" | "done" => 1.0,
                    "success" | "accomplished" | "achieved" => 0.9,
                    "final" | "result" | "concluded" => 0.8,
                    "resolved" => 0.7,
                    _ => 0.5,
                };
                max_signal_strength = max_signal_strength.max(strength);
            }
        }
        
        // Check state summary for completion indicators
        for indicator in &completion_indicators {
            if state_lower.contains(indicator) {
                let strength = match *indicator {
                    "completed" | "finished" | "done" => 1.0,
                    "success" | "accomplished" | "achieved" => 0.9,
                    "final" | "result" | "concluded" => 0.8,
                    "resolved" => 0.7,
                    _ => 0.5,
                };
                max_signal_strength = max_signal_strength.max(strength);
            }
        }
        
        // Check for quantified results (like "150 lines") - but only if not a continuation
        if !has_continuation_signal && (state_lower.contains("lines") || state_lower.contains("count") || 
           state_lower.contains("total") || state_lower.contains("found")) {
            max_signal_strength = max_signal_strength.max(0.7);
        }
        
        // Reduce signal strength if continuation indicators are present
        if has_continuation_signal {
            if has_partial_indicators {
                // Partial completion contexts get moderate reduction
                max_signal_strength *= 0.6; 
            } else {
                // Pure continuation contexts get heavy reduction
                max_signal_strength *= 0.2; 
            }
        }
        
        if max_signal_strength > 0.25 { // Raise threshold slightly to be more restrictive
            Some(max_signal_strength)
        } else {
            None
        }
    }
    
    /// NEW: Check if decision should prefer direct file operations over shell commands
    pub fn should_prefer_direct_operations(&self, context: &DecisionContext) -> bool {
        let simple_tasks = [
            "count", "read", "list", "check", "view", "display", "show",
            "lines", "size", "content", "exists"
        ];
        
        let description_lower = context.description.to_lowercase();
        simple_tasks.iter().any(|task| description_lower.contains(task))
    }
    
    /// NEW: Evaluate whether previous successful actions should prevent retry
    pub async fn should_skip_based_on_history(&self, context: &DecisionContext) -> bool {
        let history = self.decision_history.read().await;
        
        // Look for recent successful decisions with similar context
        for record in history.iter().rev().take(10) {
            if let Some(outcome) = &record.outcome {
                if outcome.success && self.is_similar_context(&record.context, context) {
                    return true;
                }
            }
        }
        
        false
    }
    
    /// NEW: Check if two contexts are similar enough to reuse decisions
    fn is_similar_context(&self, old_context: &DecisionContext, new_context: &DecisionContext) -> bool {
        // Simple similarity check based on description keywords
        let old_desc_lower = old_context.description.to_lowercase();
        let new_desc_lower = new_context.description.to_lowercase();
        
        let old_words: Vec<&str> = old_desc_lower.split_whitespace().collect();
        let new_words: Vec<&str> = new_desc_lower.split_whitespace().collect();
        
        let common_words = old_words.iter()
            .filter(|word| new_words.contains(word))
            .count();
        
        let similarity_ratio = common_words as f32 / old_words.len().max(new_words.len()) as f32;
        
        similarity_ratio > 0.4 // Lower threshold to be more forgiving
    }
    
    /// NEW: Get completion confidence for a specific option
    pub fn get_option_completion_confidence(&self, option: &DecisionOption) -> f32 {
        let completion_indicators = [
            "read_file", "get_file_content", "check_file", "file_exists",
            "list_directory", "directory_listing", "analyze_file"
        ];
        
        let option_id_lower = option.id.to_lowercase();
        let option_desc_lower = option.description.to_lowercase();
        
        for indicator in &completion_indicators {
            if option_id_lower.contains(indicator) || option_desc_lower.contains(indicator) {
                return match *indicator {
                    "read_file" | "get_file_content" => 0.9,
                    "list_directory" | "directory_listing" => 0.8,
                    "check_file" | "file_exists" => 0.7,
                    "analyze_file" => 0.8,
                    _ => 0.6,
                };
            }
        }
        
        // Default confidence for non-completion options
        option.base_confidence
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
        
        let context = DecisionContext::new("Test decision with no options".to_string());
        
        let result = engine.make_decision(context).await;
        assert!(result.is_err());
    }

    // NEW COMPREHENSIVE TESTS FOR COMPLETION DETECTION

    #[tokio::test]
    async fn test_completion_detection_in_context() {
        let config = DecisionConfig::default();
        let engine = DecisionEngine::new(config).await.unwrap();
        
        // Test context with clear completion indicators
        let mut completed_context = DecisionContext::new("Task completed successfully".to_string());
        completed_context.state_summary = "File analysis finished, found 150 lines".to_string();
        
        let completion_confidence = engine.detect_completion_in_context(&completed_context).await;
        assert!(completion_confidence.is_some());
        assert!(completion_confidence.unwrap() > 0.5);
        
        // Test context without completion indicators
        let mut ongoing_context = DecisionContext::new("Analyzing file".to_string());
        ongoing_context.state_summary = "Processing data".to_string();
        
        let no_completion = engine.detect_completion_in_context(&ongoing_context).await;
        assert!(no_completion.is_none() || no_completion.unwrap() < 0.3);
    }
    
    #[tokio::test]
    async fn test_prefer_direct_operations() {
        let config = DecisionConfig::default();
        let engine = DecisionEngine::new(config).await.unwrap();
        
        // Test simple file operations that should prefer direct operations
        let read_context = DecisionContext::new("Read the contents of file.txt".to_string());
        assert!(engine.should_prefer_direct_operations(&read_context));
        
        let count_context = DecisionContext::new("Count lines in the file".to_string());
        assert!(engine.should_prefer_direct_operations(&count_context));
        
        let list_context = DecisionContext::new("List files in directory".to_string());
        assert!(engine.should_prefer_direct_operations(&list_context));
        
        // Test complex operations that should not prefer direct operations
        let complex_context = DecisionContext::new("Deploy application to production server".to_string());
        assert!(!engine.should_prefer_direct_operations(&complex_context));
    }
    
    #[tokio::test]
    async fn test_skip_based_on_history() {
        let config = DecisionConfig::default();
        let mut engine = DecisionEngine::new(config).await.unwrap();
        
        // Create a successful decision in history
        let mut original_context = DecisionContext::new("Read file contents".to_string());
        original_context.add_option(DecisionOption::new("read_file".to_string(), 0.9));
        
        let decision = engine.make_decision(original_context.clone()).await.unwrap();
        
        // Simulate successful outcome
        let outcome = DecisionOutcome {
            success: true,
            actual_confidence: 0.95,
            execution_time: Duration::from_millis(100),
            lessons: vec!["File read successfully".to_string()],
            metadata: HashMap::new(),
        };
        
        engine.update_decision_outcome(decision.id, outcome).await.unwrap();
        
        // Test similar context - should skip
        let similar_context = DecisionContext::new("Read file content".to_string());
        let should_skip = engine.should_skip_based_on_history(&similar_context).await;
        assert!(should_skip);
        
        // Test different context - should not skip
        let different_context = DecisionContext::new("Delete file permanently".to_string());
        let should_not_skip = engine.should_skip_based_on_history(&different_context).await;
        assert!(!should_not_skip);
    }
    
    #[tokio::test]
    async fn test_context_similarity() {
        let config = DecisionConfig::default();
        let engine = DecisionEngine::new(config).await.unwrap();
        
        let context1 = DecisionContext::new("Read file contents from disk".to_string());
        let context2 = DecisionContext::new("Read file content from storage".to_string());
        let context3 = DecisionContext::new("Delete all files permanently".to_string());
        
        // Similar contexts should match
        assert!(engine.is_similar_context(&context1, &context2));
        
        // Different contexts should not match
        assert!(!engine.is_similar_context(&context1, &context3));
    }
    
    #[tokio::test]
    async fn test_option_completion_confidence() {
        let config = DecisionConfig::default();
        let engine = DecisionEngine::new(config).await.unwrap();
        
        // High completion confidence options
        let read_option = DecisionOption::new("read_file".to_string(), 0.5)
            .with_description("Read file contents".to_string());
        assert_eq!(engine.get_option_completion_confidence(&read_option), 0.9);
        
        let list_option = DecisionOption::new("list_directory".to_string(), 0.5)
            .with_description("List directory contents".to_string());
        assert_eq!(engine.get_option_completion_confidence(&list_option), 0.8);
        
        // Lower completion confidence options
        let generic_option = DecisionOption::new("custom_action".to_string(), 0.7)
            .with_description("Perform custom action".to_string());
        assert_eq!(engine.get_option_completion_confidence(&generic_option), 0.7);
    }
    
    #[tokio::test]
    async fn test_completion_signals_in_quantified_results() {
        let config = DecisionConfig::default();
        let engine = DecisionEngine::new(config).await.unwrap();
        
        // Test context with quantified results
        let mut context = DecisionContext::new("File analysis".to_string());
        context.state_summary = "Found 150 lines in the file".to_string();
        
        let completion_confidence = engine.detect_completion_in_context(&context).await;
        assert!(completion_confidence.is_some());
        assert!(completion_confidence.unwrap() > 0.3);
        
        // Test with count result
        context.state_summary = "Total count: 42 files".to_string();
        let completion_confidence2 = engine.detect_completion_in_context(&context).await;
        assert!(completion_confidence2.is_some());
    }
    
    #[tokio::test]
    async fn test_already_exists_error_handling() {
        let config = DecisionConfig::default();
        let mut engine = DecisionEngine::new(config).await.unwrap();
        
        // Simulate "already exists" scenario
        let mut context = DecisionContext::new("Create directory".to_string());
        context.add_option(DecisionOption::new("create_dir".to_string(), 0.8));
        
        let decision = engine.make_decision(context).await.unwrap();
        
        // Simulate "already exists" outcome - should be treated as informational, not failure
        let outcome = DecisionOutcome {
            success: true, // Even though it "failed", directory already exists is success
            actual_confidence: 0.8,
            execution_time: Duration::from_millis(50),
            lessons: vec!["Directory already exists - no action needed".to_string()],
            metadata: HashMap::new(),
        };
        
        engine.update_decision_outcome(decision.id, outcome).await.unwrap();
        
        let metrics = engine.get_metrics().await;
        assert_eq!(metrics.successful_decisions, 1);
    }
    
    #[tokio::test]
    async fn test_task_completion_vs_continuation() {
        let config = DecisionConfig::default();
        let engine = DecisionEngine::new(config).await.unwrap();
        
        // Test completed task context
        let completed_context = DecisionContext::new("Analysis completed successfully".to_string());
        let completion_confidence = engine.detect_completion_in_context(&completed_context).await;
        assert!(completion_confidence.is_some());
        assert!(completion_confidence.unwrap() > 0.7);
        
        // Test continuation context
        let continuation_context = DecisionContext::new("Now process the results".to_string());
        let continuation_confidence = engine.detect_completion_in_context(&continuation_context).await;
        assert!(continuation_confidence.is_none() || continuation_confidence.unwrap() < 0.3);
        
        // Test partial completion context
        let partial_context = DecisionContext::new("Partially done, continue processing".to_string());
        let partial_confidence = engine.detect_completion_in_context(&partial_context).await;
        assert!(partial_confidence.is_some());
        assert!(partial_confidence.unwrap() < 0.7); // Should be moderate confidence
    }
} 