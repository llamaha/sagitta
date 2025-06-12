//! Task completion detection and tracking

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

use crate::traits::ToolResult;

/// Task completion tracking for conversation state management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCompletion {
    /// Unique task identifier
    pub task_id: String,
    /// Completion marker message
    pub completion_marker: String,
    /// Tool outputs that led to completion
    pub tool_outputs: Vec<String>,
    /// Confidence in task completion (0.0 to 1.0)
    pub success_confidence: f32,
    /// When the task was completed
    pub completed_at: DateTime<Utc>,
    /// Tools that were used to complete this task
    pub tools_used: Vec<String>,
}

/// Signal indicating task completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionSignal {
    /// Type of completion signal
    pub signal_type: CompletionSignalType,
    /// Signal strength (0.0 to 1.0)
    pub strength: f32,
    /// Message or context that triggered the signal
    pub message: String,
    /// When the signal was detected
    pub detected_at: DateTime<Utc>,
}

/// Types of completion signals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompletionSignalType {
    /// Tool reported success
    ToolSuccess,
    /// Output contains completion phrases
    CompletionPhrase,
    /// All objectives met
    ObjectiveComplete,
    /// User indicated satisfaction
    UserSatisfaction,
    /// System detected task completion
    SystemDetection,
}

/// Dedicated analyzer for task completion detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCompletionAnalyzer {
    /// Completion phrase patterns
    completion_patterns: Vec<String>,
    /// Tool completion indicators
    tool_completion_indicators: HashMap<String, Vec<String>>,
    /// Minimum confidence threshold for completion
    min_confidence_threshold: f32,
}

impl TaskCompletionAnalyzer {
    pub fn new() -> Self {
        let mut completion_patterns = Vec::new();
        completion_patterns.extend([
            "task completed",
            "task is complete",
            "successfully completed",
            "all done",
            "finished",
            "task accomplished",
            "implementation complete",
            "work completed",
            "all steps completed",
            "objective achieved",
            "goal accomplished",
            "requirements met",
            "deliverable ready",
            "solution implemented",
            "request fulfilled",
        ].iter().map(|s| s.to_string()));
        
        let mut tool_completion_indicators = HashMap::new();
        
        // File operations completion indicators
        tool_completion_indicators.insert("edit_file".to_string(), vec![
            "successfully".to_string(),
            "created".to_string(),
            "updated".to_string(),
            "modified".to_string(),
        ]);
        
        // Shell command completion indicators
        tool_completion_indicators.insert("run_terminal_cmd".to_string(), vec![
            "command completed".to_string(),
            "execution successful".to_string(),
            "exit code 0".to_string(),
        ]);
        
        // Search and analysis completion indicators
        tool_completion_indicators.insert("codebase_search".to_string(), vec![
            "search completed".to_string(),
            "results found".to_string(),
            "analysis complete".to_string(),
        ]);
        
        Self {
            completion_patterns,
            tool_completion_indicators,
            min_confidence_threshold: 0.7,
        }
    }
    
    /// Detect task completion with enhanced multi-signal analysis
    pub fn detect_completion(
        &self,
        original_request: &str,
        response_text: &str,
        tool_results: &HashMap<String, ToolResult>,
    ) -> Option<TaskCompletion> {
        let mut completion_signals = Vec::new();
        
        // Check if this is a multi-step task that might not be complete
        if self.is_multistep_task(original_request) {
            let multistep_complete = self.check_all_multistep_tasks_complete(response_text, tool_results);
            if !multistep_complete {
                return None;
            }
        }
        
        // Detect completion phrases in response
        if let Some(signal) = self.detect_completion_phrases(response_text) {
            completion_signals.push(signal);
        }
        
        // Detect tool completion signals
        for (tool_name, result) in tool_results {
            if let Some(signal) = self.detect_tool_completion_signal(tool_name, result) {
                completion_signals.push(signal);
            }
        }
        
        // Adjust threshold based on task type and completion requirements
        let threshold = if self.has_explicit_completion_request(original_request) {
            0.9 // Higher threshold for explicit completion requests
        } else if self.requires_solution_creation(original_request) {
            0.8 // Medium-high threshold for solution creation
        } else {
            self.min_confidence_threshold
        };
        
        // For multi-step tasks, also check if all steps are mentioned as complete
        if self.is_multistep_task(original_request) {
            let all_steps_complete = self.check_all_multistep_tasks_complete(response_text, tool_results);
            if !all_steps_complete {
                return None;
            }
        }
        
        // Calculate overall confidence
        let avg_confidence = if completion_signals.is_empty() {
            0.0
        } else {
            completion_signals.iter().map(|s| s.strength).sum::<f32>() / completion_signals.len() as f32
        };
        
        if avg_confidence >= threshold {
            let completion_marker = completion_signals
                .first()
                .map(|s| s.message.clone())
                .unwrap_or_else(|| "Task completion detected".to_string());
            
            Some(TaskCompletion {
                task_id: Uuid::new_v4().to_string(),
                completion_marker,
                tool_outputs: tool_results.values().map(|r| format!("{:?}", r.data)).collect(),
                success_confidence: avg_confidence,
                completed_at: Utc::now(),
                tools_used: tool_results.keys().cloned().collect(),
            })
        } else {
            None
        }
    }
    
