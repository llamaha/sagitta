//! Pattern recognition and matching for reasoning optimization
//!
//! This module provides sophisticated pattern recognition capabilities for identifying
//! successful reasoning patterns, failure patterns, and adaptive strategy refinement.
//! It works closely with the reflection engine to provide actionable insights.

use crate::error::{Result, ReasoningError};
use crate::state::{ReasoningState, ReasoningStep, StepType, StepInput, StepOutput};
use crate::orchestration::FailureCategory;
use crate::reflection::{PerformancePattern, RecoveryPattern, RecoveryStep};
use std::collections::{HashMap, HashSet, VecDeque};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Pattern recognition engine for identifying successful reasoning patterns
#[derive(Debug)]
pub struct PatternRecognizer {
    /// Configuration for pattern recognition
    config: PatternConfig,
    /// Known successful patterns
    success_patterns: Vec<RecognizedPattern>,
    /// Known failure patterns
    failure_patterns: Vec<FailurePattern>,
    /// Pattern matching cache for performance
    pattern_cache: HashMap<String, Vec<PatternMatch>>,
    /// Similarity threshold cache
    similarity_cache: HashMap<(String, String), f32>,
}

/// Configuration for pattern recognition behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternConfig {
    /// Minimum pattern length (number of steps)
    pub min_pattern_length: usize,
    /// Maximum pattern length to consider
    pub max_pattern_length: usize,
    /// Similarity threshold for pattern matching (0.0 to 1.0)
    pub similarity_threshold: f32,
    /// Minimum occurrences to consider a pattern valid
    pub min_occurrences: u32,
    /// Enable fuzzy pattern matching
    pub enable_fuzzy_matching: bool,
    /// Enable temporal pattern analysis
    pub enable_temporal_analysis: bool,
    /// Maximum cache size for performance optimization
    pub max_cache_size: usize,
}

/// A recognized pattern from historical sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecognizedPattern {
    /// Unique pattern identifier
    pub id: String,
    /// Pattern signature (abstract representation)
    pub signature: PatternSignature,
    /// Success rate for this pattern
    pub success_rate: f32,
    /// Number of times this pattern was observed
    pub occurrence_count: u32,
    /// Average execution time when this pattern is used
    pub avg_execution_time_ms: u64,
    /// Contexts where this pattern is most effective
    pub effective_contexts: Vec<String>,
    /// Pattern complexity score
    pub complexity_score: f32,
    /// Confidence in pattern effectiveness
    pub confidence: f32,
}

/// A failure pattern that should be avoided
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailurePattern {
    /// Unique pattern identifier
    pub id: String,
    /// Pattern signature that leads to failure
    pub signature: PatternSignature,
    /// Failure rate for this pattern
    pub failure_rate: f32,
    /// Common failure categories
    pub failure_categories: Vec<FailureCategory>,
    /// Recovery strategies for this failure pattern
    pub recovery_strategies: Vec<RecoveryPattern>,
    /// Warning signs that precede this pattern
    pub warning_signs: Vec<String>,
}

/// Abstract representation of a reasoning pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternSignature {
    /// Sequence of step types in the pattern
    pub step_sequence: Vec<StepType>,
    /// Tool usage patterns
    pub tool_patterns: Vec<ToolUsagePattern>,
    /// Decision patterns
    pub decision_patterns: Vec<DecisionPattern>,
    /// Temporal characteristics
    pub temporal_characteristics: TemporalCharacteristics,
    /// Resource usage patterns
    pub resource_patterns: Vec<ResourcePattern>,
}

/// Tool usage pattern within a reasoning sequence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUsagePattern {
    /// Tool name
    pub tool_name: String,
    /// Parameter patterns (abstracted)
    pub parameter_patterns: Vec<String>,
    /// Execution order relative to other tools
    pub execution_order: u32,
    /// Success rate for this tool in this pattern
    pub success_rate: f32,
}

/// Decision-making pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionPattern {
    /// Type of decision
    pub decision_type: String,
    /// Confidence level patterns
    pub confidence_levels: Vec<f32>,
    /// Factors that influenced the decision
    pub influencing_factors: Vec<String>,
    /// Outcome quality of decisions in this pattern
    pub outcome_quality: f32,
}

/// Temporal characteristics of a pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalCharacteristics {
    /// Total pattern duration
    pub total_duration_ms: u64,
    /// Step timing intervals
    pub step_intervals: Vec<u64>,
    /// Peak activity periods
    pub peak_periods: Vec<(u64, u64)>,
    /// Idle time patterns
    pub idle_patterns: Vec<u64>,
}

/// Resource usage pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcePattern {
    /// Resource type
    pub resource_type: String,
    /// Usage intensity (0.0 to 1.0)
    pub usage_intensity: f32,
    /// Usage duration
    pub usage_duration_ms: u64,
    /// Peak usage points
    pub peak_usage: Vec<u64>,
}

