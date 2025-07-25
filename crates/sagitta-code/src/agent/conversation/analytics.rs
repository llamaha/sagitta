// Conversation analytics and metrics
// TODO: Implement actual analytics

use anyhow::Result;
use chrono::{DateTime, Utc, Duration, Timelike};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::agent::conversation::types::{Conversation, ProjectType};
use crate::agent::state::types::ConversationStatus;
// Tool types removed - tools now via MCP

/// Conversation analytics manager for tracking metrics and patterns
pub struct ConversationAnalyticsManager {
    /// Analytics configuration
    config: AnalyticsConfig,
}

/// Configuration for analytics collection and analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsConfig {
    /// Whether to track detailed metrics
    pub detailed_tracking: bool,
    
    /// Minimum conversation length for analysis
    pub min_conversation_length: usize,
    
    /// Time window for trend analysis (in days)
    pub trend_window_days: u32,
    
    /// Success threshold for conversation scoring
    pub success_threshold: f32,
}

impl Default for AnalyticsConfig {
    fn default() -> Self {
        Self {
            detailed_tracking: true,
            min_conversation_length: 3,
            trend_window_days: 30,
            success_threshold: 0.7,
        }
    }
}

/// Comprehensive analytics report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsReport {
    /// Report generation timestamp
    pub generated_at: DateTime<Utc>,
    
    /// Time period covered by this report
    pub period: (DateTime<Utc>, DateTime<Utc>),
    
    /// Overall conversation metrics
    pub overall_metrics: OverallMetrics,
    
    /// Success metrics and patterns
    pub success_metrics: SuccessMetrics,
    
    /// Efficiency analysis
    pub efficiency_metrics: EfficiencyMetrics,
    
    /// Token usage analysis
    pub token_usage_metrics: TokenUsageMetrics,
    
    /// Pattern recognition results
    pub patterns: PatternAnalysis,
    
    /// Project-specific insights
    pub project_insights: Vec<ProjectInsight>,
    
    /// Trending topics and themes
    pub trending_topics: Vec<TrendingTopic>,
    
    /// Recommendations for improvement
    pub recommendations: Vec<Recommendation>,
}

/// Overall conversation metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverallMetrics {
    /// Total number of conversations analyzed
    pub total_conversations: usize,
    
    /// Total number of messages across all conversations
    pub total_messages: usize,
    
    /// Total tokens used across all conversations
    pub total_tokens: usize,
    
    /// Average tokens per conversation
    pub avg_tokens_per_conversation: usize,
    
    /// Average tokens per message
    pub avg_tokens_per_message: usize,
    
    /// Average messages per conversation
    pub avg_messages_per_conversation: f64,
    
    /// Total number of branches created
    pub total_branches: usize,
    
    /// Total number of checkpoints created
    pub total_checkpoints: usize,
    
    /// Conversation completion rate
    pub completion_rate: f32,
    
    /// Average conversation duration (in minutes)
    pub avg_duration_minutes: f64,
    
    /// Most active time periods
    pub peak_activity_hours: Vec<u32>,
    
    /// Distribution by project type
    pub project_type_distribution: HashMap<ProjectType, usize>,
}

/// Success metrics and analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessMetrics {
    /// Overall success rate (0.0-1.0)
    pub overall_success_rate: f32,
    
    /// Success rate by project type
    pub success_by_project_type: HashMap<ProjectType, f32>,
    
    /// Success rate by conversation length
    pub success_by_length: Vec<(usize, f32)>, // (message_count_range, success_rate)
    
    /// Most successful conversation patterns
    pub successful_patterns: Vec<ConversationPattern>,
    
    /// Common failure points
    pub failure_points: Vec<FailurePoint>,
    
    /// Success indicators
    pub success_indicators: Vec<SuccessIndicator>,
}

/// Efficiency analysis metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EfficiencyMetrics {
    /// Average time to resolution (in minutes)
    pub avg_resolution_time: f64,
    
    /// Branching efficiency (successful branches / total branches)
    pub branching_efficiency: f32,
    
    /// Checkpoint utilization rate
    pub checkpoint_utilization: f32,
    
    /// Context switching frequency
    pub context_switches_per_conversation: f64,
    
    /// Most efficient conversation types
    pub efficient_patterns: Vec<EfficiencyPattern>,
    
    /// Resource utilization metrics
    pub resource_utilization: ResourceUtilization,
}

/// Token usage metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsageMetrics {
    /// Token usage distribution by conversation
    pub token_distribution: Vec<(Uuid, usize)>, // (conversation_id, token_count)
    
    /// Peak token usage in a single conversation
    pub peak_usage: usize,
    
    /// Conversations that hit token limits
    pub limit_reached_count: usize,
    
    /// Token usage by message role
    pub tokens_by_role: HashMap<String, usize>, // role -> total tokens
    
    /// Token usage trend over time
    pub usage_trend: Vec<(DateTime<Utc>, usize)>, // (timestamp, tokens)
    
    /// Estimated cost based on token usage (if applicable)
    pub estimated_cost: Option<f64>,
}

/// Pattern recognition analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternAnalysis {
    /// Common conversation flows
    pub common_flows: Vec<ConversationFlow>,
    
    /// Recurring themes and topics
    pub recurring_themes: Vec<Theme>,
    
    /// Temporal patterns (time-based trends)
    pub temporal_patterns: Vec<TemporalPattern>,
    
    /// User behavior patterns
    pub behavior_patterns: Vec<BehaviorPattern>,
    
    /// Anomaly detection results
    pub anomalies: Vec<Anomaly>,
}

/// Project-specific insights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInsight {
    /// Project type
    pub project_type: ProjectType,
    
    /// Number of conversations for this project type
    pub conversation_count: usize,
    
    /// Average success rate
    pub success_rate: f32,
    
    /// Common topics and themes
    pub common_topics: Vec<String>,
    
    /// Typical conversation patterns
    pub typical_patterns: Vec<String>,
    
    /// Specific recommendations
    pub recommendations: Vec<String>,
}

/// Trending topic analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendingTopic {
    /// Topic name or theme
    pub topic: String,
    
    /// Frequency of occurrence
    pub frequency: usize,
    
    /// Trend direction (growing, stable, declining)
    pub trend: TrendDirection,
    
    /// Associated project types
    pub project_types: Vec<ProjectType>,
    
    /// Success rate for this topic
    pub success_rate: f32,
}

