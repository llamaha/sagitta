use std::sync::Arc;
use anyhow::Result;
use chrono::{DateTime, Utc};
use tokio::sync::Mutex;
use uuid::Uuid;

use super::types::CompletionCriteria;
use super::panel::TaskPanel;
use crate::agent::conversation::manager::ConversationManager;
use crate::agent::conversation::types::Conversation;
use crate::agent::message::types::AgentMessage;

/// Monitors conversations for completion and triggers next tasks
pub struct ConversationCompletionDetector {
    task_panel: Arc<Mutex<TaskPanel>>,
    conversation_manager: Option<Arc<dyn ConversationManager>>,
    completion_criteria: CompletionCriteria,
    monitored_conversations: Arc<Mutex<std::collections::HashMap<Uuid, MonitoredConversation>>>,
}

/// Information about a conversation being monitored for completion
#[derive(Debug, Clone)]
struct MonitoredConversation {
    conversation_id: Uuid,
    task_id: Uuid,
    started_at: DateTime<Utc>,
    last_activity: DateTime<Utc>,
    has_errors: bool,
    test_status: Option<TestStatus>,
}

/// Test execution status
#[derive(Debug, Clone, PartialEq)]
enum TestStatus {
    NotRun,
    Running,
    Passed,
    Failed,
}

