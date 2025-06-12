use crate::error::{Result, ReasoningError};
use super::types::{
    FailureCategory, FailureAnalysis, RecoveryStrategy, RecoveryStrategyType,
    RecoverySuggestions, SimplifiedApproach
};

/// Handles failure analysis and recovery strategy generation
pub struct RecoveryEngine {
    // In a real implementation, this might have configuration and learning capabilities
}

impl RecoveryEngine {
    /// Create a new recovery engine
    pub fn new() -> Self {
        Self {}
    }

    /// Analyze a failure and suggest recovery strategies
    pub async fn analyze_failure_and_suggest_recovery(
        &self,
        tool_name: &str,
        error_message: &str,
        parameters: &serde_json::Value,
        retry_attempts: u32,
    ) -> Result<RecoverySuggestions> {
        let analysis = self.analyze_failure(tool_name, error_message).await;
        let strategies = self.generate_recovery_strategies(tool_name, &analysis, parameters, retry_attempts).await?;
        let user_recommendations = self.generate_user_recommendations(tool_name, &analysis, &strategies);
        let requires_manual_intervention = self.requires_manual_intervention(tool_name, error_message);

        Ok(RecoverySuggestions {
            strategies,
            failure_analysis: analysis,
            user_recommendations,
            requires_manual_intervention,
        })
    }

    /// Analyze the failure to determine category and characteristics
    async fn analyze_failure(&self, tool_name: &str, error_message: &str) -> FailureAnalysis {
        let category = self.categorize_failure(error_message);
        let root_cause = self.determine_root_cause(tool_name, error_message, &category);
        let is_recoverable = self.is_failure_recoverable(&category, error_message);
        let retry_success_probability = self.estimate_retry_success_probability(&category, error_message);
        let alternatives_available = self.check_alternatives_available(tool_name, &category);

        FailureAnalysis {
            failure_category: category,
            root_cause,
            is_recoverable,
            retry_success_probability,
            alternatives_available,
        }
    }

    /// Categorize the failure based on error message patterns
    fn categorize_failure(&self, error_message: &str) -> FailureCategory {
        let error_lower = error_message.to_lowercase();
        
        if error_lower.contains("network") || error_lower.contains("connection") 
            || error_lower.contains("timeout") || error_lower.contains("dns") {
            FailureCategory::NetworkError
        } else if error_lower.contains("permission") || error_lower.contains("unauthorized") 
            || error_lower.contains("forbidden") || error_lower.contains("access denied") {
            FailureCategory::AuthenticationError
        } else if error_lower.contains("parameter") || error_lower.contains("argument") 
            || error_lower.contains("invalid") || error_lower.contains("malformed") {
            FailureCategory::ParameterError
        } else if error_lower.contains("resource") || error_lower.contains("quota") 
            || error_lower.contains("limit") || error_lower.contains("memory") {
            FailureCategory::ResourceError
        } else if error_lower.contains("config") || error_lower.contains("setup") 
            || error_lower.contains("initialization") {
            FailureCategory::ConfigurationError
        } else if error_lower.contains("dependency") || error_lower.contains("requirement") 
            || error_lower.contains("missing") {
            FailureCategory::DependencyError
        } else if error_lower.contains("timeout") || error_lower.contains("timed out") {
            FailureCategory::TimeoutError
        } else {
            FailureCategory::UnknownError
        }
    }

    /// Determine the root cause of the failure
    fn determine_root_cause(&self, tool_name: &str, error_message: &str, category: &FailureCategory) -> String {
        match category {
            FailureCategory::NetworkError => {
                format!("Network connectivity issue while executing {}: {}", tool_name, error_message)
            }
            FailureCategory::AuthenticationError => {
                format!("Authentication or permission issue with {}: {}", tool_name, error_message)
            }
            FailureCategory::ParameterError => {
                format!("Invalid parameters provided to {}: {}", tool_name, error_message)
            }
            FailureCategory::ResourceError => {
                format!("Resource exhaustion or limits reached for {}: {}", tool_name, error_message)
            }
            FailureCategory::ConfigurationError => {
                format!("Configuration issue with {}: {}", tool_name, error_message)
            }
            FailureCategory::DependencyError => {
                format!("Missing dependencies for {}: {}", tool_name, error_message)
            }
            FailureCategory::TimeoutError => {
                format!("Tool {} timed out: {}", tool_name, error_message)
            }
            FailureCategory::UnknownError => {
                format!("Unknown error in {}: {}", tool_name, error_message)
            }
        }
    }

    /// Determine if the failure is recoverable
    fn is_failure_recoverable(&self, category: &FailureCategory, error_message: &str) -> bool {
        match category {
            FailureCategory::NetworkError => true, // Usually transient
            FailureCategory::AuthenticationError => false, // Usually requires manual intervention
            FailureCategory::ParameterError => true, // Can often be fixed by parameter variation
            FailureCategory::ResourceError => true, // May be resolved with retry or resource adjustment
            FailureCategory::ConfigurationError => false, // Usually requires manual configuration
            FailureCategory::DependencyError => false, // Usually requires manual dependency installation
            FailureCategory::TimeoutError => true, // May succeed with longer timeout
            FailureCategory::UnknownError => {
                // Try to infer from error message
                !error_message.to_lowercase().contains("fatal")
            }
        }
    }