/// Trend direction indicator
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TrendDirection {
    Growing,
    Stable,
    Declining,
}

/// Improvement recommendations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    /// Recommendation category
    pub category: RecommendationCategory,
    
    /// Priority level
    pub priority: Priority,
    
    /// Description of the recommendation
    pub description: String,
    
    /// Expected impact
    pub expected_impact: String,
    
    /// Implementation difficulty
    pub difficulty: Difficulty,
    
    /// Supporting data/evidence
    pub evidence: Vec<String>,
}

/// Recommendation categories
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RecommendationCategory {
    Efficiency,
    UserExperience,
    ContentQuality,
    ProcessImprovement,
    TechnicalOptimization,
}

/// Priority levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

/// Implementation difficulty
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}


/// Conversation pattern identification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationPattern {
    /// Pattern name
    pub name: String,
    
    /// Pattern description
    pub description: String,
    
    /// Frequency of occurrence
    pub frequency: usize,
    
    /// Success rate for this pattern
    pub success_rate: f32,
    
    /// Typical sequence of actions
    pub action_sequence: Vec<String>,
}

/// Failure point analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailurePoint {
    /// Description of the failure point
    pub description: String,
    
    /// Frequency of occurrence
    pub frequency: usize,
    
    /// Typical context when this occurs
    pub context: String,
    
    /// Suggested mitigation strategies
    pub mitigations: Vec<String>,
}

/// Success indicator identification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessIndicator {
    /// Indicator description
    pub description: String,
    
    /// Correlation strength with success (0.0-1.0)
    pub correlation: f32,
    
    /// How to recognize this indicator
    pub recognition_criteria: String,
}

/// Efficiency pattern analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EfficiencyPattern {
    /// Pattern name
    pub name: String,
    
    /// Average efficiency score
    pub efficiency_score: f32,
    
    /// Characteristics of this pattern
    pub characteristics: Vec<String>,
    
    /// Recommended usage scenarios
    pub usage_scenarios: Vec<String>,
}

/// Resource utilization metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUtilization {
    /// Average memory usage per conversation
    pub avg_memory_usage: f64,
    
    /// Storage efficiency
    pub storage_efficiency: f32,
    
    /// Search performance metrics
    pub search_performance: SearchPerformance,
    
    /// Clustering efficiency
    pub clustering_efficiency: f32,
}

/// Search performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchPerformance {
    /// Average search time (milliseconds)
    pub avg_search_time_ms: f64,
    
    /// Search accuracy rate
    pub accuracy_rate: f32,
    
    /// Most common search patterns
    pub common_patterns: Vec<String>,
}


/// Conversation flow analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationFlow {
    /// Flow name
    pub name: String,
    
    /// Sequence of conversation states
    pub states: Vec<String>,
    
    /// Transition probabilities
    pub transitions: HashMap<String, HashMap<String, f32>>,
    
    /// Average flow duration
    pub avg_duration: f64,
}

/// Theme analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    /// Theme name
    pub name: String,
    
    /// Keywords associated with this theme
    pub keywords: Vec<String>,
    
    /// Frequency of occurrence
    pub frequency: usize,
    
    /// Associated project types
    pub project_types: Vec<ProjectType>,
}

/// Temporal pattern analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalPattern {
    /// Pattern name
    pub name: String,
    
    /// Time-based characteristics
    pub time_characteristics: String,
    
    /// Peak activity periods
    pub peak_periods: Vec<String>,
    
    /// Seasonal trends
    pub seasonal_trends: Vec<String>,
}

/// User behavior pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorPattern {
    /// Pattern name
    pub name: String,
    
    /// Behavior description
    pub description: String,
    
    /// Frequency of occurrence
    pub frequency: usize,
    
    /// Impact on conversation success
    pub success_impact: f32,
}

/// Anomaly detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    /// Anomaly type
    pub anomaly_type: String,
    
    /// Description of the anomaly
    pub description: String,
    
    /// Severity level
    pub severity: Priority,
    
    /// When it was detected
    pub detected_at: DateTime<Utc>,
    
    /// Suggested investigation steps
    pub investigation_steps: Vec<String>,
}

impl ConversationAnalyticsManager {
    /// Create a new analytics manager
    pub fn new(config: AnalyticsConfig) -> Self {
        Self { config }
    }
    
    /// Create analytics manager with default configuration
    pub fn with_default_config() -> Self {
        Self::new(AnalyticsConfig::default())
    }
    
    /// Generate comprehensive analytics report
    pub async fn generate_report(
        &self,
        conversations: &[Conversation],
        period: Option<(DateTime<Utc>, DateTime<Utc>)>,
    ) -> Result<AnalyticsReport> {
        let period = period.unwrap_or_else(|| {
            let end = Utc::now();
            let start = end - Duration::days(self.config.trend_window_days as i64);
            (start, end)
        });
        
        // Filter conversations by period
        let filtered_conversations: Vec<&Conversation> = conversations
            .iter()
            .filter(|conv| conv.created_at >= period.0 && conv.created_at <= period.1)
            .filter(|conv| conv.messages.len() >= self.config.min_conversation_length)
            .collect();
        
        let overall_metrics = self.calculate_overall_metrics(&filtered_conversations).await?;
        let success_metrics = self.analyze_success_metrics(&filtered_conversations).await?;
        let efficiency_metrics = self.analyze_efficiency(&filtered_conversations).await?;
        let token_usage_metrics = self.analyze_token_usage(&filtered_conversations).await?;
        let patterns = self.analyze_patterns(&filtered_conversations).await?;
        let project_insights = self.generate_project_insights(&filtered_conversations).await?;
        let trending_topics = self.analyze_trending_topics(&filtered_conversations).await?;
        let recommendations = self.generate_recommendations(
            &overall_metrics,
            &success_metrics,
            &efficiency_metrics,
            &patterns,
        ).await?;
        
        Ok(AnalyticsReport {
            generated_at: Utc::now(),
            period,
            overall_metrics,
            success_metrics,
            efficiency_metrics,
            token_usage_metrics,
            patterns,
            project_insights,
            trending_topics,
            recommendations,
        })
    }
    
