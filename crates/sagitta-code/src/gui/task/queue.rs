use egui::{Ui, RichText, Color32, ScrollArea, Response};

use super::types::{QueuedTask, TaskPanelState};
use crate::tasks::types::TaskPriority;

/// Render the task queue UI
pub fn render_task_queue(ui: &mut Ui, state: &mut TaskPanelState, _show_dialog: &mut bool) -> (bool, bool) {
    let mut add_task_clicked = false;
    let mut start_next_clicked = false;
    
    ui.horizontal(|ui| {
        ui.heading("Task Queue");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("➕ Add Task").clicked() {
                add_task_clicked = true;
            }
            
            // Start Next button is always enabled when there are pending tasks
            let start_button_enabled = !state.task_queue.pending_tasks.is_empty() && state.task_queue.active_task.is_none();
            
            let start_button = egui::Button::new("▶ Start Next")
                .fill(if start_button_enabled { ui.style().visuals.selection.bg_fill } else { ui.style().visuals.widgets.inactive.bg_fill });
            
            if ui.add_enabled(start_button_enabled, start_button).clicked() {
                start_next_clicked = true;
            }
        });
    });

    ui.separator();

    // Queue statistics
    ui.horizontal(|ui| {
        ui.label(format!("Pending: {}", state.task_queue.pending_count()));
        ui.label(format!("Completed: {}", state.task_queue.completed_count()));
        ui.label(format!("Failed: {}", state.task_queue.failed_count()));
        
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.checkbox(&mut state.auto_progress_enabled, "Auto-progress");
        });
    });

    ui.separator();

    // Active task section
    if let Some(active_task) = &state.task_queue.active_task {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.colored_label(Color32::YELLOW, "🔄 ACTIVE");
                ui.heading(&active_task.task.title);
            });
            
            if let Some(description) = &active_task.task.description {
                ui.label(description);
            }
            
            ui.horizontal(|ui| {
                ui.label(format!("Priority: {}", format_priority(active_task.task.priority)));
                ui.label(format!("Duration: {}", active_task.duration_estimate_text()));
                
                if let Some(conversation_id) = active_task.conversation_id {
                    ui.label(format!("Conversation: {}", conversation_id));
                }
            });
            
            ui.horizontal(|ui| {
                if ui.button("⏸ Pause").clicked() {
                    // TODO: Pause active task
                }
                if ui.button("❌ Cancel").clicked() {
                    // TODO: Cancel active task
                }
                if ui.button("✅ Mark Complete").clicked() {
                    // TODO: Mark task as completed
                }
            });
        });
        
        ui.separator();
    }

    // Pending tasks queue
    ui.heading("Pending Tasks");
    
    if state.task_queue.pending_tasks.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.colored_label(Color32::GRAY, "No pending tasks");
        });
    } else {
        ScrollArea::vertical()
            .max_height(300.0)
            .show(ui, |ui| {
                let mut to_remove = None;
                let mut to_move_up = None;
                let mut to_move_down = None;
                let mut to_start_now = None;
                
                for (index, task) in state.task_queue.pending_tasks.iter().enumerate() {
                    let is_selected = state.selected_task == Some(task.task.id);
                    
                    let response = render_queued_task_item(ui, task, index, is_selected);
                    
                    if response.clicked() {
                        state.selected_task = Some(task.task.id);
                    }
                    
                    response.context_menu(|ui| {
                        if ui.button("▲ Move Up").clicked() && index > 0 {
                            to_move_up = Some(index);
                            ui.close_menu();
                        }
                        
                        if ui.button("▼ Move Down").clicked() && index < state.task_queue.pending_tasks.len() - 1 {
                            to_move_down = Some(index);
                            ui.close_menu();
                        }
                        
                        ui.separator();
                        
                        let can_start_now = state.task_queue.active_task.is_none();
                        if ui.add_enabled(can_start_now, egui::Button::new("🚀 Start Now")).clicked() {
                            to_start_now = Some(index);
                            ui.close_menu();
                        }
                        
                        if ui.button("❌ Remove").clicked() {
                            to_remove = Some(index);
                            ui.close_menu();
                        }
                    });
                }
                
                // Handle queue operations
                if let Some(index) = to_remove {
                    state.task_queue.pending_tasks.remove(index);
                }
                
                if let Some(index) = to_move_up {
                    state.task_queue.pending_tasks.swap(index - 1, index);
                }
                
                if let Some(index) = to_move_down {
                    state.task_queue.pending_tasks.swap(index, index + 1);
                }
                
                if let Some(index) = to_start_now {
                    // Move the task to front and mark as starting
                    if let Some(task) = state.task_queue.pending_tasks.remove(index) {
                        state.task_queue.pending_tasks.push_front(task);
                        start_next_clicked = true;
                    }
                }
            });
    }
    
    (add_task_clicked, start_next_clicked)
}