/// Result of pattern matching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternMatch {
    /// Pattern that was matched
    pub pattern_id: String,
    /// Similarity score (0.0 to 1.0)
    pub similarity: f32,
    /// Confidence in the match
    pub confidence: f32,
    /// Matched segments in the session
    pub matched_segments: Vec<PatternSegment>,
    /// Predicted outcome based on this pattern
    pub predicted_outcome: PredictedOutcome,
}

/// A segment of a session that matches part of a pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternSegment {
    /// Start step index
    pub start_step: usize,
    /// End step index
    pub end_step: usize,
    /// Similarity of this segment to the pattern
    pub segment_similarity: f32,
    /// Pattern section that was matched
    pub pattern_section: String,
}

/// Predicted outcome based on pattern matching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictedOutcome {
    /// Predicted success probability
    pub success_probability: f32,
    /// Estimated execution time
    pub estimated_time_ms: u64,
    /// Potential risks
    pub risks: Vec<String>,
    /// Recommended optimizations
    pub optimizations: Vec<String>,
}

/// Analysis result from pattern recognition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternAnalysis {
    /// Session that was analyzed
    pub session_id: Uuid,
    /// Successful patterns detected
    pub success_patterns: Vec<PatternMatch>,
    /// Failure patterns detected
    pub failure_patterns: Vec<PatternMatch>,
    /// Novel patterns (not seen before)
    pub novel_patterns: Vec<PatternSignature>,
    /// Pattern recommendations
    pub recommendations: Vec<PatternRecommendation>,
    /// Overall pattern quality score
    pub pattern_quality: f32,
}

/// Recommendation based on pattern analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternRecommendation {
    /// Type of recommendation
    pub recommendation_type: RecommendationType,
    /// Recommended pattern to follow
    pub recommended_pattern: Option<String>,
    /// Pattern to avoid
    pub pattern_to_avoid: Option<String>,
    /// Specific actions to take
    pub actions: Vec<String>,
    /// Expected improvement
    pub expected_improvement: f32,
}

/// Types of pattern-based recommendations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationType {
    /// Follow a successful pattern
    FollowSuccessPattern,
    /// Avoid a failure pattern
    AvoidFailurePattern,
    /// Optimize current approach
    OptimizeApproach,
    /// Try alternative pattern
    TryAlternative,
    /// Break down complex pattern
    DecomposePattern,
}

impl Default for PatternConfig {
    fn default() -> Self {
        Self {
            min_pattern_length: 3,
            max_pattern_length: 10,
            similarity_threshold: 0.7,
            min_occurrences: 3,
            enable_fuzzy_matching: true,
            enable_temporal_analysis: true,
            max_cache_size: 1000,
        }
    }
}

impl PatternRecognizer {
    /// Create a new pattern recognizer
    pub fn new() -> Self {
        Self::with_config(PatternConfig::default())
    }

    /// Create a pattern recognizer with custom configuration
    pub fn with_config(config: PatternConfig) -> Self {
        Self {
            config,
            success_patterns: Vec::new(),
            failure_patterns: Vec::new(),
            pattern_cache: HashMap::new(),
            similarity_cache: HashMap::new(),
        }
    }

    /// Analyze a session to identify patterns
    pub async fn analyze_session(&mut self, session: &ReasoningState) -> Result<PatternAnalysis> {
        let session_id = session.session_id;
        
        // Extract patterns from the session
        let extracted_patterns = self.extract_patterns_from_session(session).await?;
        
        // Match against known successful patterns
        let success_matches = self.match_against_success_patterns(&extracted_patterns).await?;
        
        // Match against known failure patterns
        let failure_matches = self.match_against_failure_patterns(&extracted_patterns).await?;
        
        // Identify novel patterns
        let novel_patterns = self.identify_novel_patterns(&extracted_patterns).await?;
        
        // Generate recommendations
        let recommendations = self.generate_pattern_recommendations(
            &success_matches,
            &failure_matches,
            &novel_patterns
        ).await?;
        
        // Calculate overall pattern quality
        let pattern_quality = self.calculate_pattern_quality(
            &success_matches,
            &failure_matches
        ).await?;
        
        Ok(PatternAnalysis {
            session_id,
            success_patterns: success_matches,
            failure_patterns: failure_matches,
            novel_patterns,
            recommendations,
            pattern_quality,
        })
    }

    /// Learn from a successful session by extracting and storing patterns
    pub async fn learn_from_success(&mut self, session: &ReasoningState) -> Result<()> {
        let patterns = self.extract_patterns_from_session(session).await?;
        
        for pattern_sig in patterns {
            self.incorporate_success_pattern(pattern_sig, session).await?;
        }
        
        // Cleanup old cache entries
        self.cleanup_cache().await;
        
        Ok(())
    }

