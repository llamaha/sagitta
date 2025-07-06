use std::sync::Arc;
use egui::{Context, ScrollArea, Frame, Vec2};
use crate::gui::theme::AppTheme;

/// Panel type enum for tracking which panel is currently open
#[derive(Debug, Clone, PartialEq)]
pub enum ActivePanel {
    None,
    Repository,
    Preview,
    Settings,
    Conversation,
    Events,
}

/// Preview panel for tool outputs and code changes
pub struct PreviewPanel {
    pub visible: bool,
    pub content: String,
    pub title: String,
}

impl PreviewPanel {
    pub fn new() -> Self {
        Self {
            visible: false,
            content: String::new(),
            title: String::new(),
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn set_content(&mut self, title: &str, content: &str) {
        self.title = title.to_string();
        self.content = content.to_string();
    }

    pub fn render(&mut self, ctx: &Context, theme: crate::gui::theme::AppTheme) {
        if !self.visible {
            return;
        }

        egui::SidePanel::right("preview_panel")
            .resizable(true)
            .default_width(400.0)
            .frame(theme.side_panel_frame())
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.heading("Preview Panel");
                    ui.separator();
                    
                    ui.horizontal(|ui| {
                        ui.heading(&self.title);
                        ui.add_space(8.0);
                        if ui.button("×").clicked() {
                            self.visible = false;
                        }
                    });
                    ui.separator();
                    
                    // Content area with scrolling
                    ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.label(&self.content);
                        });
                });
            });
    }
}

/// Logging panel for displaying Sagitta Code logs
pub struct LoggingPanel {
    pub visible: bool,
    pub logs: Vec<(std::time::SystemTime, String)>, // (timestamp, log line)
    pub filter_sagitta_code_only: bool,
}

impl LoggingPanel {
    pub fn new() -> Self {
        Self {
            visible: false,
            logs: Vec::new(),
            filter_sagitta_code_only: true,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn add_log(&mut self, line: String) {
        self.logs.push((std::time::SystemTime::now(), line));
        // Keep only the last 1000 log lines for memory
        if self.logs.len() > 1000 {
            self.logs.drain(0..(self.logs.len() - 1000));
        }
    }

    pub fn get_recent_logs(&self, seconds: u64) -> String {
        let now = std::time::SystemTime::now();
        self.logs.iter()
            .rev()
            .take_while(|(ts, _)| now.duration_since(*ts).unwrap_or_default().as_secs() < seconds)
            .map(|(_, line)| line.clone())
            .collect::<Vec<_>>()
            .into_iter().rev().collect::<Vec<_>>().join("\n")
    }

    pub fn render(&mut self, ctx: &Context) {
        if !self.visible {
            return;
        }
        egui::SidePanel::right("logging_panel")
            .resizable(true)
            .default_width(500.0)
            .frame(crate::gui::theme::AppTheme::default().side_panel_frame())
            .show(ctx, |ui| {
                ui.heading("Sagitta Code Logs");
                ui.horizontal(|ui| {
                    if ui.button("Copy 10s").clicked() {
                        let logs = self.get_recent_logs(10);
                        ui.ctx().copy_text(logs);
                    }
                    if ui.button("Copy 30s").clicked() {
                        let logs = self.get_recent_logs(30);
                        ui.ctx().copy_text(logs);
                    }
                    if ui.button("Copy 60s").clicked() {
                        let logs = self.get_recent_logs(60);
                        ui.ctx().copy_text(logs);
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (_, line) in self.logs.iter().rev().take(200).rev() {
                        ui.label(line);
                    }
                });
            });
    }
}

/// System event types for the events panel
#[derive(Debug, Clone)]
pub enum SystemEventType {
    ToolExecution,
    StateChange,
    Error,
    Info,
}

/// System event for tracking application events
#[derive(Debug, Clone)]
pub struct SystemEvent {
    pub timestamp: std::time::SystemTime,
    pub event_type: SystemEventType,
    pub message: String,
}

/// Events panel for displaying system events
pub struct EventsPanel {
    pub visible: bool,
    pub events: Vec<SystemEvent>,
    pub max_events: usize,
}

impl EventsPanel {
    pub fn new() -> Self {
        Self {
            visible: false,
            events: Vec::new(),
            max_events: 100,
        }
    }
    
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }
    
    pub fn add_event(&mut self, event_type: SystemEventType, message: String) {
        let event = SystemEvent {
            timestamp: std::time::SystemTime::now(),
            event_type,
            message,
        };
        
        self.events.push(event);
        
        // Keep only the most recent events
        if self.events.len() > self.max_events {
            self.events.remove(0);
        }
    }
    
    pub fn render(&mut self, ctx: &egui::Context, theme: crate::gui::theme::AppTheme) {
        if !self.visible {
            return;
        }
        
        egui::Window::new("🔔 System Events")
            .default_width(400.0)
            .default_height(300.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Recent system events and tool executions");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Clear").clicked() {
                            self.events.clear();
                        }
                    });
                });
                
                ui.separator();
                
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        for event in self.events.iter().rev() {
                            let time_str = event.timestamp
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| {
                                    let secs = d.as_secs();
                                    let now = std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_secs();
                                    let elapsed = now.saturating_sub(secs);
                                    
                                    if elapsed < 60 {
                                        format!("{}s ago", elapsed)
                                    } else if elapsed < 3600 {
                                        format!("{}m ago", elapsed / 60)
                                    } else {
                                        format!("{}h ago", elapsed / 3600)
                                    }
                                })
                                .unwrap_or_else(|_| "unknown".to_string());
                            
                            let (icon, color) = match event.event_type {
                                SystemEventType::ToolExecution => ("🔧", theme.accent_color()),
                                SystemEventType::StateChange => ("🔄", theme.success_color()),
                                SystemEventType::Error => ("❌", theme.error_color()),
                                SystemEventType::Info => ("ℹ️", theme.hint_text_color()),
                            };
                            
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(icon).color(color));
                                ui.label(egui::RichText::new(&time_str).small().weak());
                                ui.label(&event.message);
                            });
                            
                            ui.add_space(2.0);
                        }
                        
                        if self.events.is_empty() {
                            ui.centered_and_justified(|ui| {
                                ui.label(egui::RichText::new("No events yet").weak());
                            });
                        }
                    });
            });
    }
} 