    fn is_multistep_task(&self, request: &str) -> bool {
        let request_lower = request.to_lowercase();
        
        // Look for explicit step indicators
        let step_indicators = [
            "steps:", "step 1", "first,", "then,", "next,", "finally,",
            "1.", "2.", "3.", "•", "-", "and then", "after that",
        ];
        
        let has_step_indicators = step_indicators.iter()
            .any(|indicator| request_lower.contains(indicator));
        
        // Check for complex task patterns that inherently require multiple steps
        let complex_patterns = [
            "create and test", "build and deploy", "setup and configure",
            "analyze and implement", "research and develop", "design and implement",
            "refactor and optimize", "migrate and update", "install and configure",
        ];
        
        let has_complex_patterns = complex_patterns.iter()
            .any(|pattern| request_lower.contains(pattern));
        
        has_step_indicators || has_complex_patterns
    }
    
    fn has_explicit_completion_request(&self, request: &str) -> bool {
        let request_lower = request.to_lowercase();
        let completion_requests = [
            "make sure", "ensure that", "verify that", "confirm that",
            "complete all", "finish all", "do all", "implement all",
            "let me know when", "notify when", "confirm when",
        ];
        
        completion_requests.iter()
            .any(|phrase| request_lower.contains(phrase))
    }
    
    fn check_all_multistep_tasks_complete(&self, response_text: &str, tool_results: &HashMap<String, ToolResult>) -> bool {
        let response_lower = response_text.to_lowercase();
        
        // Special handling for complex workflows that require solution creation
        if response_lower.contains("create") || response_lower.contains("implement") || 
           response_lower.contains("build") || response_lower.contains("develop") {
            return self.has_completed_solution_creation(&response_lower, tool_results);
        }
        
        // Check for completion indicators in response
        let completion_indicators = [
            "all steps completed", "everything is done", "all tasks finished",
            "implementation complete", "all requirements met", "fully implemented",
            "all objectives achieved", "completely finished", "all done",
            "task accomplished", "work completed", "successfully completed all",
        ];
        
        let has_completion_indicator = completion_indicators.iter()
            .any(|indicator| response_lower.contains(indicator));
        
        if has_completion_indicator {
            return true;
        }
        
        // Extract action words from the original request and check if corresponding tools were used
        let task_actions = self.extract_task_actions(&response_lower);
        let tools_used: Vec<String> = tool_results.keys().cloned().collect();
        
        let mut completed_actions = 0;
        for action in &task_actions {
            for tool in &tools_used {
                if self.tool_matches_action(tool, action) {
                    completed_actions += 1;
                    break;
                }
            }
        }
        
        // For complex workflows, require higher completion rate
        let required_completion_rate = if task_actions.len() > 3 { 0.8 } else { 0.6 };
        (completed_actions as f32 / task_actions.len() as f32) >= required_completion_rate
    }
    
    fn requires_solution_creation(&self, request: &str) -> bool {
        let request_lower = request.to_lowercase();
        let creation_verbs = [
            "create", "build", "develop", "implement", "design", "write",
            "generate", "construct", "make", "produce", "establish", "setup",
        ];
        
        creation_verbs.iter().any(|verb| request_lower.contains(verb))
    }
    
    fn has_completed_solution_creation(&self, response: &str, tool_results: &HashMap<String, ToolResult>) -> bool {
        // Check if file creation/editing tools were used successfully
        let creation_tools = ["edit_file", "create_file", "write_file"];
        let used_creation_tools = creation_tools.iter()
            .any(|tool| tool_results.contains_key(*tool));
        
        if !used_creation_tools {
            return false;
        }
        
        // Check for completion phrases in the response
        let completion_phrases = [
            "created successfully", "implemented successfully", "solution complete",
            "file created", "code written", "implementation finished",
            "development complete", "successfully built", "ready to use",
        ];
        
        completion_phrases.iter().any(|phrase| response.contains(phrase))
    }
    
