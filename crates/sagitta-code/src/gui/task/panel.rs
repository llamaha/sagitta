use std::sync::Arc;
use anyhow::Result;
use egui::{Context, SidePanel, Ui};
use tokio::sync::Mutex;
use uuid::Uuid;

use super::types::{TaskPanelState, TaskPanelTab, QueuedTask};
use super::queue::{render_task_queue, render_queue_toolbar};
use super::status::{render_task_status, render_completion_settings};
use crate::tasks::types::{Task, TaskType, TaskPriority, TaskStatus, TaskMetadata};
use crate::tasks::manager::TaskManager;
use crate::agent::conversation::manager::ConversationManager;
use crate::config::types::SagittaCodeConfig;
use crate::gui::theme::AppTheme;
use crate::llm::fast_model::FastModelProvider;
use chrono::Utc;

/// Task management panel
pub struct TaskPanel {
    state: Arc<Mutex<TaskPanelState>>,
    task_manager: Option<Arc<dyn TaskManager>>,
    conversation_manager: Option<Arc<dyn ConversationManager>>,
    config: Arc<Mutex<SagittaCodeConfig>>,
    fast_model: Arc<Mutex<Option<FastModelProvider>>>,
    is_open: bool,
    show_task_creation_dialog: bool,
    task_creation_form: TaskCreationForm,
}

/// Task creation form state
#[derive(Debug, Clone, Default)]
struct TaskCreationForm {
    title: String,
    description: String,
    priority: TaskPriority,
    auto_trigger: bool,
    estimated_hours: String,
    tags: String,
}

