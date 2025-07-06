use anyhow::Result;
use chrono::{DateTime, Utc, Duration};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tokio::time::interval;
use uuid::Uuid;

use super::types::*;
use super::manager::TaskManager;
use super::conversation::ConversationTaskIntegration;

/// Task scheduler for managing scheduled tasks and conversations
pub struct TaskScheduler {
    task_manager: Arc<dyn TaskManager>,
    conversation_integration: Arc<ConversationTaskIntegration>,
    scheduled_tasks: Arc<RwLock<HashMap<Uuid, ScheduledTask>>>,
    scheduler_tx: Option<mpsc::UnboundedSender<SchedulerCommand>>,
    is_running: Arc<RwLock<bool>>,
}

/// Internal scheduled task representation
#[derive(Debug, Clone)]
struct ScheduledTask {
    task_id: Uuid,
    scheduled_at: DateTime<Utc>,
    recurring: Option<RecurringSchedule>,
    last_executed: Option<DateTime<Utc>>,
    next_execution: DateTime<Utc>,
}

/// Recurring schedule configuration
#[derive(Debug, Clone)]
pub struct RecurringSchedule {
    pub interval: ScheduleInterval,
    pub max_executions: Option<usize>,
    pub end_date: Option<DateTime<Utc>>,
    pub execution_count: usize,
}

/// Schedule interval types
#[derive(Debug, Clone)]
pub enum ScheduleInterval {
    Minutes(u32),
    Hours(u32),
    Days(u32),
    Weeks(u32),
    Monthly,
    Custom(Duration),
}

/// Commands for the scheduler
#[derive(Debug)]
enum SchedulerCommand {
    ScheduleTask {
        task_id: Uuid,
        scheduled_at: DateTime<Utc>,
        recurring: Option<RecurringSchedule>,
    },
    UnscheduleTask {
        task_id: Uuid,
    },
    UpdateSchedule {
        task_id: Uuid,
        scheduled_at: DateTime<Utc>,
        recurring: Option<RecurringSchedule>,
    },
    Shutdown,
}

/// Task execution event
#[derive(Debug, Clone)]
pub struct TaskExecutionEvent {
    pub task_id: Uuid,
    pub execution_time: DateTime<Utc>,
    pub success: bool,
    pub conversation_id: Option<Uuid>,
    pub error_message: Option<String>,
}

impl TaskScheduler {
    /// Create a new task scheduler
    pub fn new(
        task_manager: Arc<dyn TaskManager>,
        conversation_integration: Arc<ConversationTaskIntegration>,
    ) -> Self {
        Self {
            task_manager,
            conversation_integration,
            scheduled_tasks: Arc::new(RwLock::new(HashMap::new())),
            scheduler_tx: None,
            is_running: Arc::new(RwLock::new(false)),
        }
    }
    