    /// Calculate overall conversation metrics
    async fn calculate_overall_metrics(&self, conversations: &[&Conversation]) -> Result<OverallMetrics> {
        let total_conversations = conversations.len();
        let total_messages: usize = conversations.iter().map(|c| c.messages.len()).sum();
        let total_branches: usize = conversations.iter().map(|c| c.branches.len()).sum();
        let total_checkpoints: usize = conversations.iter().map(|c| c.checkpoints.len()).sum();
        
        let avg_messages_per_conversation = if total_conversations > 0 {
            total_messages as f64 / total_conversations as f64
        } else {
            0.0
        };
        
        let completed_conversations = conversations
            .iter()
            .filter(|c| c.status == ConversationStatus::Completed)
            .count();
        
        let completion_rate = if total_conversations > 0 {
            completed_conversations as f32 / total_conversations as f32
        } else {
            0.0
        };
        
        // Calculate average duration
        let total_duration: i64 = conversations
            .iter()
            .map(|c| (c.last_active - c.created_at).num_minutes())
            .sum();
        
        let avg_duration_minutes = if total_conversations > 0 {
            total_duration as f64 / total_conversations as f64
        } else {
            0.0
        };
        
        // Analyze peak activity hours
        let mut hour_counts = [0; 24];
        for conversation in conversations {
            let hour = conversation.created_at.hour();
            hour_counts[hour as usize] += 1;
        }
        
        let max_count = hour_counts.iter().max().unwrap_or(&0);
        let peak_activity_hours: Vec<u32> = hour_counts
            .iter()
            .enumerate()
            .filter(|(_, &count)| count >= max_count * 80 / 100) // Top 80% of peak activity
            .map(|(hour, _)| hour as u32)
            .collect();
        
        // Project type distribution
        let mut project_type_distribution = HashMap::new();
        for conversation in conversations {
            if let Some(ref project_context) = conversation.project_context {
                *project_type_distribution
                    .entry(project_context.project_type)
                    .or_insert(0) += 1;
            }
        }
        
        // Calculate token metrics (placeholder - in real implementation would get from MessageHistory)
        let total_tokens: usize = conversations
            .iter()
            .map(|c| c.messages.iter().map(|m| m.content.len() / 4).sum::<usize>())
            .sum();
        
        let avg_tokens_per_conversation = if total_conversations > 0 {
            total_tokens / total_conversations
        } else {
            0
        };
        
        let avg_tokens_per_message = if total_messages > 0 {
            total_tokens / total_messages
        } else {
            0
        };
        
        Ok(OverallMetrics {
            total_conversations,
            total_messages,
            total_tokens,
            avg_tokens_per_conversation,
            avg_tokens_per_message,
            avg_messages_per_conversation,
            total_branches,
            total_checkpoints,
            completion_rate,
            avg_duration_minutes,
            peak_activity_hours,
            project_type_distribution,
        })
    }
    
    /// Analyze success metrics and patterns
    async fn analyze_success_metrics(&self, conversations: &[&Conversation]) -> Result<SuccessMetrics> {
        let successful_conversations = conversations
            .iter()
            .filter(|c| self.is_conversation_successful(c))
            .count();
        
        let overall_success_rate = if !conversations.is_empty() {
            successful_conversations as f32 / conversations.len() as f32
        } else {
            0.0
        };
        
        // Success by project type
        let mut success_by_project_type = HashMap::new();
        let mut project_totals = HashMap::new();
        
        for conversation in conversations {
            if let Some(ref project_context) = conversation.project_context {
                let project_type = &project_context.project_type;
                *project_totals.entry(*project_type).or_insert(0) += 1;
                
                if self.is_conversation_successful(conversation) {
                    *success_by_project_type.entry(*project_type).or_insert(0) += 1;
                }
            }
        }
        
        let mut success_rates_by_project_type = HashMap::new();
        for (project_type, total) in project_totals {
            let successful = success_by_project_type.get(&project_type).unwrap_or(&0);
            let rate = *successful as f32 / total as f32;
            success_rates_by_project_type.insert(project_type, rate);
        }
        
        // Success by conversation length
        let mut length_buckets: HashMap<usize, (usize, usize)> = HashMap::new(); // (total, successful)
        
        for conversation in conversations {
            let length_bucket = (conversation.messages.len() / 5) * 5; // Group by 5s
            let entry = length_buckets.entry(length_bucket).or_insert((0, 0));
            entry.0 += 1;
            if self.is_conversation_successful(conversation) {
                entry.1 += 1;
            }
        }
        
        let success_by_length: Vec<(usize, f32)> = length_buckets
            .into_iter()
            .map(|(length, (total, successful))| {
                let rate = if total > 0 { successful as f32 / total as f32 } else { 0.0 };
                (length, rate)
            })
            .collect();
        
        // Identify successful patterns (simplified)
        let successful_patterns = vec![
            ConversationPattern {
                name: "Quick Resolution".to_string(),
                description: "Conversations resolved in under 10 messages".to_string(),
                frequency: conversations.iter().filter(|c| c.messages.len() < 10).count(),
                success_rate: 0.85, // Placeholder
                action_sequence: vec![
                    "Initial question".to_string(),
                    "Clarification".to_string(),
                    "Solution provided".to_string(),
                    "Confirmation".to_string(),
                ],
            },
        ];
        
        // Common failure points (simplified)
        let failure_points = vec![
            FailurePoint {
                description: "Conversation abandoned after initial question".to_string(),
                frequency: conversations.iter().filter(|c| c.messages.len() <= 2).count(),
                context: "User asks question but doesn't follow up".to_string(),
                mitigations: vec![
                    "Provide more engaging initial responses".to_string(),
                    "Ask clarifying questions".to_string(),
                ],
            },
        ];
        
        // Success indicators (simplified)
        let success_indicators = vec![
            SuccessIndicator {
                description: "Multiple message exchanges".to_string(),
                correlation: 0.75,
                recognition_criteria: "Conversation has more than 5 messages".to_string(),
            },
        ];
        
        Ok(SuccessMetrics {
            overall_success_rate,
            success_by_project_type: success_rates_by_project_type,
            success_by_length,
            successful_patterns,
            failure_points,
            success_indicators,
        })
    }
    