impl TaskPanel {
    /// Create a new task panel
    pub fn new(
        task_manager: Option<Arc<dyn TaskManager>>,
        conversation_manager: Option<Arc<dyn ConversationManager>>,
        config: Arc<Mutex<SagittaCodeConfig>>,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(TaskPanelState::default())),
            task_manager,
            conversation_manager,
            config: config.clone(),
            fast_model: Arc::new(Mutex::new(None)),
            is_open: false,
            show_task_creation_dialog: false,
            task_creation_form: TaskCreationForm::default(),
        }
    }

    /// Initialize the fast model provider
    pub async fn initialize_fast_model(&self) -> Result<()> {
        let config = self.config.lock().await;
        if config.conversation.enable_fast_model {
            let mut fast_model_provider = FastModelProvider::new(config.clone());
            fast_model_provider.initialize().await.map_err(|e| anyhow::anyhow!("Failed to initialize fast model: {}", e))?;
            
            let mut fast_model_guard = self.fast_model.lock().await;
            *fast_model_guard = Some(fast_model_provider);
            log::info!("TaskPanel: Fast model initialized for task completion detection");
        }
        Ok(())
    }

    /// Toggle panel visibility
    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }

    /// Set panel visibility
    pub fn set_open(&mut self, open: bool) {
        self.is_open = open;
    }

    /// Check if panel is open
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Show the task panel
    pub fn show(&mut self, ctx: &Context, theme: AppTheme) {
        if !self.is_open {
            return;
        }

        SidePanel::right("task_panel")
            .default_width(400.0)
            .width_range(300.0..=600.0)
            .resizable(true)
            .frame(egui::Frame::NONE
                .fill(theme.panel_background())
                .inner_margin(egui::Margin::same(10)))
            .show(ctx, |ui| {
                self.render_panel_content(ui, theme);
            });

        // Render dialogs
        if self.show_task_creation_dialog {
            self.render_task_creation_dialog(ctx, theme);
        }
    }

    /// Render the main panel content
    fn render_panel_content(&mut self, ui: &mut Ui, theme: AppTheme) {
        // Apply theme styles using proper theme methods
        let header_color = theme.panel_background();
        let text_color = theme.text_color();

        // Panel header
        ui.horizontal(|ui| {
            ui.colored_label(header_color, "🗂");
            ui.heading(egui::RichText::new("Task Management").color(text_color));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("❌").clicked() {
                    self.is_open = false;
                }
            });
        });
        
        ui.separator();

        // Tab selection
        let mut state = match self.state.try_lock() {
            Ok(guard) => guard.clone(),
            Err(_) => {
                ui.colored_label(egui::Color32::YELLOW, "Loading task state...");
                return;
            }
        };

        ui.horizontal(|ui| {
            ui.selectable_value(&mut state.active_tab, TaskPanelTab::Queue, "📋 Queue");
            ui.selectable_value(&mut state.active_tab, TaskPanelTab::Active, "🔄 Active");
            ui.selectable_value(&mut state.active_tab, TaskPanelTab::Completed, "✅ Completed");
            ui.selectable_value(&mut state.active_tab, TaskPanelTab::Settings, "⚙ Settings");
        });

        ui.separator();

        // Tab content
        match state.active_tab {
            TaskPanelTab::Queue => {
                let (add_task_clicked, start_next_clicked) = render_task_queue(ui, &mut state, &mut self.show_task_creation_dialog);
                
                if add_task_clicked {
                    // "Add Task" button was clicked
                    self.show_task_creation_dialog = true;
                }
                
                if start_next_clicked {
                    // "Start Next" or "Start Now" button was clicked
                    let state_arc = Arc::clone(&self.state);
                    let task_manager = self.task_manager.clone();
                    let conversation_manager = self.conversation_manager.clone();
                    
                    tokio::spawn(async move {
                        let mut guard = state_arc.lock().await;
                        if let Some(task) = guard.task_queue.start_next_task() {
                            drop(guard); // Release the lock before async operations
                            
                            // Create a new conversation for this task if conversation manager is available
                            if let Some(conv_manager) = conversation_manager {
                                let conversation_title = format!("Task: {}", task.task.title);
                                let workspace_id = task.task.workspace_id.unwrap_or_else(|| uuid::Uuid::new_v4());
                                
                                match conv_manager.create_conversation(conversation_title, Some(workspace_id)).await {
                                    Ok(conversation_id) => {
                                        // Update the task with the conversation ID
                                        let mut guard = state_arc.lock().await;
                                        if let Some(active_task) = &mut guard.task_queue.active_task {
                                            active_task.conversation_id = Some(conversation_id);
                                        }
                                        log::info!("Started task '{}' with conversation {}", task.task.title, conversation_id);
                                    }
                                    Err(e) => {
                                        // If conversation creation fails, fail the task
                                        let mut guard = state_arc.lock().await;
                                        guard.task_queue.fail_active_task(format!("Failed to create conversation: {}", e));
                                        log::error!("Failed to start task '{}': {}", task.task.title, e);
                                    }
                                }
                            } else {
                                log::info!("Started task '{}' (no conversation manager available)", task.task.title);
                            }
                        }
                    });
                }
                
                render_queue_toolbar(ui, &mut state);
            }
            TaskPanelTab::Active => {
                self.render_active_tasks(ui, &mut state, theme);
            }
            TaskPanelTab::Completed => {
                self.render_completed_tasks(ui, &mut state, theme);
            }
            TaskPanelTab::Settings => {
                render_completion_settings(ui, &mut state);
            }
        }

        // Update state asynchronously to avoid blocking the UI
        let state_arc = Arc::clone(&self.state);
        tokio::spawn(async move {
            let mut guard = state_arc.lock().await;
            *guard = state;
        });
    }

    /// Render active tasks tab
    fn render_active_tasks(&mut self, ui: &mut Ui, state: &mut TaskPanelState, _theme: AppTheme) {
        render_task_status(ui, state);
    }

    /// Render completed tasks tab
    fn render_completed_tasks(&mut self, ui: &mut Ui, state: &mut TaskPanelState, _theme: AppTheme) {
        ui.heading("Completed Tasks");
        
        ui.horizontal(|ui| {
            ui.checkbox(&mut state.show_completed, "Show completed tasks");
            if ui.button("🗑 Clear All").clicked() {
                state.task_queue.completed_tasks.clear();
            }
        });
        
        ui.separator();

        if state.task_queue.completed_tasks.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.colored_label(egui::Color32::GRAY, "No completed tasks");
            });
        } else {
            egui::ScrollArea::vertical().show(ui, |ui| {
                for task in &state.task_queue.completed_tasks {
                    self.render_completed_task_item(ui, task);
                }
            });
        }
        
        ui.separator();
        
        if !state.task_queue.failed_tasks.is_empty() {
            ui.heading("Failed Tasks");
            
            egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                for task in &state.task_queue.failed_tasks {
                    self.render_failed_task_item(ui, task);
                }
            });
        }
    }

    /// Render a completed task item
    fn render_completed_task_item(&self, ui: &mut Ui, task: &QueuedTask) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.colored_label(egui::Color32::GREEN, "✅");
                ui.heading(&task.task.title);
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(completed_at) = task.task.completed_at {
                        ui.label(format!("{}", completed_at.format("%Y-%m-%d %H:%M")));
                    }
                });
            });
            
            if let Some(description) = &task.task.description {
                ui.label(description);
            }
            
            ui.horizontal(|ui| {
                if let Some(duration) = task.estimated_duration {
                    ui.label(format!("Duration: {}", format_duration(duration)));
                }
                
                if let Some(_conversation_id) = task.conversation_id {
                    if ui.button("📂 View Conversation").clicked() {
                        // TODO: Switch to conversation
                    }
                }
            });
        });
    }

    /// Render a failed task item
    fn render_failed_task_item(&self, ui: &mut Ui, task: &QueuedTask) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.colored_label(egui::Color32::RED, "❌");
                ui.heading(&task.task.title);
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("🔄 Retry").clicked() {
                        // TODO: Move task back to queue
                    }
                });
            });
            
            if let Some(description) = &task.task.description {
                ui.label(description);
            }
            
            // Show failure reason if available
            if let Some(reason) = task.task.metadata.custom_fields.get("failure_reason") {
                ui.colored_label(egui::Color32::RED, format!("Reason: {}", reason));
            }
        });
    }

    /// Render task creation dialog
    fn render_task_creation_dialog(&mut self, ctx: &Context, _theme: AppTheme) {
        egui::Window::new("Create New Task")
            .default_width(500.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    // Title
                    ui.horizontal(|ui| {
                        ui.label("Title:");
                        ui.text_edit_singleline(&mut self.task_creation_form.title);
                    });
                    
                    // Description
                    ui.label("Description:");
                    ui.text_edit_multiline(&mut self.task_creation_form.description);
                    
                    // Priority
                    ui.horizontal(|ui| {
                        ui.label("Priority:");
                        egui::ComboBox::from_id_salt("task_priority")
                            .selected_text(format!("{:?}", self.task_creation_form.priority))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.task_creation_form.priority, TaskPriority::Low, "Low");
                                ui.selectable_value(&mut self.task_creation_form.priority, TaskPriority::Normal, "Normal");
                                ui.selectable_value(&mut self.task_creation_form.priority, TaskPriority::High, "High");
                                ui.selectable_value(&mut self.task_creation_form.priority, TaskPriority::Critical, "Critical");
                            });
                    });
                    
                    // Auto-trigger
                    ui.checkbox(&mut self.task_creation_form.auto_trigger, "Auto-trigger when previous task completes");
                    
                    // Estimated time
                    ui.horizontal(|ui| {
                        ui.label("Estimated hours:");
                        ui.text_edit_singleline(&mut self.task_creation_form.estimated_hours);
                    });
                    
                    // Tags
                    ui.horizontal(|ui| {
                        ui.label("Tags (comma-separated):");
                        ui.text_edit_singleline(&mut self.task_creation_form.tags);
                    });
                    
                    ui.separator();
                    
                    // Buttons
                    ui.horizontal(|ui| {
                        if ui.button("Create Task").clicked() {
                            self.create_task_from_form();
                            self.show_task_creation_dialog = false;
                        }
                        
                        if ui.button("Cancel").clicked() {
                            self.show_task_creation_dialog = false;
                            self.task_creation_form = TaskCreationForm::default();
                        }
                    });
                });
            });
    }

    /// Create a task from the form data
    fn create_task_from_form(&mut self) {
        let form = &self.task_creation_form;
        
        if form.title.trim().is_empty() {
            return;
        }
        
        let estimated_hours = form.estimated_hours.parse::<f32>().ok();
        let estimated_duration = estimated_hours.map(|hours| {
            std::time::Duration::from_secs((hours * 3600.0) as u64)
        });
        
        let tags: Vec<String> = form.tags
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        
        let task = Task {
            id: Uuid::new_v4(),
            title: form.title.clone(),
            description: if form.description.trim().is_empty() {
                None
            } else {
                Some(form.description.clone())
            },
            task_type: TaskType::Custom("User Created".to_string()),
            priority: form.priority,
            status: TaskStatus::Pending,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            due_date: None,
            scheduled_at: None,
            completed_at: None,
            source_conversation_id: None,
            target_conversation_id: None,
            workspace_id: None,
            metadata: TaskMetadata {
                estimated_hours,
                ..Default::default()
            },
            dependencies: Vec::new(),
            tags,
        };
        
        let queued_task = QueuedTask::new(task, form.auto_trigger)
            .with_estimated_duration(estimated_duration.unwrap_or_else(|| std::time::Duration::from_secs(3600)));
        
        // Add task to queue asynchronously
        let state_arc = Arc::clone(&self.state);
        tokio::spawn(async move {
            let mut guard = state_arc.lock().await;
            guard.task_queue.add_task(queued_task);
        });
        
        // Reset form
        self.task_creation_form = TaskCreationForm::default();
    }

    /// Add a task from a conversation
    pub async fn add_task_from_conversation(
        &self,
        title: String,
        description: Option<String>,
        conversation_id: Uuid,
        auto_trigger: bool,
    ) -> Result<Uuid> {
        let task = Task {
            id: Uuid::new_v4(),
            title,
            description,
            task_type: TaskType::ConversationFollowUp,
            priority: TaskPriority::Normal,
            status: TaskStatus::Pending,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            due_date: None,
            scheduled_at: None,
            completed_at: None,
            source_conversation_id: Some(conversation_id),
            target_conversation_id: None,
            workspace_id: None,
            metadata: TaskMetadata::default(),
            dependencies: Vec::new(),
            tags: Vec::new(),
        };
        
        let task_id = task.id;
        let queued_task = QueuedTask::new(task, auto_trigger);
        
        self.state.lock().await.task_queue.add_task(queued_task);
        
        Ok(task_id)
    }

    /// Start the next task in the queue
    pub async fn start_next_task(&self) -> Result<Option<Uuid>> {
        let mut state = self.state.lock().await;
        
        if let Some(task) = state.task_queue.start_next_task() {
            let task_id = task.task.id;
            
            // Create a new conversation for this task
            if let Some(conversation_manager) = &self.conversation_manager {
                let conversation_title = format!("Task: {}", task.task.title);
                let workspace_id = task.task.workspace_id.unwrap_or_else(|| Uuid::new_v4());
                
                match conversation_manager.create_conversation(conversation_title, Some(workspace_id)).await {
                    Ok(conversation_id) => {
                        // Update the task with the conversation ID
                        if let Some(active_task) = &mut state.task_queue.active_task {
                            active_task.conversation_id = Some(conversation_id);
                        }
                        
                        return Ok(Some(task_id));
                    }
                    Err(e) => {
                        // If conversation creation fails, fail the task
                        state.task_queue.fail_active_task(format!("Failed to create conversation: {}", e));
                        return Err(e.into());
                    }
                }
            }
            
            Ok(Some(task_id))
        } else {
            Ok(None)
        }
    }

    /// Mark the active task as completed
    pub async fn complete_active_task(&self) -> Result<Option<Uuid>> {
        let mut state = self.state.lock().await;
        
        if let Some(task) = state.task_queue.complete_active_task() {
            let task_id = task.task.id;
            
            // If auto-progress is enabled and there are more tasks, start the next one
            if state.auto_progress_enabled && !state.task_queue.pending_tasks.is_empty() {
                drop(state); // Release the lock before the async call
                self.start_next_task().await?;
            }
            
            Ok(Some(task_id))
        } else {
            Ok(None)
        }
    }

    /// Get the current task queue state
    pub async fn get_queue_state(&self) -> TaskPanelState {
        self.state.lock().await.clone()
    }

    /// Check if a conversation stream has ended and decide if task is complete using fast-model
    pub async fn check_conversation_completion(&self, conversation_content: &str) -> Result<bool> {
        // First check for explicit completion words
        let completion_words = ["completed", "finished", "done", "success"];
        let content_lower = conversation_content.to_lowercase();
        
        for word in completion_words {
            if content_lower.contains(word) {
                log::info!("TaskPanel: Found explicit completion word: {}", word);
                return Ok(true);
            }
        }
        
        // Use fast-model for intelligent completion detection if available
        if let Some(fast_model) = self.fast_model.lock().await.as_ref() {
            let prompt = format!(
                "Analyze this conversation output and determine if the task appears to be completed successfully.\n\
                Look for:\n\
                - Implementation finished\n\
                - Tests passing\n\
                - No errors or failures\n\
                - User satisfaction\n\
                - Clear completion indicators\n\n\
                Respond with only 'COMPLETE' or 'INCOMPLETE' followed by confidence (0.0-1.0).\n\
                Format: COMPLETE:0.9 or INCOMPLETE:0.3\n\n\
                Conversation:\n{}", 
                conversation_content.chars().take(2000).collect::<String>()
            );
            
            match fast_model.generate_simple_text(&prompt).await {
                Ok(response) => {
                    let response = response.trim().to_uppercase();
                    if let Some((status, confidence_str)) = response.split_once(':') {
                        if status == "COMPLETE" {
                            if let Ok(confidence) = confidence_str.parse::<f32>() {
                                if confidence >= 0.7 {
                                    log::info!("TaskPanel: Fast-model detected completion with confidence: {}", confidence);
                                    return Ok(true);
                                } else {
                                    log::info!("TaskPanel: Fast-model detected completion but low confidence: {}", confidence);
                                }
                            }
                        }
                    }
                    log::info!("TaskPanel: Fast-model determined task incomplete: {}", response);
                }
                Err(e) => {
                    log::warn!("TaskPanel: Fast-model completion check failed: {}", e);
                }
            }
        }
        
        Ok(false)
    }

    /// Auto-progress to next task if enabled and current task is complete
    pub async fn handle_stream_completion(&self, conversation_content: &str) -> Result<()> {
        let state = self.state.lock().await;
        if !state.auto_progress_enabled {
            return Ok(());
        }
        drop(state);

        if self.check_conversation_completion(conversation_content).await? {
            log::info!("TaskPanel: Task completion detected, marking as complete and starting next task");
            
            if let Some(completed_task_id) = self.complete_active_task().await? {
                log::info!("TaskPanel: Completed task {} and automatically started next task", completed_task_id);
            }
        }
        
        Ok(())
    }
}

/// Format duration in a human-readable way
fn format_duration(duration: std::time::Duration) -> String {
    let total_seconds = duration.as_secs();
    
    if total_seconds < 60 {
        format!("{}s", total_seconds)
    } else if total_seconds < 3600 {
        format!("{}m", total_seconds / 60)
    } else {
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        if minutes > 0 {
            format!("{}h {}m", hours, minutes)
        } else {
            format!("{}h", hours)
        }
    }
}