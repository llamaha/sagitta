use egui::{Ui, RichText, Color32, ScrollArea};
use chrono::Utc;

use super::types::{TaskPanelState, QueuedTask};

/// Render the task status overview
pub fn render_task_status(ui: &mut Ui, state: &TaskPanelState) {
    ui.heading("Task Status Overview");
    ui.separator();

    // Overall statistics
    render_statistics_cards(ui, state);
    ui.separator();

    // Recent activity
    render_recent_activity(ui, state);
    ui.separator();

    // Progress tracking
    render_progress_tracking(ui, state);
}

/// Render statistics cards
fn render_statistics_cards(ui: &mut Ui, state: &TaskPanelState) {
    ui.columns(4, |columns| {
        // Pending tasks card
        columns[0].group(|ui| {
            ui.vertical_centered(|ui| {
                ui.colored_label(Color32::YELLOW, RichText::new(format!("{}", state.task_queue.pending_count())).size(24.0));
                ui.label("Pending");
            });
        });

        // Active task card
        columns[1].group(|ui| {
            ui.vertical_centered(|ui| {
                let active_count = if state.task_queue.active_task.is_some() { 1 } else { 0 };
                ui.colored_label(Color32::GREEN, RichText::new(format!("{}", active_count)).size(24.0));
                ui.label("Active");
            });
        });

        // Completed tasks card
        columns[2].group(|ui| {
            ui.vertical_centered(|ui| {
                ui.colored_label(Color32::BLUE, RichText::new(format!("{}", state.task_queue.completed_count())).size(24.0));
                ui.label("Completed");
            });
        });

        // Failed tasks card
        columns[3].group(|ui| {
            ui.vertical_centered(|ui| {
                ui.colored_label(Color32::RED, RichText::new(format!("{}", state.task_queue.failed_count())).size(24.0));
                ui.label("Failed");
            });
        });
    });
}

/// Render recent activity timeline
fn render_recent_activity(ui: &mut Ui, state: &TaskPanelState) {
    ui.heading("Recent Activity");
    
    ScrollArea::vertical()
        .max_height(200.0)
        .show(ui, |ui| {
            let mut all_tasks = Vec::new();
            
            // Collect all tasks with timestamps
            if let Some(active_task) = &state.task_queue.active_task {
                all_tasks.push((active_task, "active"));
            }
            
            for task in &state.task_queue.completed_tasks {
                all_tasks.push((task, "completed"));
            }
            
            for task in &state.task_queue.failed_tasks {
                all_tasks.push((task, "failed"));
            }
            
            // Sort by most recent activity
            all_tasks.sort_by_key(|(task, _)| {
                task.task.updated_at
            });
            all_tasks.reverse();
            
            if all_tasks.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.colored_label(Color32::GRAY, "No recent activity");
                });
            } else {
                for (task, status) in all_tasks.iter().take(10) {
                    render_activity_item(ui, task, status);
                }
            }
        });
}

/// Render a single activity item
fn render_activity_item(ui: &mut Ui, task: &QueuedTask, status: &str) {
    ui.horizontal(|ui| {
        // Status indicator
        let (icon, color) = match status {
            "active" => ("🔄", Color32::YELLOW),
            "completed" => ("✅", Color32::GREEN),
            "failed" => ("❌", Color32::RED),
            _ => ("📋", Color32::GRAY),
        };
        
        ui.colored_label(color, icon);
        
        // Task info
        ui.label(&task.task.title);
        
        // Timestamp
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let time_text = match status {
                "active" => {
                    if let Some(started_at) = task.started_at {
                        format!("Started {}", format_relative_time(started_at))
                    } else {
                        "Just started".to_string()
                    }
                }
                "completed" => {
                    if let Some(completed_at) = task.task.completed_at {
                        format!("Completed {}", format_relative_time(completed_at))
                    } else {
                        "Recently completed".to_string()
                    }
                }
                "failed" => {
                    format!("Failed {}", format_relative_time(task.task.updated_at))
                }
                _ => format_relative_time(task.task.updated_at),
            };
            
            ui.colored_label(Color32::GRAY, time_text);
        });
    });
    
    // Show description if available
    if let Some(description) = &task.task.description {
        ui.horizontal(|ui| {
            ui.add_space(20.0); // Indent
            ui.label(RichText::new(description).size(11.0).color(Color32::LIGHT_GRAY));
        });
    }
    
    ui.separator();
}