    /// Start the scheduler
    pub async fn start(&mut self) -> Result<mpsc::UnboundedReceiver<TaskExecutionEvent>> {
        let (scheduler_tx, mut scheduler_rx) = mpsc::unbounded_channel::<SchedulerCommand>();
        let (event_tx, event_rx) = mpsc::unbounded_channel::<TaskExecutionEvent>();
        
        self.scheduler_tx = Some(scheduler_tx);
        
        let task_manager = Arc::clone(&self.task_manager);
        let conversation_integration = Arc::clone(&self.conversation_integration);
        let scheduled_tasks = Arc::clone(&self.scheduled_tasks);
        let is_running = Arc::clone(&self.is_running);
        
        // Set running state
        {
            let mut running = is_running.write().await;
            *running = true;
        }
        
        // Spawn the scheduler task
        tokio::spawn(async move {
            let mut check_interval = interval(tokio::time::Duration::from_secs(60)); // Check every minute
            
            loop {
                tokio::select! {
                    // Handle scheduler commands
                    command = scheduler_rx.recv() => {
                        match command {
                            Some(SchedulerCommand::ScheduleTask { task_id, scheduled_at, recurring }) => {
                                let scheduled_task = ScheduledTask {
                                    task_id,
                                    scheduled_at,
                                    recurring,
                                    last_executed: None,
                                    next_execution: scheduled_at,
                                };
                                
                                let mut tasks = scheduled_tasks.write().await;
                                tasks.insert(task_id, scheduled_task);
                            }
                            Some(SchedulerCommand::UnscheduleTask { task_id }) => {
                                let mut tasks = scheduled_tasks.write().await;
                                tasks.remove(&task_id);
                            }
                            Some(SchedulerCommand::UpdateSchedule { task_id, scheduled_at, recurring }) => {
                                let mut tasks = scheduled_tasks.write().await;
                                if let Some(scheduled_task) = tasks.get_mut(&task_id) {
                                    scheduled_task.scheduled_at = scheduled_at;
                                    scheduled_task.recurring = recurring;
                                    scheduled_task.next_execution = scheduled_at;
                                }
                            }
                            Some(SchedulerCommand::Shutdown) => {
                                break;
                            }
                            None => {
                                break;
                            }
                        }
                    }
                    
                    // Check for tasks to execute
                    _ = check_interval.tick() => {
                        let now = Utc::now();
                        let mut tasks_to_execute = Vec::new();
                        
                        // Find tasks ready for execution
                        {
                            let tasks = scheduled_tasks.read().await;
                            for (task_id, scheduled_task) in tasks.iter() {
                                if scheduled_task.next_execution <= now {
                                    tasks_to_execute.push(*task_id);
                                }
                            }
                        }
                        
                        // Execute ready tasks
                        for task_id in tasks_to_execute {
                            let execution_result = Self::execute_scheduled_task(
                                task_id,
                                &task_manager,
                                &conversation_integration,
                            ).await;
                            
                            // Send execution event
                            let event = TaskExecutionEvent {
                                task_id,
                                execution_time: now,
                                success: execution_result.is_ok(),
                                conversation_id: execution_result.as_ref().ok().and_then(|r| r.conversation_id),
                                error_message: execution_result.err().map(|e| e.to_string()),
                            };
                            
                            if event_tx.send(event).is_err() {
                                // Event receiver dropped, continue anyway
                            }
                            
                            // Update scheduled task for recurring execution
                            Self::update_recurring_task(task_id, &scheduled_tasks).await;
                        }
                    }
                }
                
                // Check if we should continue running
                let running = is_running.read().await;
                if !*running {
                    break;
                }
            }
        });
        
        Ok(event_rx)
    }
    
    /// Stop the scheduler
    pub async fn stop(&mut self) -> Result<()> {
        // Set running state to false
        {
            let mut running = self.is_running.write().await;
            *running = false;
        }
        
        // Send shutdown command
        if let Some(ref tx) = self.scheduler_tx {
            let _ = tx.send(SchedulerCommand::Shutdown);
        }
        
        Ok(())
    }
    
    /// Schedule a task for execution
    pub async fn schedule_task(
        &self,
        task_id: Uuid,
        scheduled_at: DateTime<Utc>,
        recurring: Option<RecurringSchedule>,
    ) -> Result<()> {
        if let Some(ref tx) = self.scheduler_tx {
            tx.send(SchedulerCommand::ScheduleTask {
                task_id,
                scheduled_at,
                recurring,
            })?;
        }
        Ok(())
    }
    
    /// Unschedule a task
    pub async fn unschedule_task(&self, task_id: Uuid) -> Result<()> {
        if let Some(ref tx) = self.scheduler_tx {
            tx.send(SchedulerCommand::UnscheduleTask { task_id })?;
        }
        Ok(())
    }
    
    /// Update a task's schedule
    pub async fn update_task_schedule(
        &self,
        task_id: Uuid,
        scheduled_at: DateTime<Utc>,
        recurring: Option<RecurringSchedule>,
    ) -> Result<()> {
        if let Some(ref tx) = self.scheduler_tx {
            tx.send(SchedulerCommand::UpdateSchedule {
                task_id,
                scheduled_at,
                recurring,
            })?;
        }
        Ok(())
    }
    
