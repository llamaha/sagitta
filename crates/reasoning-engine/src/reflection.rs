//! Reflection and learning engine for analyzing reasoning session performance
//!
//! This module provides sophisticated self-analysis capabilities that allow the reasoning
//! engine to learn from its experiences and improve future performance. It implements
//! pattern recognition, success/failure analysis, and adaptive strategy refinement.

use crate::error::{Result, ReasoningError};
use crate::state::{ReasoningState, ReasoningStep, StepType, SessionSummary};
use crate::orchestration::FailureCategory;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Reflection engine for analyzing and learning from reasoning sessions
#[derive(Debug)]
pub struct ReflectionEngine {
    /// Configuration for reflection behavior
    config: ReflectionConfig,
    /// Historical performance patterns
    performance_patterns: HashMap<String, PerformancePattern>,
    /// Strategy effectiveness tracking
    strategy_effectiveness: HashMap<String, StrategyMetrics>,
    /// Common failure patterns and their solutions
    failure_recovery_patterns: HashMap<FailureCategory, Vec<RecoveryPattern>>,
}

/// Configuration for reflection engine behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionConfig {
    /// Enable automatic reflection after each session
    pub auto_reflect: bool,
    /// Minimum session duration to trigger reflection (in milliseconds)
    pub min_session_duration_ms: u64,
    /// Number of historical sessions to consider for pattern analysis
    pub pattern_analysis_window: usize,
    /// Confidence threshold for recommending strategy changes
    pub strategy_change_threshold: f32,
    /// Enable learning from failure patterns
    pub enable_failure_learning: bool,
    /// Maximum number of patterns to store per category
    pub max_patterns_per_category: usize,
}

/// Performance pattern identified through analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformancePattern {
    /// Unique pattern identifier
    pub id: String,
    /// Pattern description
    pub description: String,
    /// Conditions that trigger this pattern
    pub trigger_conditions: Vec<String>,
    /// Success rate when this pattern is followed
    pub success_rate: f32,
    /// Average execution time improvement
    pub avg_time_improvement_ms: i64,
    /// Number of sessions that contributed to this pattern
    pub sample_size: u32,
    /// Recommended actions when this pattern is detected
    pub recommended_actions: Vec<String>,
    /// Confidence in this pattern (0.0 to 1.0)
    pub confidence: f32,
}

/// Metrics for strategy effectiveness
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyMetrics {
    /// Strategy identifier
    pub strategy_id: String,
    /// Number of times this strategy was used
    pub usage_count: u32,
    /// Number of successful outcomes
    pub success_count: u32,
    /// Average execution time when using this strategy
    pub avg_execution_time_ms: u64,
    /// Most common failure modes when using this strategy
    pub common_failures: HashMap<String, u32>,
    /// Recommended improvements for this strategy
    pub improvement_suggestions: Vec<String>,
}

/// Pattern for recovering from specific failure scenarios
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryPattern {
    /// Pattern identifier
    pub id: String,
    /// Failure category this pattern addresses
    pub failure_category: FailureCategory,
    /// Specific failure indicators
    pub failure_indicators: Vec<String>,
    /// Recovery steps
    pub recovery_steps: Vec<RecoveryStep>,
    /// Success rate of this recovery pattern
    pub success_rate: f32,
    /// Average time to recovery
    pub avg_recovery_time_ms: u64,
}

/// Individual recovery step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryStep {
    /// Step description
    pub description: String,
    /// Step type
    pub step_type: RecoveryStepType,
    /// Parameters for this step
    pub parameters: HashMap<String, String>,
    /// Expected outcome
    pub expected_outcome: String,
}

/// Types of recovery steps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryStepType {
    /// Retry with modified parameters
    RetryWithChanges,
    /// Switch to alternative tool or approach
    AlternativeApproach,
    /// Request user intervention
    UserIntervention,
    /// Simplify the task
    TaskSimplification,
    /// Gather additional context
    ContextGathering,
}