    /// Estimate the probability of success with retry
    fn estimate_retry_success_probability(&self, category: &FailureCategory, error_message: &str) -> f32 {
        match category {
            FailureCategory::NetworkError => 0.7, // Often transient
            FailureCategory::AuthenticationError => 0.1, // Rarely fixed by retry alone
            FailureCategory::ParameterError => 0.6, // Good chance with parameter variation
            FailureCategory::ResourceError => 0.4, // May be resolved if resources freed up
            FailureCategory::ConfigurationError => 0.2, // Usually requires manual intervention
            FailureCategory::DependencyError => 0.2, // Usually requires manual intervention
            FailureCategory::TimeoutError => 0.8, // Often succeeds with more time
            FailureCategory::UnknownError => {
                if error_message.to_lowercase().contains("temporary") {
                    0.6
                } else {
                    0.3
                }
            }
        }
    }

    /// Check if alternative tools are available
    fn check_alternatives_available(&self, tool_name: &str, category: &FailureCategory) -> bool {
        match tool_name {
            "git_clone" => true, // Could use download_file or manual clone
            "shell_execute" => true, // Could break down into smaller commands
            "file_edit" => true, // Could use different editing approaches
            "search_replace" => true, // Could use manual file operations
            _ => match category {
                FailureCategory::NetworkError => true, // May have offline alternatives
                FailureCategory::ParameterError => true, // Can try parameter variations
                _ => false,
            }
        }
    }

    /// Generate recovery strategies based on failure analysis
    async fn generate_recovery_strategies(
        &self,
        tool_name: &str,
        analysis: &FailureAnalysis,
        parameters: &serde_json::Value,
        retry_attempts: u32,
    ) -> Result<Vec<RecoveryStrategy>> {
        let mut strategies = Vec::new();

        // Basic retry strategy if recoverable
        if analysis.is_recoverable && retry_attempts < 3 {
            strategies.push(RecoveryStrategy {
                strategy_type: RecoveryStrategyType::BasicRetry,
                alternative_tool: None,
                modified_parameters: None,
                simplified_approach: None,
                description: format!("Retry {} with same parameters", tool_name),
                confidence: analysis.retry_success_probability,
            });
        }

        // Alternative tool strategies
        strategies.extend(self.generate_alternative_tool_strategies(tool_name, analysis).await);

        // Parameter variation strategies
        strategies.extend(self.generate_parameter_variation_strategies(tool_name, parameters).await);

        // Simplified approach strategies
        strategies.extend(self.generate_simplified_approach_strategies(tool_name, analysis, parameters).await);

        // Manual fallback strategies
        strategies.extend(self.generate_manual_fallback_strategies(tool_name, analysis).await);

        Ok(strategies)
    }

    /// Generate alternative tool strategies
    async fn generate_alternative_tool_strategies(
        &self,
        tool_name: &str,
        _analysis: &FailureAnalysis,
    ) -> Vec<RecoveryStrategy> {
        let alternatives = match tool_name {
            "git_clone" => vec![
                ("download_file", "Download repository as ZIP file", 0.8),
                ("shell_execute", "Use git command directly", 0.6),
            ],
            "file_edit" => vec![
                ("search_replace", "Use search and replace operations", 0.7),
                ("shell_execute", "Use sed/awk for file editing", 0.5),
            ],
            "shell_execute" => vec![
                ("file_edit", "Break down into file operations", 0.6),
            ],
            _ => vec![],
        };

        alternatives.into_iter().map(|(alt_tool, description, confidence)| {
            RecoveryStrategy {
                strategy_type: RecoveryStrategyType::AlternativeTool,
                alternative_tool: Some(alt_tool.to_string()),
                modified_parameters: None,
                simplified_approach: None,
                description: description.to_string(),
                confidence,
            }
        }).collect()
    }

    /// Generate parameter variation strategies
    async fn generate_parameter_variation_strategies(
        &self,
        tool_name: &str,
        parameters: &serde_json::Value,
    ) -> Vec<RecoveryStrategy> {
        let mut strategies = Vec::new();

        match tool_name {
            "shell_execute" => {
                if let Some(command) = parameters.get("command").and_then(|c| c.as_str()) {
                    // Add timeout if not present
                    if !command.contains("timeout") {
                        let mut modified = parameters.clone();
                        modified["command"] = serde_json::Value::String(format!("timeout 30 {}", command));
                        strategies.push(RecoveryStrategy {
                            strategy_type: RecoveryStrategyType::ParameterVariation,
                            alternative_tool: None,
                            modified_parameters: Some(modified),
                            simplified_approach: None,
                            description: "Add timeout to shell command".to_string(),
                            confidence: 0.6,
                        });
                    }
                }
            }
            "git_clone" => {
                if let Some(url) = parameters.get("url").and_then(|u| u.as_str()) {
                    // Try with different clone options
                    let mut modified = parameters.clone();
                    modified["depth"] = serde_json::Value::Number(1.into());
                    strategies.push(RecoveryStrategy {
                        strategy_type: RecoveryStrategyType::ParameterVariation,
                        alternative_tool: None,
                        modified_parameters: Some(modified),
                        simplified_approach: None,
                        description: "Use shallow clone to reduce download size".to_string(),
                        confidence: 0.7,
                    });
                }
            }
            _ => {}
        }

        strategies
    }