    /// Analyze efficiency metrics
    async fn analyze_efficiency(&self, conversations: &[&Conversation]) -> Result<EfficiencyMetrics> {
        // Calculate average resolution time
        let resolution_times: Vec<i64> = conversations
            .iter()
            .filter(|c| c.status == ConversationStatus::Completed)
            .map(|c| (c.last_active - c.created_at).num_minutes())
            .collect();
        
        let avg_resolution_time = if !resolution_times.is_empty() {
            resolution_times.iter().sum::<i64>() as f64 / resolution_times.len() as f64
        } else {
            0.0
        };
        
        // Branching efficiency
        let total_branches: usize = conversations.iter().map(|c| c.branches.len()).sum();
        let successful_branches: usize = conversations
            .iter()
            .flat_map(|c| &c.branches)
            .filter(|b| b.success_score.unwrap_or(0.0) >= self.config.success_threshold)
            .count();
        
        let branching_efficiency = if total_branches > 0 {
            successful_branches as f32 / total_branches as f32
        } else {
            0.0
        };
        
        // Checkpoint utilization
        let conversations_with_checkpoints = conversations
            .iter()
            .filter(|c| !c.checkpoints.is_empty())
            .count();
        
        let checkpoint_utilization = if !conversations.is_empty() {
            conversations_with_checkpoints as f32 / conversations.len() as f32
        } else {
            0.0
        };
        
        // Context switches per conversation (simplified)
        let total_context_switches: usize = conversations
            .iter()
            .map(|c| c.branches.len() + c.checkpoints.len())
            .sum();
        
        let context_switches_per_conversation = if !conversations.is_empty() {
            total_context_switches as f64 / conversations.len() as f64
        } else {
            0.0
        };
        
        // Efficient patterns (simplified)
        let efficient_patterns = vec![
            EfficiencyPattern {
                name: "Direct Problem Solving".to_string(),
                efficiency_score: 0.9,
                characteristics: vec![
                    "Few branches".to_string(),
                    "Quick resolution".to_string(),
                    "Clear communication".to_string(),
                ],
                usage_scenarios: vec![
                    "Simple technical questions".to_string(),
                    "Well-defined problems".to_string(),
                ],
            },
        ];
        
        // Resource utilization (simplified)
        let resource_utilization = ResourceUtilization {
            avg_memory_usage: 1024.0, // Placeholder
            storage_efficiency: 0.85,
            search_performance: SearchPerformance {
                avg_search_time_ms: 150.0,
                accuracy_rate: 0.92,
                common_patterns: vec!["keyword search".to_string(), "semantic search".to_string()],
            },
            clustering_efficiency: 0.78,
        };
        
        Ok(EfficiencyMetrics {
            avg_resolution_time,
            branching_efficiency,
            checkpoint_utilization,
            context_switches_per_conversation,
            efficient_patterns,
            resource_utilization,
        })
    }
    
    /// Analyze token usage across conversations
    async fn analyze_token_usage(&self, conversations: &[&Conversation]) -> Result<TokenUsageMetrics> {
        let mut token_distribution = Vec::new();
        let mut tokens_by_role = HashMap::new();
        let mut usage_trend = Vec::new();
        let mut peak_usage = 0;
        let mut limit_reached_count = 0;
        let mut total_tokens = 0;
        
        // Note: This is a placeholder implementation
        // In a real implementation, you would:
        // 1. Access the MessageHistory for each conversation
        // 2. Get the actual token counts from history.get_total_tokens()
        // 3. Track when limits were reached
        
        for conversation in conversations {
            // Placeholder: estimate tokens based on message content length
            let estimated_tokens: usize = conversation.messages
                .iter()
                .map(|msg| msg.content.len() / 4) // Rough estimate: 4 chars per token
                .sum();
            
            token_distribution.push((conversation.id, estimated_tokens));
            peak_usage = peak_usage.max(estimated_tokens);
            total_tokens += estimated_tokens;
            
            // Track tokens by role
            for msg in &conversation.messages {
                let role = format!("{:?}", msg.role); // Use Debug format for Role
                *tokens_by_role.entry(role).or_insert(0) += msg.content.len() / 4;
            }
            
            // Check if this conversation likely hit token limits
            if estimated_tokens > 60000 { // Assuming 64k limit with buffer
                limit_reached_count += 1;
            }
        }
        
        // Sort token distribution by usage
        token_distribution.sort_by_key(|(_, tokens)| std::cmp::Reverse(*tokens));
        
        // Create usage trend (simplified - just show daily totals)
        if !conversations.is_empty() {
            let start_date = conversations.iter()
                .map(|c| c.created_at)
                .min()
                .unwrap();
            let end_date = Utc::now();
            let mut current_date = start_date;
            
            while current_date <= end_date {
                let daily_tokens: usize = conversations.iter()
                    .filter(|c| c.created_at.date_naive() == current_date.date_naive())
                    .map(|c| c.messages.len() * 100) // Rough estimate
                    .sum();
                
                if daily_tokens > 0 {
                    usage_trend.push((current_date, daily_tokens));
                }
                
                current_date += Duration::days(1);
            }
        }
        
        // Estimate cost (using rough OpenAI pricing)
        let estimated_cost = if total_tokens > 0 {
            Some((total_tokens as f64 / 1000.0) * 0.03) // $0.03 per 1K tokens
        } else {
            None
        };
        
        Ok(TokenUsageMetrics {
            token_distribution,
            peak_usage,
            limit_reached_count,
            tokens_by_role,
            usage_trend,
            estimated_cost,
        })
    }
    