/// Reflection analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionAnalysis {
    /// Session that was analyzed
    pub session_id: Uuid,
    /// Overall session quality score (0.0 to 1.0)
    pub quality_score: f32,
    /// Identified strengths in the session
    pub strengths: Vec<String>,
    /// Areas for improvement
    pub improvement_areas: Vec<String>,
    /// Patterns that were successfully applied
    pub successful_patterns: Vec<String>,
    /// Failed patterns and why they failed
    pub failed_patterns: Vec<PatternFailure>,
    /// Recommendations for future sessions
    pub recommendations: Vec<Recommendation>,
    /// Performance metrics
    pub metrics: SessionMetrics,
}

/// Information about a pattern that failed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternFailure {
    /// Pattern that failed
    pub pattern_id: String,
    /// Reason for failure
    pub failure_reason: String,
    /// Suggested modifications
    pub suggested_modifications: Vec<String>,
}

/// Recommendation for improving future performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    /// Recommendation category
    pub category: RecommendationCategory,
    /// Recommendation description
    pub description: String,
    /// Expected impact (0.0 to 1.0)
    pub expected_impact: f32,
    /// Implementation difficulty (0.0 to 1.0)
    pub implementation_difficulty: f32,
    /// Priority (0.0 to 1.0)
    pub priority: f32,
}

/// Categories of recommendations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationCategory {
    /// Strategy improvements
    Strategy,
    /// Tool usage optimization
    ToolUsage,
    /// Error handling enhancement
    ErrorHandling,
    /// Performance optimization
    Performance,
    /// Communication improvement
    Communication,
}

/// Session performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetrics {
    /// Total execution time
    pub total_time_ms: u64,
    /// Number of reasoning steps
    pub step_count: u32,
    /// Number of tool executions
    pub tool_execution_count: u32,
    /// Number of retry attempts
    pub retry_count: u32,
    /// Time spent on tool execution vs reasoning
    pub tool_vs_reasoning_ratio: f32,
    /// Average confidence across steps
    pub avg_confidence: f32,
    /// Error frequency
    pub error_rate: f32,
}

impl Default for ReflectionConfig {
    fn default() -> Self {
        Self {
            auto_reflect: true,
            min_session_duration_ms: 1000, // 1 second
            pattern_analysis_window: 50,
            strategy_change_threshold: 0.7,
            enable_failure_learning: true,
            max_patterns_per_category: 20,
        }
    }
}

impl ReflectionEngine {
    /// Create a new reflection engine
    pub fn new() -> Self {
        Self::with_config(ReflectionConfig::default())
    }

    /// Create a new reflection engine with custom configuration
    pub fn with_config(config: ReflectionConfig) -> Self {
        Self {
            config,
            performance_patterns: HashMap::new(),
            strategy_effectiveness: HashMap::new(),
            failure_recovery_patterns: HashMap::new(),
        }
    }

    /// Perform reflection analysis on a completed reasoning session
    pub async fn reflect_on_session(&mut self, session: &ReasoningState) -> Result<ReflectionAnalysis> {
        // Calculate session metrics
        let metrics = self.calculate_session_metrics(session)?;
        
        // Analyze session quality
        let quality_score = self.calculate_quality_score(session, &metrics)?;
        
        // Identify successful patterns
        let successful_patterns = self.identify_successful_patterns(session)?;
        
        // Identify areas for improvement
        let improvement_areas = self.identify_improvement_areas(session, &metrics)?;
        
        // Generate recommendations
        let recommendations = self.generate_recommendations(session, &metrics, &improvement_areas)?;
        
        // Update internal knowledge
        self.update_patterns(session, &successful_patterns, &improvement_areas).await?;
        
        Ok(ReflectionAnalysis {
            session_id: session.session_id,
            quality_score,
            strengths: successful_patterns.clone(),
            improvement_areas,
            successful_patterns,
            failed_patterns: Vec::new(), // TODO: Implement pattern failure detection
            recommendations,
            metrics,
        })
    }