    /// Generate simplified approach strategies
    async fn generate_simplified_approach_strategies(
        &self,
        tool_name: &str,
        _analysis: &FailureAnalysis,
        _parameters: &serde_json::Value,
    ) -> Vec<RecoveryStrategy> {
        let simplified_approaches = match tool_name {
            "file_edit" => vec![
                SimplifiedApproach {
                    reduced_parameters: serde_json::json!({"backup": false}),
                    reduction_description: "Skip file backup to reduce disk usage".to_string(),
                    maintains_core_functionality: true,
                },
            ],
            "git_clone" => vec![
                SimplifiedApproach {
                    reduced_parameters: serde_json::json!({"depth": 1, "single_branch": true}),
                    reduction_description: "Use shallow clone with single branch".to_string(),
                    maintains_core_functionality: true,
                },
            ],
            _ => vec![],
        };

        simplified_approaches.into_iter().map(|approach| {
            RecoveryStrategy {
                strategy_type: RecoveryStrategyType::SimplifiedApproach,
                alternative_tool: None,
                modified_parameters: None,
                simplified_approach: Some(approach),
                description: format!("Use simplified approach for {}", tool_name),
                confidence: 0.5,
            }
        }).collect()
    }

    /// Generate manual fallback strategies
    async fn generate_manual_fallback_strategies(
        &self,
        tool_name: &str,
        _analysis: &FailureAnalysis,
    ) -> Vec<RecoveryStrategy> {
        vec![
            RecoveryStrategy {
                strategy_type: RecoveryStrategyType::ManualFallback,
                alternative_tool: Some("shell_execute".to_string()),
                modified_parameters: None,
                simplified_approach: None,
                description: format!("Manually execute {} operations using shell commands", tool_name),
                confidence: 0.4,
            }
        ]
    }

    /// Generate user recommendations
    fn generate_user_recommendations(
        &self,
        tool_name: &str,
        analysis: &FailureAnalysis,
        strategies: &[RecoveryStrategy],
    ) -> Vec<String> {
        let mut recommendations = Vec::new();

        match &analysis.failure_category {
            FailureCategory::NetworkError => {
                recommendations.push("Check network connectivity and try again".to_string());
                recommendations.push("Consider using a VPN if behind a firewall".to_string());
            }
            FailureCategory::AuthenticationError => {
                recommendations.push("Verify credentials and permissions".to_string());
                recommendations.push("Check if the tool requires additional setup".to_string());
            }
            FailureCategory::ParameterError => {
                recommendations.push("Review the parameters provided to the tool".to_string());
                recommendations.push("Check the tool documentation for correct usage".to_string());
            }
            FailureCategory::ResourceError => {
                recommendations.push("Free up system resources and try again".to_string());
                recommendations.push("Consider breaking down the operation into smaller parts".to_string());
            }
            FailureCategory::ConfigurationError => {
                recommendations.push("Check the tool configuration and setup".to_string());
                recommendations.push("Verify that all required dependencies are installed".to_string());
            }
            FailureCategory::DependencyError => {
                recommendations.push("Install missing dependencies".to_string());
                recommendations.push("Check system requirements for the tool".to_string());
            }
            FailureCategory::TimeoutError => {
                recommendations.push("Increase timeout values if possible".to_string());
                recommendations.push("Consider breaking down the operation into smaller parts".to_string());
            }
            FailureCategory::UnknownError => {
                recommendations.push("Review the error message for more specific guidance".to_string());
                recommendations.push("Consider using an alternative approach".to_string());
            }
        }

        // Add strategy-specific recommendations
        if strategies.iter().any(|s| s.strategy_type == RecoveryStrategyType::AlternativeTool) {
            recommendations.push(format!("Consider using alternative tools for {}", tool_name));
        }

        if strategies.iter().any(|s| s.strategy_type == RecoveryStrategyType::SimplifiedApproach) {
            recommendations.push("Try a simplified approach with reduced functionality".to_string());
        }

        recommendations
    }

    /// Determine if manual intervention is required
    fn requires_manual_intervention(&self, _tool_name: &str, error_message: &str) -> bool {
        let error_lower = error_message.to_lowercase();
        error_lower.contains("permission") 
            || error_lower.contains("access denied")
            || error_lower.contains("forbidden")
            || error_lower.contains("configuration")
            || error_lower.contains("setup required")
            || error_lower.contains("missing dependency")
    }
} 