    fn extract_task_actions(&self, request: &str) -> Vec<String> {
        let action_verbs = [
            "create", "build", "implement", "design", "write", "read", "update", "delete",
            "analyze", "search", "find", "install", "configure", "setup", "test", "deploy",
            "refactor", "optimize", "fix", "debug", "migrate", "upgrade", "document",
        ];
        
        let mut actions = Vec::new();
        let words: Vec<&str> = request.split_whitespace().collect();
        
        for (i, word) in words.iter().enumerate() {
            let word_lower = word.to_lowercase();
            if action_verbs.contains(&word_lower.as_str()) {
                // Try to capture the action with its object
                let action = if i + 1 < words.len() {
                    format!("{} {}", word_lower, words[i + 1].to_lowercase())
                } else {
                    word_lower
                };
                actions.push(action);
            }
        }
        
        actions
    }
    
    fn tool_matches_action(&self, tool_name: &str, action: &str) -> bool {
        let tool_lower = tool_name.to_lowercase();
        let action_lower = action.to_lowercase();
        
        // Direct name matching
        if tool_lower.contains(&action_lower) || action_lower.contains(&tool_lower) {
            return true;
        }
        
        // Semantic matching for common patterns
        match action_lower.as_str() {
            action if action.contains("create") || action.contains("write") => {
                tool_lower.contains("edit") || tool_lower.contains("file") || tool_lower.contains("create")
            }
            action if action.contains("search") || action.contains("find") => {
                tool_lower.contains("search") || tool_lower.contains("grep") || tool_lower.contains("find")
            }
            action if action.contains("run") || action.contains("execute") => {
                tool_lower.contains("terminal") || tool_lower.contains("cmd") || tool_lower.contains("run")
            }
            action if action.contains("read") || action.contains("view") => {
                tool_lower.contains("read") || tool_lower.contains("file") || tool_lower.contains("view")
            }
            _ => false,
        }
    }
    
    fn detect_completion_phrases(&self, text: &str) -> Option<CompletionSignal> {
        let text_lower = text.to_lowercase();
        
        for pattern in &self.completion_patterns {
            if text_lower.contains(pattern) {
                let strength = if pattern.contains("successfully") || pattern.contains("accomplished") {
                    0.9
                } else if pattern.contains("complete") || pattern.contains("finished") {
                    0.8
                } else {
                    0.7
                };
                
                return Some(CompletionSignal {
                    signal_type: CompletionSignalType::CompletionPhrase,
                    strength,
                    message: format!("Completion phrase detected: {}", pattern),
                    detected_at: Utc::now(),
                });
            }
        }
        
        None
    }
    
    fn detect_tool_completion_signal(&self, tool_name: &str, result: &ToolResult) -> Option<CompletionSignal> {
        if result.success {
            let content_text = result.data.as_str().unwrap_or("").to_lowercase();
            
            if let Some(indicators) = self.tool_completion_indicators.get(tool_name) {
                for indicator in indicators {
                    if content_text.contains(indicator) {
                        return Some(CompletionSignal {
                            signal_type: CompletionSignalType::ToolSuccess,
                            strength: 0.8,
                            message: format!("Tool {} completed successfully: {}", tool_name, indicator),
                            detected_at: Utc::now(),
                        });
                    }
                }
            }
            
            // Generic success signal
            Some(CompletionSignal {
                signal_type: CompletionSignalType::ToolSuccess,
                strength: 0.6,
                message: format!("Tool {} executed successfully", tool_name),
                detected_at: Utc::now(),
            })
        } else {
            // Check for patterns in error messages that might indicate partial success
            if let Some(error_msg) = &result.error {
                if error_msg.contains("already exists") || 
                   error_msg.contains("already available") ||
                   error_msg.contains("up to date") {
                    Some(CompletionSignal {
                        signal_type: CompletionSignalType::ToolSuccess,
                        strength: 0.8,
                        message: format!("Tool {} achieved desired state (already exists)", tool_name),
                        detected_at: Utc::now(),
                    })
                } else {
                    None
                }
            } else {
                None
            }
        }
    }
}

impl Default for TaskCompletionAnalyzer {
    fn default() -> Self {
        Self::new()
    }
} 