    /// Learn from a failed session by identifying failure patterns
    pub async fn learn_from_failure(&mut self, session: &ReasoningState, failure_category: FailureCategory) -> Result<()> {
        let patterns = self.extract_patterns_from_session(session).await?;
        
        for pattern_sig in patterns {
            self.incorporate_failure_pattern(pattern_sig, failure_category.clone(), session).await?;
        }
        
        Ok(())
    }

    /// Get live pattern recommendations for an ongoing session
    pub async fn get_live_recommendations(&self, current_session: &ReasoningState) -> Result<Vec<PatternRecommendation>> {
        let current_patterns = self.extract_partial_patterns(current_session).await?;
        
        let mut recommendations = Vec::new();
        
        // Check for emerging failure patterns
        for pattern in &current_patterns {
            if let Some(failure_match) = self.check_emerging_failure_pattern(pattern).await? {
                recommendations.push(PatternRecommendation {
                    recommendation_type: RecommendationType::AvoidFailurePattern,
                    recommended_pattern: None,
                    pattern_to_avoid: Some(failure_match.pattern_id),
                    actions: vec![
                        "Consider alternative approach".to_string(),
                        "Review recent steps for potential issues".to_string(),
                    ],
                    expected_improvement: failure_match.confidence * 0.5,
                });
            }
        }
        
        // Suggest successful patterns to follow
        for pattern in &current_patterns {
            if let Some(success_match) = self.find_compatible_success_pattern(pattern).await? {
                recommendations.push(PatternRecommendation {
                    recommendation_type: RecommendationType::FollowSuccessPattern,
                    recommended_pattern: Some(success_match.pattern_id.clone()),
                    pattern_to_avoid: None,
                    actions: vec![
                        format!("Follow established successful pattern: {}", success_match.pattern_id),
                        "Maintain current approach with minor optimizations".to_string(),
                    ],
                    expected_improvement: success_match.confidence,
                });
            }
        }
        
        Ok(recommendations)
    }

    /// Extract patterns from a complete session
    async fn extract_patterns_from_session(&self, session: &ReasoningState) -> Result<Vec<PatternSignature>> {
        let steps = &session.history;
        let mut patterns = Vec::new();
        
        // Extract patterns of different lengths
        for length in self.config.min_pattern_length..=self.config.max_pattern_length.min(steps.len()) {
            for start in 0..=(steps.len().saturating_sub(length)) {
                let end = start + length;
                let pattern_steps = &steps[start..end];
                
                let signature = self.create_pattern_signature(pattern_steps, session).await?;
                patterns.push(signature);
            }
        }
        
        Ok(patterns)
    }

    /// Extract partial patterns from an ongoing session
    async fn extract_partial_patterns(&self, session: &ReasoningState) -> Result<Vec<PatternSignature>> {
        let steps = &session.history;
        let mut patterns = Vec::new();
        
        // Look at recent patterns only
        let recent_length = self.config.max_pattern_length.min(steps.len());
        if recent_length >= self.config.min_pattern_length {
            let start = steps.len().saturating_sub(recent_length);
            let recent_steps = &steps[start..];
            
            let signature = self.create_pattern_signature(recent_steps, session).await?;
            patterns.push(signature);
        }
        
        Ok(patterns)
    }

    /// Create a pattern signature from a sequence of steps
    async fn create_pattern_signature(&self, steps: &[ReasoningStep], session: &ReasoningState) -> Result<PatternSignature> {
        let step_sequence = steps.iter().map(|s| s.step_type.clone()).collect();
        
        let tool_patterns = self.extract_tool_patterns(steps).await?;
        let decision_patterns = self.extract_decision_patterns(steps).await?;
        let temporal_characteristics = self.extract_temporal_characteristics(steps).await?;
        let resource_patterns = self.extract_resource_patterns(steps, session).await?;
        
        Ok(PatternSignature {
            step_sequence,
            tool_patterns,
            decision_patterns,
            temporal_characteristics,
            resource_patterns,
        })
    }

    /// Extract tool usage patterns from steps
    async fn extract_tool_patterns(&self, steps: &[ReasoningStep]) -> Result<Vec<ToolUsagePattern>> {
        let mut tool_patterns = Vec::new();
        let mut tool_order = HashMap::new();
        let mut order_counter = 0u32;
        
        for step in steps {
            // Extract tool names from tools_used field
            for tool_name in &step.tools_used {
                let entry = tool_order.entry(tool_name.clone()).or_insert(order_counter);
                if *entry == order_counter {
                    order_counter += 1;
                }
                
                // Extract parameters from input
                let parameter_patterns = match &step.input {
                    StepInput::ToolExecution { tool: _, args } => {
                        self.abstract_parameters(args).await?
                    },
                    StepInput::Data(data) => {
                        self.abstract_parameters(data).await?
                    },
                    _ => vec![]
                };
                
                tool_patterns.push(ToolUsagePattern {
                    tool_name: tool_name.clone(),
                    parameter_patterns,
                    execution_order: *entry,
                    success_rate: if step.success { 1.0 } else { 0.0 },
                });
            }
        }
        
        Ok(tool_patterns)
    }