    /// Analyze multiple sessions to identify patterns
    pub async fn analyze_session_patterns(&mut self, sessions: &[ReasoningState]) -> Result<Vec<PerformancePattern>> {
        let mut patterns = Vec::new();
        
        // Group sessions by outcome
        let successful_sessions: Vec<_> = sessions.iter()
            .filter(|s| s.is_successful())
            .collect();
        
        let failed_sessions: Vec<_> = sessions.iter()
            .filter(|s| !s.is_successful())
            .collect();
        
        // Analyze successful patterns
        patterns.extend(self.extract_success_patterns(&successful_sessions)?);
        
        // Analyze failure patterns for learning
        if self.config.enable_failure_learning {
            patterns.extend(self.extract_failure_avoidance_patterns(&failed_sessions)?);
        }
        
        // Update internal pattern storage
        for pattern in &patterns {
            self.performance_patterns.insert(pattern.id.clone(), pattern.clone());
        }
        
        Ok(patterns)
    }

    /// Get recommendations for an ongoing session
    pub async fn get_live_recommendations(&self, current_session: &ReasoningState) -> Result<Vec<Recommendation>> {
        let mut recommendations = Vec::new();
        
        // Check for applicable patterns
        for pattern in self.performance_patterns.values() {
            if self.pattern_applies_to_session(pattern, current_session)? {
                recommendations.extend(self.pattern_to_recommendations(pattern)?);
            }
        }
        
        // Sort by priority
        recommendations.sort_by(|a, b| b.priority.partial_cmp(&a.priority).unwrap_or(std::cmp::Ordering::Equal));
        
        Ok(recommendations)
    }

    /// Update strategy effectiveness based on session outcome
    pub async fn update_strategy_effectiveness(&mut self, session: &ReasoningState) -> Result<()> {
        for strategy in &session.strategies_attempted {
            let metrics = self.strategy_effectiveness.entry(strategy.clone())
                .or_insert_with(|| StrategyMetrics {
                    strategy_id: strategy.clone(),
                    usage_count: 0,
                    success_count: 0,
                    avg_execution_time_ms: 0,
                    common_failures: HashMap::new(),
                    improvement_suggestions: Vec::new(),
                });
            
            metrics.usage_count += 1;
            
            if session.is_successful() {
                metrics.success_count += 1;
            } else {
                // Record failure reason
                if let Some(reason) = &session.completion_reason {
                    *metrics.common_failures.entry(reason.clone()).or_insert(0) += 1;
                }
            }
            
            // Update average execution time
            let session_duration = session.updated_at.signed_duration_since(session.created_at)
                .num_milliseconds() as u64;
            let total_time = metrics.avg_execution_time_ms * (metrics.usage_count as u64 - 1) + session_duration;
            metrics.avg_execution_time_ms = total_time / metrics.usage_count as u64;
        }
        
        Ok(())
    }

    // Internal helper methods

    fn calculate_session_metrics(&self, session: &ReasoningState) -> Result<SessionMetrics> {
        let total_time_ms = session.updated_at.signed_duration_since(session.created_at)
            .num_milliseconds() as u64;
        
        let step_count = session.history.len() as u32;
        
        let tool_execution_count = session.history.iter()
            .filter(|step| step.step_type == StepType::Execute)
            .count() as u32;
        
        let retry_count = session.history.iter()
            .filter(|step| step.error.is_some())
            .count() as u32;
        
        let tool_time_ms: u64 = session.history.iter()
            .filter(|step| step.step_type == StepType::Execute)
            .filter_map(|step| step.duration_ms)
            .sum();
        
        let tool_vs_reasoning_ratio = if total_time_ms > 0 {
            tool_time_ms as f32 / total_time_ms as f32
        } else {
            0.0
        };
        
        let avg_confidence = if !session.history.is_empty() {
            session.history.iter().map(|step| step.confidence).sum::<f32>() / session.history.len() as f32
        } else {
            0.0
        };
        
        let error_rate = if step_count > 0 {
            retry_count as f32 / step_count as f32
        } else {
            0.0
        };
        
        Ok(SessionMetrics {
            total_time_ms,
            step_count,
            tool_execution_count,
            retry_count,
            tool_vs_reasoning_ratio,
            avg_confidence,
            error_rate,
        })
    }