impl ConversationCompletionDetector {
    /// Create a new completion detector
    pub fn new(
        task_panel: Arc<Mutex<TaskPanel>>,
        conversation_manager: Option<Arc<dyn ConversationManager>>,
        completion_criteria: CompletionCriteria,
    ) -> Self {
        Self {
            task_panel,
            conversation_manager,
            completion_criteria,
            monitored_conversations: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Start monitoring a conversation for completion
    pub async fn start_monitoring(&self, conversation_id: Uuid, task_id: Uuid) {
        let mut monitored = self.monitored_conversations.lock().await;
        monitored.insert(
            conversation_id,
            MonitoredConversation {
                conversation_id,
                task_id,
                started_at: Utc::now(),
                last_activity: Utc::now(),
                has_errors: false,
                test_status: Some(TestStatus::NotRun),
            },
        );
        log::info!("Started monitoring conversation {} for task {}", conversation_id, task_id);
    }

    /// Stop monitoring a conversation
    pub async fn stop_monitoring(&self, conversation_id: Uuid) {
        let mut monitored = self.monitored_conversations.lock().await;
        if monitored.remove(&conversation_id).is_some() {
            log::info!("Stopped monitoring conversation {}", conversation_id);
        }
    }

    /// Check if a conversation should be considered complete
    pub async fn check_conversation_completion(&self, conversation_id: Uuid) -> Result<bool> {
        if let Some(conversation_manager) = &self.conversation_manager {
            if let Some(conversation) = conversation_manager.get_conversation(conversation_id).await? {
                let completion_result = self.analyze_conversation_for_completion(&conversation).await;
                
                // Update monitoring status
                if let Ok(is_complete) = completion_result {
                    if is_complete {
                        self.handle_conversation_completion(conversation_id).await?;
                        return Ok(true);
                    }
                }
                
                completion_result
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }

    /// Analyze a conversation to determine if it's complete
    async fn analyze_conversation_for_completion(&self, conversation: &Conversation) -> Result<bool> {
        let recent_messages: Vec<&AgentMessage> = conversation.messages
            .iter()
            .rev()
            .take(10)
            .collect();

        if recent_messages.is_empty() {
            return Ok(false);
        }

        let mut completion_indicators = CompletionIndicators::default();
        
        // Analyze recent messages for completion indicators
        for message in &recent_messages {
            self.analyze_message_for_completion(message, &mut completion_indicators);
        }

        // Check timeout
        if let Some(timeout_minutes) = self.completion_criteria.timeout_minutes {
            let timeout_duration = chrono::Duration::minutes(timeout_minutes as i64);
            if let Some(latest_message) = conversation.messages.last() {
                if latest_message.timestamp + timeout_duration < Utc::now() {
                    log::warn!("Conversation {} timed out after {} minutes", conversation.id, timeout_minutes);
                    return Ok(false); // Consider as failed, not completed
                }
            }
        }

        // Evaluate completion based on criteria
        self.evaluate_completion(&completion_indicators).await
    }

    /// Analyze a single message for completion indicators
    fn analyze_message_for_completion(&self, message: &AgentMessage, indicators: &mut CompletionIndicators) {
        let content_lower = message.content.to_lowercase();

        // Check for completion keywords
        for keyword in &self.completion_criteria.completion_keywords {
            if content_lower.contains(&keyword.to_lowercase()) {
                indicators.completion_keywords_found += 1;
                indicators.explicit_completion = true;
            }
        }

        // Check for failure keywords
        for keyword in &self.completion_criteria.failure_keywords {
            if content_lower.contains(&keyword.to_lowercase()) {
                indicators.failure_keywords_found += 1;
                indicators.has_errors = true;
            }
        }

        // Check for test results
        if content_lower.contains("test") {
            if content_lower.contains("passed") || content_lower.contains("✅") || content_lower.contains("success") {
                indicators.tests_passed = true;
            } else if content_lower.contains("failed") || content_lower.contains("❌") || content_lower.contains("error") {
                indicators.tests_failed = true;
            }
        }

        // Check for lint results
        if content_lower.contains("lint") || content_lower.contains("clippy") {
            if content_lower.contains("no errors") || content_lower.contains("no warnings") || content_lower.contains("clean") {
                indicators.lint_clean = true;
            } else if content_lower.contains("error") || content_lower.contains("warning") {
                indicators.lint_errors = true;
            }
        }

        // Check for implementation completion
        if content_lower.contains("implemented") || content_lower.contains("implementation") {
            if content_lower.contains("complete") || content_lower.contains("finished") {
                indicators.implementation_complete = true;
            }
        }
    }

    /// Evaluate whether the conversation is complete based on indicators
    async fn evaluate_completion(&self, indicators: &CompletionIndicators) -> Result<bool> {
        // If explicit completion is required and not found, not complete
        if self.completion_criteria.require_explicit_completion && !indicators.explicit_completion {
            return Ok(false);
        }

        // If tests are required and failed, not complete
        if self.completion_criteria.require_tests_pass {
            if indicators.tests_failed {
                return Ok(false);
            }
            if !indicators.tests_passed {
                return Ok(false);
            }
        }

        // If lint check is required and has errors, not complete
        if self.completion_criteria.check_lint_errors && indicators.lint_errors {
            return Ok(false);
        }

        // If there are failure indicators, not complete
        if indicators.has_errors || indicators.failure_keywords_found > 0 {
            return Ok(false);
        }

        // Positive completion indicators
        let positive_indicators = indicators.completion_keywords_found > 0
            || indicators.implementation_complete
            || (self.completion_criteria.require_tests_pass && indicators.tests_passed)
            || (self.completion_criteria.check_lint_errors && indicators.lint_clean);

        Ok(positive_indicators)
    }

    /// Handle conversation completion by triggering the next task
    async fn handle_conversation_completion(&self, conversation_id: Uuid) -> Result<()> {
        // Stop monitoring this conversation
        self.stop_monitoring(conversation_id).await;
        
        // Complete the active task and potentially start the next one
        let task_panel = self.task_panel.lock().await;
        if let Ok(Some(completed_task_id)) = task_panel.complete_active_task().await {
            log::info!("Completed task {} from conversation {}", completed_task_id, conversation_id);
            
            // The complete_active_task method will automatically start the next task
            // if auto-progress is enabled
        }
        
        Ok(())
    }

    /// Periodic check for all monitored conversations
    pub async fn check_all_monitored_conversations(&self) -> Result<()> {
        let conversation_ids: Vec<Uuid> = {
            let monitored = self.monitored_conversations.lock().await;
            monitored.keys().cloned().collect()
        };

        for conversation_id in conversation_ids {
            if let Err(e) = self.check_conversation_completion(conversation_id).await {
                log::error!("Error checking completion for conversation {}: {}", conversation_id, e);
            }
        }

        Ok(())
    }

    /// Get completion criteria
    pub fn get_completion_criteria(&self) -> &CompletionCriteria {
        &self.completion_criteria
    }

    /// Update completion criteria
    pub fn update_completion_criteria(&mut self, criteria: CompletionCriteria) {
        self.completion_criteria = criteria;
    }
}

/// Indicators found in conversation analysis
#[derive(Debug, Default)]
struct CompletionIndicators {
    completion_keywords_found: usize,
    failure_keywords_found: usize,
    explicit_completion: bool,
    tests_passed: bool,
    tests_failed: bool,
    lint_clean: bool,
    lint_errors: bool,
    implementation_complete: bool,
    has_errors: bool,
}

/// Start the completion detector background task
pub async fn start_completion_detector(
    detector: Arc<ConversationCompletionDetector>,
    check_interval_seconds: u64,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(check_interval_seconds));
        
        loop {
            interval.tick().await;
            
            if let Err(e) = detector.check_all_monitored_conversations().await {
                log::error!("Error in completion detector periodic check: {}", e);
            }
        }
    })
}