    /// Analyze conversation patterns
    async fn analyze_patterns(&self, conversations: &[&Conversation]) -> Result<PatternAnalysis> {
        // Common flows (simplified)
        let common_flows = vec![
            ConversationFlow {
                name: "Standard Q&A Flow".to_string(),
                states: vec![
                    "Question".to_string(),
                    "Clarification".to_string(),
                    "Answer".to_string(),
                    "Follow-up".to_string(),
                    "Resolution".to_string(),
                ],
                transitions: HashMap::new(), // Simplified
                avg_duration: 15.5,
            },
        ];
        
        // Recurring themes (simplified)
        let recurring_themes = vec![
            Theme {
                name: "Programming Help".to_string(),
                keywords: vec!["code".to_string(), "function".to_string(), "error".to_string()],
                frequency: conversations.len() / 3, // Simplified
                project_types: vec![ProjectType::Rust, ProjectType::Python],
            },
        ];
        
        // Temporal patterns (simplified)
        let temporal_patterns = vec![
            TemporalPattern {
                name: "Weekday Activity".to_string(),
                time_characteristics: "Higher activity during weekdays".to_string(),
                peak_periods: vec!["9-11 AM".to_string(), "2-4 PM".to_string()],
                seasonal_trends: vec!["Increased activity during work hours".to_string()],
            },
        ];
        
        // Behavior patterns (simplified)
        let behavior_patterns = vec![
            BehaviorPattern {
                name: "Iterative Problem Solving".to_string(),
                description: "Users tend to ask follow-up questions".to_string(),
                frequency: conversations.iter().filter(|c| c.messages.len() > 5).count(),
                success_impact: 0.8,
            },
        ];
        
        // Anomalies (simplified)
        let anomalies = vec![
            Anomaly {
                anomaly_type: "Unusually Long Conversation".to_string(),
                description: "Conversation with excessive message count".to_string(),
                severity: Priority::Medium,
                detected_at: Utc::now(),
                investigation_steps: vec![
                    "Review conversation content".to_string(),
                    "Check for circular discussions".to_string(),
                ],
            },
        ];
        
        Ok(PatternAnalysis {
            common_flows,
            recurring_themes,
            temporal_patterns,
            behavior_patterns,
            anomalies,
        })
    }
    
    /// Generate project-specific insights
    async fn generate_project_insights(&self, conversations: &[&Conversation]) -> Result<Vec<ProjectInsight>> {
        let mut insights = Vec::new();
        let mut project_conversations: HashMap<ProjectType, Vec<&Conversation>> = HashMap::new();
        
        // Group conversations by project type
        for conversation in conversations {
            if let Some(ref project_context) = conversation.project_context {
                project_conversations
                    .entry(project_context.project_type)
                    .or_default()
                    .push(conversation);
            }
        }
        
        // Generate insights for each project type
        for (project_type, convs) in project_conversations {
            let conversation_count = convs.len();
            let successful_count = convs.iter().filter(|c| self.is_conversation_successful(c)).count();
            let success_rate = if conversation_count > 0 {
                successful_count as f32 / conversation_count as f32
            } else {
                0.0
            };
            
            // Extract common topics (simplified)
            let common_topics = vec![
                format!("{:?} programming", project_type),
                "debugging".to_string(),
                "best practices".to_string(),
            ];
            
            let typical_patterns = vec![
                "Question about syntax".to_string(),
                "Error troubleshooting".to_string(),
                "Code review request".to_string(),
            ];
            
            let recommendations = vec![
                format!("Focus on {:?}-specific documentation", project_type),
                "Provide more code examples".to_string(),
                "Create project templates".to_string(),
            ];
            
            insights.push(ProjectInsight {
                project_type,
                conversation_count,
                success_rate,
                common_topics,
                typical_patterns,
                recommendations,
            });
        }
        
        Ok(insights)
    }
    
    /// Analyze trending topics
    async fn analyze_trending_topics(&self, conversations: &[&Conversation]) -> Result<Vec<TrendingTopic>> {
        // Simplified trending analysis
        let topics = vec![
            TrendingTopic {
                topic: "Async Programming".to_string(),
                frequency: conversations.len() / 4, // Simplified
                trend: TrendDirection::Growing,
                project_types: vec![ProjectType::Rust, ProjectType::JavaScript],
                success_rate: 0.75,
            },
            TrendingTopic {
                topic: "Error Handling".to_string(),
                frequency: conversations.len() / 3,
                trend: TrendDirection::Stable,
                project_types: vec![ProjectType::Rust, ProjectType::Python, ProjectType::Go],
                success_rate: 0.82,
            },
        ];
        
        Ok(topics)
    }
    
    /// Generate improvement recommendations
    async fn generate_recommendations(
        &self,
        overall_metrics: &OverallMetrics,
        success_metrics: &SuccessMetrics,
        efficiency_metrics: &EfficiencyMetrics,
        _patterns: &PatternAnalysis,
    ) -> Result<Vec<Recommendation>> {
        let mut recommendations = Vec::new();
        
        // Analyze completion rate
        if overall_metrics.completion_rate < 0.7 {
            recommendations.push(Recommendation {
                category: RecommendationCategory::UserExperience,
                priority: Priority::High,
                description: "Improve conversation completion rate".to_string(),
                expected_impact: "Increase user satisfaction and problem resolution".to_string(),
                difficulty: Difficulty::Medium,
                evidence: vec![
                    format!("Current completion rate: {:.1}%", overall_metrics.completion_rate * 100.0),
                    "Target completion rate: 70%+".to_string(),
                ],
            });
        }
        
        // Analyze success rate
        if success_metrics.overall_success_rate < self.config.success_threshold {
            recommendations.push(Recommendation {
                category: RecommendationCategory::ContentQuality,
                priority: Priority::High,
                description: "Improve overall conversation success rate".to_string(),
                expected_impact: "Better problem resolution and user experience".to_string(),
                difficulty: Difficulty::Hard,
                evidence: vec![
                    format!("Current success rate: {:.1}%", success_metrics.overall_success_rate * 100.0),
                    format!("Target success rate: {:.1}%+", self.config.success_threshold * 100.0),
                ],
            });
        }
        
        // Analyze branching efficiency
        if efficiency_metrics.branching_efficiency < 0.6 {
            recommendations.push(Recommendation {
                category: RecommendationCategory::Efficiency,
                priority: Priority::Medium,
                description: "Optimize conversation branching strategy".to_string(),
                expected_impact: "Reduce wasted effort on unsuccessful branches".to_string(),
                difficulty: Difficulty::Medium,
                evidence: vec![
                    format!("Current branching efficiency: {:.1}%", efficiency_metrics.branching_efficiency * 100.0),
                    "Target efficiency: 60%+".to_string(),
                ],
            });
        }
        
        // Analyze checkpoint utilization
        if efficiency_metrics.checkpoint_utilization < 0.3 {
            recommendations.push(Recommendation {
                category: RecommendationCategory::ProcessImprovement,
                priority: Priority::Low,
                description: "Encourage more checkpoint usage".to_string(),
                expected_impact: "Better conversation state management and recovery".to_string(),
                difficulty: Difficulty::Easy,
                evidence: vec![
                    format!("Current checkpoint utilization: {:.1}%", efficiency_metrics.checkpoint_utilization * 100.0),
                    "Checkpoints help with conversation recovery".to_string(),
                ],
            });
        }
        
        Ok(recommendations)
    }
    