    /// Abstract parameters to create reusable patterns
    async fn abstract_parameters(&self, parameters: &serde_json::Value) -> Result<Vec<String>> {
        let mut patterns = Vec::new();
        
        if let Some(obj) = parameters.as_object() {
            for (key, value) in obj {
                let pattern = match value {
                    serde_json::Value::String(s) => {
                        if s.len() > 50 {
                            format!("{}:long_string", key)
                        } else if s.chars().all(|c| c.is_ascii_digit()) {
                            format!("{}:numeric_string", key)
                        } else {
                            format!("{}:string", key)
                        }
                    },
                    serde_json::Value::Number(_) => format!("{}:number", key),
                    serde_json::Value::Bool(_) => format!("{}:boolean", key),
                    serde_json::Value::Array(_) => format!("{}:array", key),
                    serde_json::Value::Object(_) => format!("{}:object", key),
                    serde_json::Value::Null => format!("{}:null", key),
                };
                patterns.push(pattern);
            }
        }
        
        Ok(patterns)
    }

    /// Extract decision patterns from steps
    async fn extract_decision_patterns(&self, steps: &[ReasoningStep]) -> Result<Vec<DecisionPattern>> {
        let mut decision_patterns = Vec::new();
        
        for step in steps {
            if step.step_type == StepType::Decide {
                let confidence_levels = vec![step.confidence];
                
                decision_patterns.push(DecisionPattern {
                    decision_type: step.reasoning.clone(),
                    confidence_levels,
                    influencing_factors: self.extract_influencing_factors(step).await?,
                    outcome_quality: step.confidence,
                });
            }
        }
        
        Ok(decision_patterns)
    }

    /// Extract factors that influenced a decision
    async fn extract_influencing_factors(&self, step: &ReasoningStep) -> Result<Vec<String>> {
        let mut factors = Vec::new();
        
        // Add basic factors based on step characteristics
        if step.confidence > 0.8 {
            factors.push("high_confidence".to_string());
        } else if step.confidence < 0.3 {
            factors.push("low_confidence".to_string());
        }
        
        // Check complexity of input parameters
        let input_complexity = match &step.input {
            StepInput::Data(data) => data.as_object().map_or(0, |o| o.len()),
            StepInput::ToolExecution { tool: _, args } => args.as_object().map_or(0, |o| o.len()),
            _ => 0
        };
        
        if input_complexity > 5 {
            factors.push("complex_parameters".to_string());
        }
        
        // Check if multiple tools were used
        if step.tools_used.len() > 1 {
            factors.push("multiple_tools".to_string());
        }
        
        Ok(factors)
    }

    /// Extract temporal characteristics from steps
    async fn extract_temporal_characteristics(&self, steps: &[ReasoningStep]) -> Result<TemporalCharacteristics> {
        let mut step_intervals = Vec::new();
        let mut total_duration = 0u64;
        
        // Calculate intervals between steps
        for window in steps.windows(2) {
            let interval = window[1].timestamp.signed_duration_since(window[0].timestamp)
                .num_milliseconds().max(0) as u64;
            step_intervals.push(interval);
            total_duration += interval;
        }
        
        // Add individual step durations
        for step in steps {
            if let Some(duration_ms) = step.duration_ms {
                total_duration += duration_ms;
            }
        }
        
        Ok(TemporalCharacteristics {
            total_duration_ms: total_duration,
            step_intervals,
            peak_periods: vec![], // TODO: Implement peak period detection
            idle_patterns: vec![], // TODO: Implement idle pattern detection
        })
    }

    /// Extract resource usage patterns
    async fn extract_resource_patterns(&self, _steps: &[ReasoningStep], _session: &ReasoningState) -> Result<Vec<ResourcePattern>> {
        // TODO: Implement resource pattern extraction
        // This would require integration with resource monitoring
        Ok(vec![])
    }