    fn calculate_quality_score(&self, session: &ReasoningState, metrics: &SessionMetrics) -> Result<f32> {
        let mut score = 0.0;
        let mut weight_sum = 0.0;
        
        // Success factor (40% of score)
        if session.is_successful() {
            score += 0.4;
        }
        weight_sum += 0.4;
        
        // Confidence factor (20% of score)
        score += metrics.avg_confidence * 0.2;
        weight_sum += 0.2;
        
        // Efficiency factor (20% of score)
        let efficiency = 1.0 - (metrics.error_rate * 0.5); // Penalize errors
        score += efficiency * 0.2;
        weight_sum += 0.2;
        
        // Speed factor (20% of score)
        let speed_bonus = if metrics.total_time_ms < 30000 { 0.2 } // Under 30s
                          else if metrics.total_time_ms < 60000 { 0.1 } // Under 1min
                          else { 0.0 };
        score += speed_bonus;
        weight_sum += 0.2;
        
        Ok(score / weight_sum)
    }

    fn identify_successful_patterns(&self, session: &ReasoningState) -> Result<Vec<String>> {
        let mut patterns = Vec::new();
        
        if session.is_successful() {
            // Pattern: Quick tool resolution
            if session.history.len() <= 3 && session.history.iter().any(|s| s.step_type == StepType::Execute) {
                patterns.push("Quick tool resolution".to_string());
            }
            
            // Pattern: High confidence throughout
            if session.history.iter().all(|s| s.confidence > 0.7) {
                patterns.push("Consistent high confidence".to_string());
            }
            
            // Pattern: No retry needed
            if session.history.iter().all(|s| s.error.is_none()) {
                patterns.push("Error-free execution".to_string());
            }
            
            // Pattern: Effective tool usage
            let tool_steps: Vec<_> = session.history.iter()
                .filter(|s| s.step_type == StepType::Execute)
                .collect();
            if !tool_steps.is_empty() && tool_steps.iter().all(|s| s.success) {
                patterns.push("Effective tool usage".to_string());
            }
        }
        
        Ok(patterns)
    }

    fn identify_improvement_areas(&self, session: &ReasoningState, metrics: &SessionMetrics) -> Result<Vec<String>> {
        let mut areas = Vec::new();
        
        // High error rate
        if metrics.error_rate > 0.3 {
            areas.push("Reduce error frequency".to_string());
        }
        
        // Low confidence
        if metrics.avg_confidence < 0.5 {
            areas.push("Improve decision confidence".to_string());
        }
        
        // Too many steps for simple tasks
        if metrics.step_count > 10 && session.context.original_request.len() < 100 {
            areas.push("Simplify execution path".to_string());
        }
        
        // Slow execution
        if metrics.total_time_ms > 60000 { // Over 1 minute
            areas.push("Optimize execution speed".to_string());
        }
        
        // Poor tool/reasoning balance
        if metrics.tool_vs_reasoning_ratio > 0.8 {
            areas.push("Balance tool usage with reasoning".to_string());
        }
        
        Ok(areas)
    }