/// Render a single queued task item
fn render_queued_task_item(ui: &mut Ui, task: &QueuedTask, index: usize, is_selected: bool) -> Response {
    let mut frame = egui::Frame::new()
        .fill(if is_selected { Color32::from_gray(40) } else { Color32::TRANSPARENT })
        .stroke(egui::Stroke::new(1.0, Color32::from_gray(60)))
        .inner_margin(egui::Margin::same(8));
    
    if is_selected {
        frame = frame.stroke(egui::Stroke::new(2.0, Color32::YELLOW));
    }
    
    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            // Queue position indicator
            ui.colored_label(Color32::GRAY, format!("#{}", index + 1));
            
            // Priority indicator
            let priority_color = match task.task.priority {
                TaskPriority::Critical => Color32::RED,
                TaskPriority::High => Color32::from_rgb(255, 165, 0), // Orange
                TaskPriority::Normal => Color32::GRAY,
                TaskPriority::Low => Color32::from_gray(120),
            };
            ui.colored_label(priority_color, format_priority_icon(task.task.priority));
            
            // Task title
            ui.heading(&task.task.title);
            
            // Auto-trigger indicator
            if task.auto_trigger {
                ui.colored_label(Color32::GREEN, "🤖");
            }
            
            // Duration estimate
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.colored_label(Color32::GRAY, &task.duration_estimate_text());
            });
        });
        
        if let Some(description) = &task.task.description {
            ui.label(RichText::new(description).size(12.0).color(Color32::LIGHT_GRAY));
        }
        
        // Task metadata
        ui.horizontal(|ui| {
            ui.label(format!("Queued: {}", task.queued_at.format("%H:%M")));
            
            if let Some(source_id) = task.task.source_conversation_id {
                ui.label(format!("From: {}", source_id.to_string().chars().take(8).collect::<String>()));
            }
            
            if !task.task.tags.is_empty() {
                ui.label(format!("Tags: {}", task.task.tags.join(", ")));
            }
        });
    }).response
}

/// Format priority as text
fn format_priority(priority: TaskPriority) -> &'static str {
    match priority {
        TaskPriority::Critical => "Critical",
        TaskPriority::High => "High",
        TaskPriority::Normal => "Normal",
        TaskPriority::Low => "Low",
    }
}

/// Format priority as icon
fn format_priority_icon(priority: TaskPriority) -> &'static str {
    match priority {
        TaskPriority::Critical => "🔥",
        TaskPriority::High => "⚡",
        TaskPriority::Normal => "📋",
        TaskPriority::Low => "📝",
    }
}

/// Render task creation dialog
pub fn render_task_creation_dialog(ui: &mut Ui, show_dialog: &mut bool) {
    if *show_dialog {
        egui::Window::new("Create New Task")
            .default_width(400.0)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                // TODO: Implement task creation form
                ui.label("Task creation dialog - TODO");
                
                ui.horizontal(|ui| {
                    if ui.button("Create").clicked() {
                        *show_dialog = false;
                    }
                    if ui.button("Cancel").clicked() {
                        *show_dialog = false;
                    }
                });
            });
    }
}

/// Render queue operations toolbar
pub fn render_queue_toolbar(ui: &mut Ui, state: &mut TaskPanelState) {
    ui.horizontal(|ui| {
        if ui.button("🗑 Clear Completed").clicked() {
            state.task_queue.completed_tasks.clear();
        }
        
        if ui.button("🔄 Retry Failed").clicked() {
            // Move failed tasks back to pending
            let failed_tasks = state.task_queue.failed_tasks.drain(..).collect::<Vec<_>>();
            for mut task in failed_tasks {
                task.completion_status = super::types::QueueTaskStatus::Queued;
                task.task.status = crate::tasks::types::TaskStatus::Pending;
                state.task_queue.pending_tasks.push_back(task);
            }
        }
        
        ui.separator();
        
        // Filter input
        ui.label("Filter:");
        ui.text_edit_singleline(&mut state.filter_text);
        
        if !state.filter_text.is_empty() && ui.button("❌").clicked() {
            state.filter_text.clear();
        }
    });
}