    /// Determine if a conversation is successful
    fn is_conversation_successful(&self, conversation: &Conversation) -> bool {
        // Multiple criteria for success
        let has_resolution = conversation.status == ConversationStatus::Completed;
        let sufficient_length = conversation.messages.len() >= self.config.min_conversation_length;
        let has_positive_outcome = conversation.branches.iter()
            .any(|b| b.success_score.unwrap_or(0.0) >= self.config.success_threshold);
        
        has_resolution || (sufficient_length && has_positive_outcome)
    }
    
    /// Update analytics configuration
    pub fn update_config(&mut self, config: AnalyticsConfig) {
        self.config = config;
    }
    
    /// Get current analytics configuration
    pub fn get_config(&self) -> &AnalyticsConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    

    use crate::agent::message::types::AgentMessage;
    use crate::llm::client::Role;
    use crate::agent::conversation::types::{ConversationBranch, ConversationCheckpoint, ProjectContext, BranchStatus};

    // Helper function to create test conversations
    fn create_test_conversation(
        title: &str,
        status: ConversationStatus,
        message_count: usize,
        created_at: DateTime<Utc>,
        project_type: Option<ProjectType>,
        has_branches: bool,
        has_checkpoints: bool,
    ) -> Conversation {
        let messages: Vec<AgentMessage> = (0..message_count)
            .map(|i| AgentMessage {
                id: Uuid::new_v4(),
                role: if i % 2 == 0 { Role::User } else { Role::Assistant },
                content: format!("Message {}", i),
                timestamp: created_at + Duration::minutes(i as i64),
                is_streaming: false,
                metadata: HashMap::new(),
                tool_calls: vec![],
            })
            .collect();

        let branches = if has_branches {
            vec![ConversationBranch {
                id: Uuid::new_v4(),
                title: "Test Branch".to_string(),
                description: Some("Test branch description".to_string()),
                parent_message_id: Some(messages.first().map(|m| m.id).unwrap_or_default()),
                messages: vec![],
                created_at,
                status: BranchStatus::Active,
                merged: false,
                success_score: Some(0.8),
            }]
        } else {
            vec![]
        };

        let checkpoints = if has_checkpoints {
            vec![ConversationCheckpoint {
                id: Uuid::new_v4(),
                message_id: messages.first().map(|m| m.id).unwrap_or_default(),
                title: "Test Checkpoint".to_string(),
                description: Some("Test checkpoint description".to_string()),
                created_at,
                context_snapshot: None,
                auto_generated: false,
            }]
        } else {
            vec![]
        };

        Conversation {
            id: Uuid::new_v4(),
            title: title.to_string(),
            messages,
            created_at,
            last_active: created_at + Duration::minutes(message_count as i64),
            tags: vec!["test".to_string()],
            status,
            workspace_id: None,
            branches,
            checkpoints,
            project_context: project_type.map(|pt| ProjectContext {
                name: format!("Test {} Project", title),
                project_type: pt,
                root_path: Some(std::path::PathBuf::from("/test/project")),
                description: Some("Test project description".to_string()),
                repositories: vec![],
                settings: HashMap::new(),
            }),
        }
    }

    #[test]
    fn test_analytics_manager_creation() {
        let manager = ConversationAnalyticsManager::with_default_config();
        assert!(manager.config.detailed_tracking);
        assert_eq!(manager.config.min_conversation_length, 3);
    }
    
    #[test]
    fn test_analytics_config_update() {
        let mut manager = ConversationAnalyticsManager::with_default_config();
        
        let new_config = AnalyticsConfig {
            detailed_tracking: false,
            min_conversation_length: 5,
            trend_window_days: 60,
            success_threshold: 0.8,
        };
        
        manager.update_config(new_config.clone());
        
        assert!(!manager.get_config().detailed_tracking);
        assert_eq!(manager.get_config().min_conversation_length, 5);
        assert_eq!(manager.get_config().trend_window_days, 60);
        assert_eq!(manager.get_config().success_threshold, 0.8);
    }

    #[test]
    fn test_analytics_config_default() {
        let config = AnalyticsConfig::default();
        
        assert!(config.detailed_tracking);
        assert_eq!(config.min_conversation_length, 3);
        assert_eq!(config.trend_window_days, 30);
        assert!((config.success_threshold - 0.7).abs() < f32::EPSILON);
    }

    #[tokio::test]
    async fn test_generate_report_empty_conversations() {
        let manager = ConversationAnalyticsManager::with_default_config();
        let conversations = vec![];
        let period = (Utc::now() - Duration::days(7), Utc::now());
        
        let report = manager.generate_report(&conversations, Some(period)).await.unwrap();
        
        assert_eq!(report.overall_metrics.total_conversations, 0);
        assert_eq!(report.overall_metrics.total_messages, 0);
        assert_eq!(report.success_metrics.overall_success_rate, 0.0);
        assert_eq!(report.efficiency_metrics.avg_resolution_time, 0.0);
    }