    fn generate_recommendations(&self, session: &ReasoningState, metrics: &SessionMetrics, improvement_areas: &[String]) -> Result<Vec<Recommendation>> {
        let mut recommendations = Vec::new();
        
        for area in improvement_areas {
            match area.as_str() {
                "Reduce error frequency" => {
                    recommendations.push(Recommendation {
                        category: RecommendationCategory::ErrorHandling,
                        description: "Implement more robust error handling and validation".to_string(),
                        expected_impact: 0.7,
                        implementation_difficulty: 0.4,
                        priority: 0.8,
                    });
                }
                "Improve decision confidence" => {
                    recommendations.push(Recommendation {
                        category: RecommendationCategory::Strategy,
                        description: "Gather more context before making decisions".to_string(),
                        expected_impact: 0.6,
                        implementation_difficulty: 0.3,
                        priority: 0.7,
                    });
                }
                "Optimize execution speed" => {
                    recommendations.push(Recommendation {
                        category: RecommendationCategory::Performance,
                        description: "Cache results and use parallel execution where possible".to_string(),
                        expected_impact: 0.8,
                        implementation_difficulty: 0.6,
                        priority: 0.6,
                    });
                }
                _ => {
                    recommendations.push(Recommendation {
                        category: RecommendationCategory::Strategy,
                        description: format!("Address improvement area: {}", area),
                        expected_impact: 0.5,
                        implementation_difficulty: 0.5,
                        priority: 0.5,
                    });
                }
            }
        }
        
        Ok(recommendations)
    }

    async fn update_patterns(&mut self, session: &ReasoningState, successful_patterns: &[String], _improvement_areas: &[String]) -> Result<()> {
        // Update pattern success rates
        for pattern_name in successful_patterns {
            if let Some(pattern) = self.performance_patterns.get_mut(pattern_name) {
                pattern.sample_size += 1;
                // Recalculate success rate (simplified - in reality would need more sophisticated tracking)
                if session.is_successful() {
                    pattern.success_rate = (pattern.success_rate * (pattern.sample_size - 1) as f32 + 1.0) / pattern.sample_size as f32;
                } else {
                    pattern.success_rate = (pattern.success_rate * (pattern.sample_size - 1) as f32) / pattern.sample_size as f32;
                }
            }
        }
        
        Ok(())
    }

    fn extract_success_patterns(&self, sessions: &[&ReasoningState]) -> Result<Vec<PerformancePattern>> {
        let mut patterns = Vec::new();
        
        // Pattern: Sessions that complete quickly with few steps
        let quick_sessions: Vec<_> = sessions.iter()
            .filter(|s| s.history.len() <= 5)
            .collect();
        
        if quick_sessions.len() > 3 {
            patterns.push(PerformancePattern {
                id: "quick_completion".to_string(),
                description: "Tasks completed with minimal steps".to_string(),
                trigger_conditions: vec!["Simple request".to_string(), "Clear objective".to_string()],
                success_rate: 0.9,
                avg_time_improvement_ms: -10000, // 10s faster
                sample_size: quick_sessions.len() as u32,
                recommended_actions: vec!["Use direct tool calls".to_string(), "Avoid over-analysis".to_string()],
                confidence: 0.8,
            });
        }
        
        Ok(patterns)
    }

    fn extract_failure_avoidance_patterns(&self, _failed_sessions: &[&ReasoningState]) -> Result<Vec<PerformancePattern>> {
        // TODO: Implement failure pattern analysis
        Ok(Vec::new())
    }