/// Render progress tracking section
fn render_progress_tracking(ui: &mut Ui, state: &TaskPanelState) {
    ui.heading("Progress Tracking");
    
    // Auto-progress settings
    ui.horizontal(|ui| {
        ui.checkbox(&mut state.auto_progress_enabled.clone(), "Auto-progress enabled");
        
        if state.auto_progress_enabled {
            ui.colored_label(Color32::GREEN, "Tasks will automatically progress when completed");
        } else {
            ui.colored_label(Color32::GRAY, "Manual task progression required");
        }
    });
    
    // Completion criteria summary
    ui.group(|ui| {
        ui.label("Completion Criteria:");
        
        let criteria = &state.completion_criteria;
        
        ui.horizontal(|ui| {
            if criteria.require_tests_pass {
                ui.colored_label(Color32::GREEN, "✅ Tests must pass");
            } else {
                ui.colored_label(Color32::GRAY, "⏭ Tests optional");
            }
            
            if criteria.check_lint_errors {
                ui.colored_label(Color32::GREEN, "✅ No lint errors");
            } else {
                ui.colored_label(Color32::GRAY, "⏭ Lint optional");
            }
        });
        
        if let Some(timeout) = criteria.timeout_minutes {
            ui.label(format!("⏱ Timeout: {} minutes", timeout));
        }
        
        if !criteria.completion_keywords.is_empty() {
            ui.label(format!("🔍 Completion keywords: {}", criteria.completion_keywords.join(", ")));
        }
    });
    
    // Active task progress
    if let Some(active_task) = &state.task_queue.active_task {
        ui.group(|ui| {
            ui.label("Active Task Progress:");
            
            ui.horizontal(|ui| {
                ui.label(&active_task.task.title);
                
                if let Some(started_at) = active_task.started_at {
                    let elapsed = Utc::now().signed_duration_since(started_at);
                    ui.label(format!("Running for: {}", format_duration(elapsed.to_std().unwrap_or_default())));
                }
            });
            
            // Progress indicators
            if let Some(conversation_id) = active_task.conversation_id {
                ui.horizontal(|ui| {
                    ui.label("Conversation:");
                    ui.colored_label(Color32::BLUE, format!("{}", conversation_id.to_string().chars().take(8).collect::<String>()));
                    
                    if ui.button("📂 View").clicked() {
                        // TODO: Switch to conversation
                    }
                });
            }
            
            // Manual completion controls
            ui.horizontal(|ui| {
                if ui.button("✅ Mark Complete").clicked() {
                    // TODO: Manually complete task
                }
                
                if ui.button("❌ Mark Failed").clicked() {
                    // TODO: Manually fail task
                }
                
                if ui.button("⏸ Pause").clicked() {
                    // TODO: Pause task
                }
            });
        });
    }
}

/// Format relative time (e.g., "2 minutes ago")
fn format_relative_time(time: chrono::DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(time);
    
    if duration.num_seconds() < 60 {
        "just now".to_string()
    } else if duration.num_minutes() < 60 {
        format!("{} min ago", duration.num_minutes())
    } else if duration.num_hours() < 24 {
        format!("{} hr ago", duration.num_hours())
    } else {
        format!("{} days ago", duration.num_days())
    }
}

/// Format duration in a human-readable way
fn format_duration(duration: std::time::Duration) -> String {
    let total_seconds = duration.as_secs();
    
    if total_seconds < 60 {
        format!("{}s", total_seconds)
    } else if total_seconds < 3600 {
        format!("{}m {}s", total_seconds / 60, total_seconds % 60)
    } else {
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        format!("{}h {}m", hours, minutes)
    }
}

/// Render task completion settings
pub fn render_completion_settings(ui: &mut Ui, state: &mut TaskPanelState) {
    ui.heading("Task Completion Settings");
    ui.separator();
    
    let criteria = &mut state.completion_criteria;
    
    ui.checkbox(&mut criteria.require_tests_pass, "Require tests to pass");
    ui.checkbox(&mut criteria.require_explicit_completion, "Require explicit completion message");
    ui.checkbox(&mut criteria.check_lint_errors, "Check for lint errors");
    
    ui.horizontal(|ui| {
        ui.label("Timeout (minutes):");
        if let Some(ref mut timeout) = criteria.timeout_minutes {
            ui.add(egui::DragValue::new(timeout).range(1..=480).suffix(" min"));
        } else {
            if ui.button("Set timeout").clicked() {
                criteria.timeout_minutes = Some(60);
            }
        }
        
        if criteria.timeout_minutes.is_some() && ui.button("Remove timeout").clicked() {
            criteria.timeout_minutes = None;
        }
    });
    
    ui.separator();
    
    // Completion keywords
    ui.label("Completion Keywords (one per line):");
    let mut keywords_text = criteria.completion_keywords.join("\n");
    if ui.text_edit_multiline(&mut keywords_text).changed() {
        criteria.completion_keywords = keywords_text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .collect();
    }
    
    ui.separator();
    
    // Failure keywords
    ui.label("Failure Keywords (one per line):");
    let mut failure_keywords_text = criteria.failure_keywords.join("\n");
    if ui.text_edit_multiline(&mut failure_keywords_text).changed() {
        criteria.failure_keywords = failure_keywords_text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .collect();
    }
}