    /// Get all scheduled tasks
    pub async fn get_scheduled_tasks(&self) -> Result<Vec<ScheduledTaskInfo>> {
        let tasks = self.scheduled_tasks.read().await;
        let mut scheduled_tasks = Vec::new();
        
        for (task_id, scheduled_task) in tasks.iter() {
            if let Some(task) = self.task_manager.get_task(*task_id).await? {
                scheduled_tasks.push(ScheduledTaskInfo {
                    task_id: *task_id,
                    task_title: task.title,
                    task_type: task.task_type,
                    scheduled_at: scheduled_task.scheduled_at,
                    next_execution: scheduled_task.next_execution,
                    last_executed: scheduled_task.last_executed,
                    recurring: scheduled_task.recurring.clone(),
                });
            }
        }
        
        // Sort by next execution time
        scheduled_tasks.sort_by(|a, b| a.next_execution.cmp(&b.next_execution));
        
        Ok(scheduled_tasks)
    }
    
    /// Schedule a conversation follow-up
    pub async fn schedule_conversation_followup(
        &self,
        conversation_id: Uuid,
        followup_at: DateTime<Utc>,
        title: String,
        context: String,
    ) -> Result<Uuid> {
        // Create a conversation follow-up task
        let metadata = TaskMetadata {
            conversation_context: Some(ConversationTaskContext {
                trigger_message_id: Uuid::new_v4(), // Placeholder
                context_summary: context,
                requirements: vec!["Follow up on conversation".to_string()],
                expected_outcomes: vec!["Continued conversation".to_string()],
                branch_id: None,
                checkpoint_id: None,
            }),
            ..Default::default()
        };
        
        let _request = CreateTaskRequest {
            title,
            description: Some("Scheduled conversation follow-up".to_string()),
            task_type: TaskType::ConversationFollowUp,
            priority: TaskPriority::Normal,
            due_date: None,
            scheduled_at: Some(followup_at),
            workspace_id: None, // Could be enhanced to get from conversation
            source_conversation_id: Some(conversation_id),
            metadata,
            tags: vec!["followup".to_string()],
        };
        
        // Note: This requires mutable access to task manager
        todo!("Implement task creation - requires mutable access to task manager")
    }
    
    /// Execute a scheduled task
    async fn execute_scheduled_task(
        task_id: Uuid,
        task_manager: &Arc<dyn TaskManager>,
        conversation_integration: &Arc<ConversationTaskIntegration>,
    ) -> Result<TaskExecutionResult> {
        // Get the task
        let task = task_manager.get_task(task_id).await?
            .ok_or_else(|| anyhow::anyhow!("Scheduled task not found: {}", task_id))?;
        
        // Execute based on task type
        match task.task_type {
            TaskType::ConversationFollowUp => {
                // Create a new conversation for the follow-up
                let conversation_id = conversation_integration.create_conversation_from_task(&task).await?;
                
                Ok(TaskExecutionResult {
                    task_id,
                    success: true,
                    started_at: Utc::now(),
                    completed_at: Utc::now(),
                    conversation_id: Some(conversation_id),
                    output: Some(format!("Created follow-up conversation: {conversation_id}")),
                    error_message: None,
                    artifacts: Vec::new(),
                })
            }
            _ => {
                // For other task types, use the regular task manager execution
                // Note: This requires mutable access to task manager
                todo!("Implement task execution - requires mutable access to task manager")
            }
        }
    }
    