    fn pattern_applies_to_session(&self, pattern: &PerformancePattern, session: &ReasoningState) -> Result<bool> {
        // Simple heuristic - check if any trigger conditions match the session
        for condition in &pattern.trigger_conditions {
            if session.context.original_request.to_lowercase().contains(&condition.to_lowercase()) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn pattern_to_recommendations(&self, pattern: &PerformancePattern) -> Result<Vec<Recommendation>> {
        let mut recommendations = Vec::new();
        
        for action in &pattern.recommended_actions {
            recommendations.push(Recommendation {
                category: RecommendationCategory::Strategy,
                description: action.clone(),
                expected_impact: pattern.success_rate * pattern.confidence,
                implementation_difficulty: 0.3, // Assume patterns are relatively easy to implement
                priority: pattern.confidence,
            });
        }
        
        Ok(recommendations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ReasoningState;
    use chrono::Utc;
    
    #[test]
    fn test_reflection_engine_creation() {
        let engine = ReflectionEngine::new();
        assert!(engine.performance_patterns.is_empty());
    }

    #[tokio::test]
    async fn test_session_metrics_calculation() {
        let mut engine = ReflectionEngine::new();
        let mut session = ReasoningState::new("Test request".to_string());
        
        // Add some history
        session.add_step(crate::state::ReasoningStep {
            id: uuid::Uuid::new_v4(),
            step_type: StepType::Execute,
            timestamp: Utc::now(),
            duration_ms: Some(1000),
            input: crate::state::StepInput::Text("test".to_string()),
            output: crate::state::StepOutput::Text("test".to_string()),
            reasoning: "test".to_string(),
            confidence: 0.8,
            success: true,
            error: None,
            tools_used: vec!["test_tool".to_string()],
            decisions_made: Vec::new(),
            knowledge_gained: std::collections::HashMap::new(),
            parent_step: None,
            child_steps: Vec::new(),
        });
        
        let metrics = engine.calculate_session_metrics(&session).unwrap();
        assert_eq!(metrics.step_count, 1);
        assert_eq!(metrics.tool_execution_count, 1);
    }

    #[tokio::test]
    async fn test_quality_score_calculation() {
        let engine = ReflectionEngine::new();
        let mut session = ReasoningState::new("Test request".to_string());
        session.set_completed(true, "Success".to_string());
        
        let metrics = SessionMetrics {
            total_time_ms: 5000,
            step_count: 3,
            tool_execution_count: 1,
            retry_count: 0,
            tool_vs_reasoning_ratio: 0.3,
            avg_confidence: 0.9,
            error_rate: 0.0,
        };
        
        let score = engine.calculate_quality_score(&session, &metrics).unwrap();
        assert!(score > 0.5); // Should be a good score
    }

    #[tokio::test]
    async fn test_successful_pattern_identification() {
        let engine = ReflectionEngine::new();
        let mut session = ReasoningState::new("Test request".to_string());
        session.set_completed(true, "Success".to_string());
        
        // Add a quick, successful execution step
        session.add_step(crate::state::ReasoningStep {
            id: uuid::Uuid::new_v4(),
            step_type: StepType::Execute,
            timestamp: Utc::now(),
            duration_ms: Some(500),
            input: crate::state::StepInput::Text("test".to_string()),
            output: crate::state::StepOutput::Text("test".to_string()),
            reasoning: "test".to_string(),
            confidence: 0.9,
            success: true,
            error: None,
            tools_used: vec!["test_tool".to_string()],
            decisions_made: Vec::new(),
            knowledge_gained: std::collections::HashMap::new(),
            parent_step: None,
            child_steps: Vec::new(),
        });
        
        let patterns = engine.identify_successful_patterns(&session).unwrap();
        assert!(!patterns.is_empty());
        assert!(patterns.contains(&"Quick tool resolution".to_string()));
        assert!(patterns.contains(&"Error-free execution".to_string()));
    }

    #[tokio::test]
    async fn test_improvement_area_identification() {
        let engine = ReflectionEngine::new();
        let session = ReasoningState::new("Test request".to_string());
        
        let metrics = SessionMetrics {
            total_time_ms: 70000, // Over 1 minute
            step_count: 15, // Many steps
            tool_execution_count: 5,
            retry_count: 3, // High error rate
            tool_vs_reasoning_ratio: 0.9, // Too tool-heavy
            avg_confidence: 0.3, // Low confidence
            error_rate: 0.2,
        };
        
        let areas = engine.identify_improvement_areas(&session, &metrics).unwrap();
        assert!(areas.contains(&"Optimize execution speed".to_string()));
        assert!(areas.contains(&"Simplify execution path".to_string()));
        assert!(areas.contains(&"Improve decision confidence".to_string()));
        assert!(areas.contains(&"Balance tool usage with reasoning".to_string()));
    }
}