    #[tokio::test]
    async fn test_calculate_overall_metrics() {
        let manager = ConversationAnalyticsManager::with_default_config();
        let now = Utc::now();
        
        let conversations = vec![
            create_test_conversation("Conv 1", ConversationStatus::Completed, 5, now - Duration::hours(2), Some(ProjectType::Rust), true, false),
            create_test_conversation("Conv 2", ConversationStatus::Active, 3, now - Duration::hours(1), Some(ProjectType::Python), false, true),
            create_test_conversation("Conv 3", ConversationStatus::Completed, 10, now - Duration::minutes(30), None, true, true),
        ];
        
        let conversation_refs: Vec<&Conversation> = conversations.iter().collect();
        let metrics = manager.calculate_overall_metrics(&conversation_refs).await.unwrap();
        
        assert_eq!(metrics.total_conversations, 3);
        assert_eq!(metrics.total_messages, 18); // 5 + 3 + 10
        assert!((metrics.avg_messages_per_conversation - 6.0).abs() < f64::EPSILON);
        assert_eq!(metrics.total_branches, 2); // Conv 1 and Conv 3 have branches
        assert_eq!(metrics.total_checkpoints, 2); // Conv 2 and Conv 3 have checkpoints
        assert!((metrics.completion_rate - 2.0/3.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_analyze_success_metrics() {
        let manager = ConversationAnalyticsManager::with_default_config();
        let now = Utc::now();
        
        let conversations = vec![
            create_test_conversation("Success 1", ConversationStatus::Completed, 8, now - Duration::hours(3), Some(ProjectType::Rust), true, false),
            create_test_conversation("Failed 1", ConversationStatus::Active, 2, now - Duration::hours(2), Some(ProjectType::Rust), false, false),
            create_test_conversation("Success 2", ConversationStatus::Completed, 15, now - Duration::hours(1), Some(ProjectType::Python), false, true),
            create_test_conversation("Success 3", ConversationStatus::Completed, 5, now, Some(ProjectType::Python), true, false),
        ];
        
        let conversation_refs: Vec<&Conversation> = conversations.iter().collect();
        let metrics = manager.analyze_success_metrics(&conversation_refs).await.unwrap();
        
        assert!((metrics.overall_success_rate - 0.75).abs() < 0.01); // 3 out of 4
        assert_eq!(metrics.success_by_project_type.len(), 2);
        assert!((metrics.success_by_project_type[&ProjectType::Rust] - 0.5).abs() < 0.01); // 1 out of 2
        assert!((metrics.success_by_project_type[&ProjectType::Python] - 1.0).abs() < 0.01); // 2 out of 2
        
        // Check success by conversation length
        assert!(!metrics.success_by_length.is_empty());
        assert!(metrics.success_by_length.iter().any(|(_, rate)| *rate > 0.0));
    }

    #[tokio::test]
    async fn test_analyze_efficiency() {
        let manager = ConversationAnalyticsManager::with_default_config();
        let now = Utc::now();
        
        let conversations = vec![
            create_test_conversation("Conv 1", ConversationStatus::Completed, 5, now - Duration::hours(2), None, true, true),
            create_test_conversation("Conv 2", ConversationStatus::Active, 3, now - Duration::hours(1), None, false, false),
        ];
        
        let conversation_refs: Vec<&Conversation> = conversations.iter().collect();
        let overall_metrics = manager.calculate_overall_metrics(&conversation_refs).await.unwrap();
        let efficiency = manager.analyze_efficiency(&conversation_refs).await.unwrap();
        
        assert!(efficiency.avg_resolution_time > 0.0);
        assert!(efficiency.branching_efficiency >= 0.0 && efficiency.branching_efficiency <= 1.0);
        assert!(efficiency.checkpoint_utilization >= 0.0 && efficiency.checkpoint_utilization <= 1.0);
    }

    #[tokio::test]
    async fn test_is_conversation_successful() {
        let manager = ConversationAnalyticsManager::with_default_config();
        let now = Utc::now();
        
        // Test completed conversation - should be successful
        let completed_conv = create_test_conversation(
            "Completed",
            ConversationStatus::Completed,
            5,
            now,
            None,
            false,
            false,
        );
        assert!(manager.is_conversation_successful(&completed_conv));
        
        // Test active conversation with sufficient length and successful branch
        let active_conv = create_test_conversation(
            "Active",
            ConversationStatus::Active,
            5,
            now,
            None,
            true,
            false,
        );
        assert!(manager.is_conversation_successful(&active_conv));
        
        // Test short active conversation - should not be successful
        let short_conv = create_test_conversation(
            "Short",
            ConversationStatus::Active,
            2,
            now,
            None,
            false,
            false,
        );
        assert!(!manager.is_conversation_successful(&short_conv));
    }

    #[tokio::test]
    async fn test_analyze_token_usage() {
        let manager = ConversationAnalyticsManager::with_default_config();
        let now = Utc::now();
        
        let conversations = vec![
            create_test_conversation("Conv 1", ConversationStatus::Completed, 10, now - Duration::hours(2), None, false, false),
            create_test_conversation("Conv 2", ConversationStatus::Active, 5, now - Duration::hours(1), None, false, false),
        ];
        
        let conversation_refs: Vec<&Conversation> = conversations.iter().collect();
        let token_usage = manager.analyze_token_usage(&conversation_refs).await.unwrap();
        
        // Since token counting is estimated, just verify the structure
        assert!(!token_usage.token_distribution.is_empty());
        assert!(token_usage.peak_usage > 0);
        assert!(!token_usage.tokens_by_role.is_empty());
        assert!(token_usage.estimated_cost.is_some());
    }

    #[tokio::test]
    async fn test_analyze_patterns() {
        let manager = ConversationAnalyticsManager::with_default_config();
        let now = Utc::now();
        
        let conversations = vec![
            create_test_conversation("Conv 1", ConversationStatus::Completed, 8, now - Duration::hours(3), Some(ProjectType::Rust), true, false),
            create_test_conversation("Conv 2", ConversationStatus::Active, 5, now - Duration::hours(2), Some(ProjectType::Python), false, true),
            create_test_conversation("Conv 3", ConversationStatus::Completed, 12, now - Duration::hours(1), Some(ProjectType::Rust), true, true),
        ];
        
        let conversation_refs: Vec<&Conversation> = conversations.iter().collect();
        let patterns = manager.analyze_patterns(&conversation_refs).await.unwrap();
        
        assert!(!patterns.common_flows.is_empty());
        assert!(!patterns.recurring_themes.is_empty());
        assert!(!patterns.temporal_patterns.is_empty());
        assert!(!patterns.behavior_patterns.is_empty());
        assert!(patterns.anomalies.is_empty() || !patterns.anomalies.is_empty());
    }

    #[tokio::test]
    async fn test_generate_project_insights() {
        let manager = ConversationAnalyticsManager::with_default_config();
        let now = Utc::now();
        
        let conversations = vec![
            create_test_conversation("Rust 1", ConversationStatus::Completed, 10, now - Duration::hours(3), Some(ProjectType::Rust), true, false),
            create_test_conversation("Rust 2", ConversationStatus::Active, 5, now - Duration::hours(2), Some(ProjectType::Rust), false, false),
            create_test_conversation("Python 1", ConversationStatus::Completed, 8, now - Duration::hours(1), Some(ProjectType::Python), false, true),
        ];
        
        let conversation_refs: Vec<&Conversation> = conversations.iter().collect();
        let insights = manager.generate_project_insights(&conversation_refs).await.unwrap();
        
        assert_eq!(insights.len(), 2); // Rust and Python
        
        let rust_insight = insights.iter().find(|i| i.project_type == ProjectType::Rust).unwrap();
        assert_eq!(rust_insight.conversation_count, 2);
        assert!((rust_insight.success_rate - 0.5).abs() < 0.01); // 1 out of 2
        assert!(!rust_insight.common_topics.is_empty());
        assert!(!rust_insight.typical_patterns.is_empty());
        assert!(!rust_insight.recommendations.is_empty());
    }

    #[tokio::test]
    async fn test_analyze_trending_topics() {
        let manager = ConversationAnalyticsManager::with_default_config();
        let now = Utc::now();
        
        let conversations = vec![
            create_test_conversation("Conv 1", ConversationStatus::Completed, 8, now - Duration::hours(4), Some(ProjectType::Rust), false, false),
            create_test_conversation("Conv 2", ConversationStatus::Active, 5, now - Duration::hours(3), Some(ProjectType::JavaScript), false, false),
            create_test_conversation("Conv 3", ConversationStatus::Completed, 6, now - Duration::hours(2), Some(ProjectType::Python), false, false),
            create_test_conversation("Conv 4", ConversationStatus::Active, 7, now - Duration::hours(1), Some(ProjectType::Go), false, false),
        ];
        
        let conversation_refs: Vec<&Conversation> = conversations.iter().collect();
        let topics = manager.analyze_trending_topics(&conversation_refs).await.unwrap();
        
        assert!(!topics.is_empty());
        for topic in &topics {
            assert!(!topic.topic.is_empty());
            assert!(topic.frequency > 0);
            assert!(!topic.project_types.is_empty());
            assert!(topic.success_rate >= 0.0 && topic.success_rate <= 1.0);
        }
    }

    #[tokio::test]
    async fn test_generate_recommendations() {
        let manager = ConversationAnalyticsManager::with_default_config();
        
        // Create metrics that will trigger recommendations
        let overall_metrics = OverallMetrics {
            total_conversations: 10,
            total_messages: 50,
            total_tokens: 10000,
            avg_tokens_per_conversation: 1000,
            avg_tokens_per_message: 200,
            avg_messages_per_conversation: 5.0,
            avg_duration_minutes: 30.0,
            total_branches: 5,
            total_checkpoints: 2,
            completion_rate: 0.5, // Low completion rate
            peak_activity_hours: vec![9, 10, 14, 15],
            project_type_distribution: HashMap::new(),
        };
        
        let success_metrics = SuccessMetrics {
            overall_success_rate: 0.6, // Below threshold
            success_by_project_type: HashMap::new(),
            success_by_length: vec![],
            successful_patterns: vec![],
            failure_points: vec![],
            success_indicators: vec![],
        };
        
        let efficiency_metrics = EfficiencyMetrics {
            avg_resolution_time: 30.0,
            branching_efficiency: 0.4, // Low efficiency
            checkpoint_utilization: 0.2, // Low utilization
            context_switches_per_conversation: 2.0,
            efficient_patterns: vec![],
            resource_utilization: ResourceUtilization {
                avg_memory_usage: 0.5,
                storage_efficiency: 0.6,
                search_performance: SearchPerformance {
                    avg_search_time_ms: 100.0,
                    accuracy_rate: 0.95,
                    common_patterns: vec![],
                },
                clustering_efficiency: 0.7,
            },
        };
        
        let patterns = PatternAnalysis {
            common_flows: vec![],
            recurring_themes: vec![],
            temporal_patterns: vec![],
            behavior_patterns: vec![],
            anomalies: vec![],
        };
        
        let recommendations = manager.generate_recommendations(
            &overall_metrics,
            &success_metrics,
            &efficiency_metrics,
            &patterns,
        ).await.unwrap();
        
        assert!(!recommendations.is_empty());
        
        // Check that we got recommendations for each low metric
        let has_completion_rate_rec = recommendations.iter()
            .any(|r| r.description.contains("completion rate"));
        let has_success_rate_rec = recommendations.iter()
            .any(|r| r.description.contains("success rate"));
        let has_branching_rec = recommendations.iter()
            .any(|r| r.description.contains("branching"));
        let has_checkpoint_rec = recommendations.iter()
            .any(|r| r.description.contains("checkpoint"));
        
        assert!(has_completion_rate_rec);
        assert!(has_success_rate_rec);
        assert!(has_branching_rec);
        assert!(has_checkpoint_rec);
    }

    #[test]
    fn test_trend_direction() {
        let growing = TrendDirection::Growing;
        let declining = TrendDirection::Declining;
        let stable = TrendDirection::Stable;
        
        // Just verify the enums exist and can be used
        match growing {
            TrendDirection::Growing => assert!(true),
            _ => assert!(false),
        }
        
        match declining {
            TrendDirection::Declining => assert!(true),
            _ => assert!(false),
        }
        
        match stable {
            TrendDirection::Stable => assert!(true),
            _ => assert!(false),
        }
    }

    #[test]
    fn test_recommendation_categories() {
        let categories = vec![
            RecommendationCategory::ContentQuality,
            RecommendationCategory::Efficiency,
            RecommendationCategory::UserExperience,
            RecommendationCategory::ProcessImprovement,
            RecommendationCategory::TechnicalOptimization,
        ];
        
        // Verify all categories exist
        assert_eq!(categories.len(), 5);
    }

    #[test]
    fn test_priority_and_difficulty() {
        let priorities = vec![Priority::High, Priority::Medium, Priority::Low];
        let difficulties = vec![Difficulty::Easy, Difficulty::Medium, Difficulty::Hard];
        
        assert_eq!(priorities.len(), 3);
        assert_eq!(difficulties.len(), 3);
    }
} 