    /// Match patterns against known successful patterns
    async fn match_against_success_patterns(&self, patterns: &[PatternSignature]) -> Result<Vec<PatternMatch>> {
        let mut matches = Vec::new();
        
        for pattern in patterns {
            for success_pattern in &self.success_patterns {
                let similarity = self.calculate_pattern_similarity(pattern, &success_pattern.signature).await?;
                
                if similarity >= self.config.similarity_threshold {
                    let pattern_match = PatternMatch {
                        pattern_id: success_pattern.id.clone(),
                        similarity,
                        confidence: similarity * success_pattern.confidence,
                        matched_segments: vec![], // TODO: Implement segment matching
                        predicted_outcome: PredictedOutcome {
                            success_probability: success_pattern.success_rate,
                            estimated_time_ms: success_pattern.avg_execution_time_ms,
                            risks: vec![],
                            optimizations: vec!["Follow established successful pattern".to_string()],
                        },
                    };
                    matches.push(pattern_match);
                }
            }
        }
        
        Ok(matches)
    }

    /// Match patterns against known failure patterns
    async fn match_against_failure_patterns(&self, patterns: &[PatternSignature]) -> Result<Vec<PatternMatch>> {
        let mut matches = Vec::new();
        
        for pattern in patterns {
            for failure_pattern in &self.failure_patterns {
                let similarity = self.calculate_pattern_similarity(pattern, &failure_pattern.signature).await?;
                
                if similarity >= self.config.similarity_threshold {
                    let pattern_match = PatternMatch {
                        pattern_id: failure_pattern.id.clone(),
                        similarity,
                        confidence: similarity * (1.0 - failure_pattern.failure_rate),
                        matched_segments: vec![], // TODO: Implement segment matching
                        predicted_outcome: PredictedOutcome {
                            success_probability: 1.0 - failure_pattern.failure_rate,
                            estimated_time_ms: 0, // Unknown for failure patterns
                            risks: failure_pattern.warning_signs.clone(),
                            optimizations: vec!["Avoid this failure pattern".to_string()],
                        },
                    };
                    matches.push(pattern_match);
                }
            }
        }
        
        Ok(matches)
    }

    /// Calculate similarity between two pattern signatures
    async fn calculate_pattern_similarity(&self, pattern1: &PatternSignature, pattern2: &PatternSignature) -> Result<f32> {
        // Create cache key
        let key = (
            format!("{:?}", pattern1.step_sequence),
            format!("{:?}", pattern2.step_sequence)
        );
        
        // Check cache first
        if let Some(&cached_similarity) = self.similarity_cache.get(&key) {
            return Ok(cached_similarity);
        }
        
        let step_similarity = self.calculate_step_sequence_similarity(&pattern1.step_sequence, &pattern2.step_sequence).await?;
        let tool_similarity = self.calculate_tool_pattern_similarity(&pattern1.tool_patterns, &pattern2.tool_patterns).await?;
        let temporal_similarity = self.calculate_temporal_similarity(&pattern1.temporal_characteristics, &pattern2.temporal_characteristics).await?;
        
        // Weighted combination of similarities
        let overall_similarity = 0.5 * step_similarity + 0.3 * tool_similarity + 0.2 * temporal_similarity;
        
        Ok(overall_similarity)
    }

    /// Calculate similarity between step sequences using edit distance
    async fn calculate_step_sequence_similarity(&self, seq1: &[StepType], seq2: &[StepType]) -> Result<f32> {
        if seq1.is_empty() && seq2.is_empty() {
            return Ok(1.0);
        }
        
        if seq1.is_empty() || seq2.is_empty() {
            return Ok(0.0);
        }
        
        let edit_distance = self.levenshtein_distance(seq1, seq2);
        let max_len = seq1.len().max(seq2.len());
        
        let similarity = 1.0 - (edit_distance as f32 / max_len as f32);
        Ok(similarity.max(0.0))
    }

    /// Calculate Levenshtein distance between two sequences
    fn levenshtein_distance<T: PartialEq>(&self, seq1: &[T], seq2: &[T]) -> usize {
        let len1 = seq1.len();
        let len2 = seq2.len();
        
        if len1 == 0 {
            return len2;
        }
        if len2 == 0 {
            return len1;
        }
        
        let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];
        
        for i in 0..=len1 {
            matrix[i][0] = i;
        }
        for j in 0..=len2 {
            matrix[0][j] = j;
        }
        
        for i in 1..=len1 {
            for j in 1..=len2 {
                let cost = if seq1[i - 1] == seq2[j - 1] { 0 } else { 1 };
                
                matrix[i][j] = (matrix[i - 1][j] + 1)
                    .min(matrix[i][j - 1] + 1)
                    .min(matrix[i - 1][j - 1] + cost);
            }
        }
        