    /// Update recurring task for next execution
    async fn update_recurring_task(
        task_id: Uuid,
        scheduled_tasks: &Arc<RwLock<HashMap<Uuid, ScheduledTask>>>,
    ) {
        let mut tasks = scheduled_tasks.write().await;
        
        if let Some(scheduled_task) = tasks.get_mut(&task_id) {
            scheduled_task.last_executed = Some(Utc::now());
            
            if let Some(ref mut recurring) = scheduled_task.recurring {
                recurring.execution_count += 1;
                
                // Check if we should continue recurring
                let should_continue = if let Some(max_executions) = recurring.max_executions {
                    recurring.execution_count < max_executions
                } else {
                    true
                } && if let Some(end_date) = recurring.end_date {
                    Utc::now() < end_date
                } else {
                    true
                };
                
                if should_continue {
                    // Calculate next execution time
                    let next_execution = match recurring.interval {
                        ScheduleInterval::Minutes(minutes) => {
                            scheduled_task.next_execution + Duration::minutes(minutes as i64)
                        }
                        ScheduleInterval::Hours(hours) => {
                            scheduled_task.next_execution + Duration::hours(hours as i64)
                        }
                        ScheduleInterval::Days(days) => {
                            scheduled_task.next_execution + Duration::days(days as i64)
                        }
                        ScheduleInterval::Weeks(weeks) => {
                            scheduled_task.next_execution + Duration::weeks(weeks as i64)
                        }
                        ScheduleInterval::Monthly => {
                            // Approximate monthly as 30 days
                            scheduled_task.next_execution + Duration::days(30)
                        }
                        ScheduleInterval::Custom(duration) => {
                            scheduled_task.next_execution + duration
                        }
                    };
                    
                    scheduled_task.next_execution = next_execution;
                } else {
                    // Remove the task from scheduling
                    tasks.remove(&task_id);
                }
            } else {
                // Non-recurring task, remove after execution
                tasks.remove(&task_id);
            }
        }
    }
}

/// Information about a scheduled task
#[derive(Debug, Clone)]
pub struct ScheduledTaskInfo {
    pub task_id: Uuid,
    pub task_title: String,
    pub task_type: TaskType,
    pub scheduled_at: DateTime<Utc>,
    pub next_execution: DateTime<Utc>,
    pub last_executed: Option<DateTime<Utc>>,
    pub recurring: Option<RecurringSchedule>,
}

impl ScheduleInterval {
    /// Convert to chrono Duration
    pub fn to_duration(&self) -> Duration {
        match self {
            ScheduleInterval::Minutes(minutes) => Duration::minutes(*minutes as i64),
            ScheduleInterval::Hours(hours) => Duration::hours(*hours as i64),
            ScheduleInterval::Days(days) => Duration::days(*days as i64),
            ScheduleInterval::Weeks(weeks) => Duration::weeks(*weeks as i64),
            ScheduleInterval::Monthly => Duration::days(30), // Approximate
            ScheduleInterval::Custom(duration) => *duration,
        }
    }
}

impl Default for RecurringSchedule {
    fn default() -> Self {
        Self {
            interval: ScheduleInterval::Days(1),
            max_executions: None,
            end_date: None,
            execution_count: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::manager::InMemoryTaskManager;
    
    
    #[tokio::test]
    async fn test_scheduler_creation() {
        let task_manager = Arc::new(InMemoryTaskManager::new());
        // Note: We need a concrete implementation of ConversationManager
        // This is a placeholder test since InMemoryConversationManager doesn't exist yet
        // let conversation_manager = Arc::new(InMemoryConversationManager::new());
        // let conversation_integration = Arc::new(ConversationTaskIntegration::new(
        //     task_manager.clone(),
        //     conversation_manager,
        // ));
        
        // let scheduler = TaskScheduler::new(task_manager, conversation_integration);
        // assert!(!*scheduler.is_running.read().await);
    }
    
    #[tokio::test]
    async fn test_schedule_interval_conversion() {
        let interval = ScheduleInterval::Hours(2);
        let duration = interval.to_duration();
        assert_eq!(duration, Duration::hours(2));
        
        let interval = ScheduleInterval::Days(7);
        let duration = interval.to_duration();
        assert_eq!(duration, Duration::days(7));
    }
    
    #[tokio::test]
    async fn test_recurring_schedule_default() {
        let schedule = RecurringSchedule::default();
        assert_eq!(schedule.execution_count, 0);
        assert!(schedule.max_executions.is_none());
        assert!(schedule.end_date.is_none());
    }
} 