        matrix[len1][len2]
    }

    /// Calculate similarity between tool patterns
    async fn calculate_tool_pattern_similarity(&self, patterns1: &[ToolUsagePattern], patterns2: &[ToolUsagePattern]) -> Result<f32> {
        if patterns1.is_empty() && patterns2.is_empty() {
            return Ok(1.0);
        }
        
        if patterns1.is_empty() || patterns2.is_empty() {
            return Ok(0.0);
        }
        
        let tool_names1: HashSet<_> = patterns1.iter().map(|p| &p.tool_name).collect();
        let tool_names2: HashSet<_> = patterns2.iter().map(|p| &p.tool_name).collect();
        
        let intersection = tool_names1.intersection(&tool_names2).count();
        let union = tool_names1.union(&tool_names2).count();
        
        if union == 0 {
            Ok(0.0)
        } else {
            Ok(intersection as f32 / union as f32)
        }
    }

    /// Calculate similarity between temporal characteristics
    async fn calculate_temporal_similarity(&self, temporal1: &TemporalCharacteristics, temporal2: &TemporalCharacteristics) -> Result<f32> {
        // Simple duration-based similarity for now
        let duration_diff = (temporal1.total_duration_ms as i64 - temporal2.total_duration_ms as i64).abs();
        let max_duration = temporal1.total_duration_ms.max(temporal2.total_duration_ms);
        
        if max_duration == 0 {
            Ok(1.0)
        } else {
            let similarity = 1.0 - (duration_diff as f32 / max_duration as f32);
            Ok(similarity.max(0.0))
        }
    }

    /// Identify novel patterns that haven't been seen before
    async fn identify_novel_patterns(&self, patterns: &[PatternSignature]) -> Result<Vec<PatternSignature>> {
        let mut novel_patterns = Vec::new();
        
        for pattern in patterns {
            let mut is_novel = true;
            
            // Check against success patterns
            for success_pattern in &self.success_patterns {
                let similarity = self.calculate_pattern_similarity(pattern, &success_pattern.signature).await?;
                if similarity >= self.config.similarity_threshold {
                    is_novel = false;
                    break;
                }
            }
            
            // Check against failure patterns
            if is_novel {
                for failure_pattern in &self.failure_patterns {
                    let similarity = self.calculate_pattern_similarity(pattern, &failure_pattern.signature).await?;
                    if similarity >= self.config.similarity_threshold {
                        is_novel = false;
                        break;
                    }
                }
            }
            
            if is_novel {
                novel_patterns.push(pattern.clone());
            }
        }
        
        Ok(novel_patterns)
    }

    /// Generate pattern-based recommendations
    async fn generate_pattern_recommendations(
        &self,
        success_matches: &[PatternMatch],
        failure_matches: &[PatternMatch],
        _novel_patterns: &[PatternSignature],
    ) -> Result<Vec<PatternRecommendation>> {
        let mut recommendations = Vec::new();
        
        // Recommend following successful patterns
        for success_match in success_matches {
            if success_match.confidence > 0.7 {
                recommendations.push(PatternRecommendation {
                    recommendation_type: RecommendationType::FollowSuccessPattern,
                    recommended_pattern: Some(success_match.pattern_id.clone()),
                    pattern_to_avoid: None,
                    actions: vec![
                        "Continue following this successful pattern".to_string(),
                        "Monitor execution for continued success".to_string(),
                    ],
                    expected_improvement: success_match.confidence,
                });
            }
        }
        
        // Recommend avoiding failure patterns
        for failure_match in failure_matches {
            if failure_match.confidence > 0.6 {
                recommendations.push(PatternRecommendation {
                    recommendation_type: RecommendationType::AvoidFailurePattern,
                    recommended_pattern: None,
                    pattern_to_avoid: Some(failure_match.pattern_id.clone()),
                    actions: vec![
                        "Avoid this failure pattern".to_string(),
                        "Consider alternative approaches".to_string(),
                    ],
                    expected_improvement: failure_match.confidence * 0.8,
                });
            }
        }
        
        Ok(recommendations)
    }

    /// Calculate overall pattern quality for a session
    async fn calculate_pattern_quality(&self, success_matches: &[PatternMatch], failure_matches: &[PatternMatch]) -> Result<f32> {
        let success_score: f32 = success_matches.iter().map(|m| m.confidence).sum();
        let failure_penalty: f32 = failure_matches.iter().map(|m| m.confidence * 0.5).sum();
        
        let quality = (success_score - failure_penalty).max(0.0).min(1.0);
        Ok(quality)
    }

    /// Incorporate a new success pattern
    async fn incorporate_success_pattern(&mut self, signature: PatternSignature, _session: &ReasoningState) -> Result<()> {
        let pattern_id = format!("success_{}", uuid::Uuid::new_v4());
        
        let pattern = RecognizedPattern {
            id: pattern_id,
            signature,
            success_rate: 1.0, // Initial rate, will be updated
            occurrence_count: 1,
            avg_execution_time_ms: 1000, // TODO: Calculate from session
            effective_contexts: vec!["general".to_string()],
            complexity_score: 0.5, // TODO: Calculate complexity
            confidence: 0.8,
        };
        
        self.success_patterns.push(pattern);
        Ok(())
    }

    /// Incorporate a new failure pattern
    async fn incorporate_failure_pattern(&mut self, signature: PatternSignature, failure_category: FailureCategory, _session: &ReasoningState) -> Result<()> {
        let pattern_id = format!("failure_{}", uuid::Uuid::new_v4());
        
        let pattern = FailurePattern {
            id: pattern_id,
            signature,
            failure_rate: 1.0, // Initial rate, will be updated
            failure_categories: vec![failure_category],
            recovery_strategies: vec![], // TODO: Generate recovery strategies
            warning_signs: vec!["pattern_detected".to_string()],
        };
        
        self.failure_patterns.push(pattern);
        Ok(())
    }

    /// Check if current pattern might lead to failure
    async fn check_emerging_failure_pattern(&self, pattern: &PatternSignature) -> Result<Option<PatternMatch>> {
        for failure_pattern in &self.failure_patterns {
            let similarity = self.calculate_pattern_similarity(pattern, &failure_pattern.signature).await?;
            
            if similarity >= self.config.similarity_threshold * 0.8 { // Lower threshold for early warning
                return Ok(Some(PatternMatch {
                    pattern_id: failure_pattern.id.clone(),
                    similarity,
                    confidence: similarity * failure_pattern.failure_rate,
                    matched_segments: vec![],
                    predicted_outcome: PredictedOutcome {
                        success_probability: 1.0 - failure_pattern.failure_rate,
                        estimated_time_ms: 0,
                        risks: failure_pattern.warning_signs.clone(),
                        optimizations: vec!["Consider alternative approach".to_string()],
                    },
                }));
            }
        }
        
        Ok(None)
    }

    /// Find a compatible success pattern to follow
    async fn find_compatible_success_pattern(&self, pattern: &PatternSignature) -> Result<Option<PatternMatch>> {
        for success_pattern in &self.success_patterns {
            let similarity = self.calculate_pattern_similarity(pattern, &success_pattern.signature).await?;
            
            if similarity >= self.config.similarity_threshold * 0.6 { // Lower threshold for suggestions
                return Ok(Some(PatternMatch {
                    pattern_id: success_pattern.id.clone(),
                    similarity,
                    confidence: similarity * success_pattern.confidence,
                    matched_segments: vec![],
                    predicted_outcome: PredictedOutcome {
                        success_probability: success_pattern.success_rate,
                        estimated_time_ms: success_pattern.avg_execution_time_ms,
                        risks: vec![],
                        optimizations: vec!["Follow this successful pattern".to_string()],
                    },
                }));
            }
        }
        
        Ok(None)
    }

    /// Clean up old cache entries to maintain performance
    async fn cleanup_cache(&mut self) {
        if self.pattern_cache.len() > self.config.max_cache_size {
            self.pattern_cache.clear();
        }
        
        if self.similarity_cache.len() > self.config.max_cache_size {
            self.similarity_cache.clear();
        }
    }

    /// Get statistics about recognized patterns
    pub fn get_pattern_statistics(&self) -> PatternStatistics {
        PatternStatistics {
            total_success_patterns: self.success_patterns.len(),
            total_failure_patterns: self.failure_patterns.len(),
            cache_hit_rate: 0.0, // TODO: Implement cache hit tracking
            average_pattern_complexity: self.success_patterns.iter()
                .map(|p| p.complexity_score)
                .sum::<f32>() / self.success_patterns.len().max(1) as f32,
        }
    }
}

/// Statistics about pattern recognition performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternStatistics {
    /// Number of success patterns learned
    pub total_success_patterns: usize,
    /// Number of failure patterns learned
    pub total_failure_patterns: usize,
    /// Cache hit rate for performance optimization
    pub cache_hit_rate: f32,
    /// Average complexity of learned patterns
    pub average_pattern_complexity: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{ReasoningStep, ConversationPhase, ReasoningContext, ConversationContext, SessionMetadata, ToolExecutionState, task_completion::TaskCompletionAnalyzer};
    use std::time::{SystemTime, UNIX_EPOCH};
    
    fn create_test_step(step_type: StepType, tool_names: Vec<String>) -> ReasoningStep {
        ReasoningStep {
            id: uuid::Uuid::new_v4(),
            step_type,
            timestamp: chrono::Utc::now(),
            duration_ms: Some(100),
            input: StepInput::Data(serde_json::json!({"test": "value"})),
            output: StepOutput::Data(serde_json::json!({"result": "success"})),
            reasoning: "test step".to_string(),
            confidence: 0.8,
            success: true,
            error: None,
            tools_used: tool_names,
            decisions_made: Vec::new(),
            knowledge_gained: std::collections::HashMap::new(),
            parent_step: None,
            child_steps: Vec::new(),
        }
    }
    
    fn create_test_reasoning_state() -> ReasoningState {
        let context = ReasoningContext::new("test request".to_string());
        ReasoningState {
            session_id: uuid::Uuid::new_v4(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            context,
            history: Vec::new(),
            current_goal: None,
            sub_goals: std::collections::VecDeque::new(),
            completed_goals: Vec::new(),
            iteration_count: 0,
            confidence_score: 0.5,
            overall_progress: 0.0,
            decision_points: Vec::new(),
            checkpoints: Vec::new(),
            current_checkpoint: None,
            patterns_used: Vec::new(),
            strategies_attempted: Vec::new(),
            success_indicators: std::collections::HashMap::new(),
            streaming_state: crate::state::StreamingState::default(),
            mode: crate::state::ReasoningMode::Autonomous,
            metadata: std::collections::HashMap::new(),
            completion_reason: None,
            is_final_success_status: None,
            conversation_context: ConversationContext::default(),
            session_metadata: SessionMetadata::default(),
            tool_execution_state: ToolExecutionState::default(),
            current_task_completion: None,
            last_analyzed_content: None,
            content_analysis_cache: std::collections::HashMap::new(),
            task_completion_analyzer: TaskCompletionAnalyzer::default(),
        }
    }
    
    #[tokio::test]
    async fn test_pattern_recognizer_creation() {
        let recognizer = PatternRecognizer::new();
        assert_eq!(recognizer.success_patterns.len(), 0);
        assert_eq!(recognizer.failure_patterns.len(), 0);
    }
    
    #[tokio::test]
    async fn test_pattern_signature_creation() {
        let recognizer = PatternRecognizer::new();
        
        let steps = vec![
            create_test_step(StepType::Analyze, vec!["test_tool".to_string()]),
            create_test_step(StepType::Decide, vec![]),
            create_test_step(StepType::Execute, vec!["another_tool".to_string()]),
        ];
        
        let session = create_test_reasoning_state();
        let signature = recognizer.create_pattern_signature(&steps, &session).await;
        
        assert!(signature.is_ok());
        let sig = signature.unwrap();
        assert_eq!(sig.step_sequence.len(), 3);
        assert_eq!(sig.step_sequence[0], StepType::Analyze);
        assert_eq!(sig.step_sequence[1], StepType::Decide);
        assert_eq!(sig.step_sequence[2], StepType::Execute);
    }
    
    #[tokio::test]
    async fn test_step_sequence_similarity() {
        let recognizer = PatternRecognizer::new();
        
        let seq1 = vec![StepType::Analyze, StepType::Decide, StepType::Execute];
        let seq2 = vec![StepType::Analyze, StepType::Decide, StepType::Execute];
        let seq3 = vec![StepType::Analyze, StepType::Execute];
        
        let sim1 = recognizer.calculate_step_sequence_similarity(&seq1, &seq2).await.unwrap();
        let sim2 = recognizer.calculate_step_sequence_similarity(&seq1, &seq3).await.unwrap();
        
        assert!((sim1 - 1.0).abs() < 0.001); // Identical sequences
        assert!(sim2 < sim1); // Different sequences have lower similarity
    }
    
    #[tokio::test]
    async fn test_levenshtein_distance() {
        let recognizer = PatternRecognizer::new();
        
        let seq1 = vec![1, 2, 3];
        let seq2 = vec![1, 2, 3];
        let seq3 = vec![1, 3];
        
        assert_eq!(recognizer.levenshtein_distance(&seq1, &seq2), 0);
        assert_eq!(recognizer.levenshtein_distance(&seq1, &seq3), 1);
    }
    
    #[tokio::test]
    async fn test_tool_pattern_similarity() {
        let recognizer = PatternRecognizer::new();
        
        let patterns1 = vec![
            ToolUsagePattern {
                tool_name: "tool1".to_string(),
                parameter_patterns: vec!["param:string".to_string()],
                execution_order: 0,
                success_rate: 1.0,
            },
            ToolUsagePattern {
                tool_name: "tool2".to_string(),
                parameter_patterns: vec!["param:number".to_string()],
                execution_order: 1,
                success_rate: 1.0,
            },
        ];
        
        let patterns2 = vec![
            ToolUsagePattern {
                tool_name: "tool1".to_string(),
                parameter_patterns: vec!["param:string".to_string()],
                execution_order: 0,
                success_rate: 1.0,
            },
        ];
        
        let similarity = recognizer.calculate_tool_pattern_similarity(&patterns1, &patterns2).await.unwrap();
        assert!(similarity > 0.0 && similarity < 1.0);
    }
}
