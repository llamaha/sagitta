// Panel management for the Sagitta Code application

use std::sync::Arc;
use egui::{Context, ScrollArea, Window, RichText, Color32, TextEdit, ComboBox, Button, Vec2, Rounding};
use egui_plot::{Line, Plot, Bar, BarChart, Legend, PlotPoints};
use super::super::theme::AppTheme;
use super::super::theme_customizer::ThemeCustomizer;
use crate::agent::conversation::types::ProjectType;
use crate::agent::conversation::analytics::AnalyticsReport;
use std::path::PathBuf;

/// Panel type enum for tracking which panel is currently open
#[derive(Debug, Clone, PartialEq)]
pub enum ActivePanel {
    None,
    Repository,
    Preview,
    Settings,
    Conversation,
    Events,
    Analytics,
    ThemeCustomizer,
    ModelSelection,
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

    pub fn render(&mut self, ctx: &Context, theme: AppTheme) {
        if !self.visible {
            return;
        }

        egui::SidePanel::right("preview_panel")
            .resizable(true)
            .default_width(400.0)
            .frame(egui::Frame::none().fill(theme.panel_background()))
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

    pub fn render(&mut self, ctx: &egui::Context, theme: AppTheme) {
        if !self.visible {
            return;
        }
        egui::SidePanel::right("logging_panel")
            .resizable(true)
            .default_width(500.0)
            .frame(egui::Frame::none().fill(theme.panel_background()))
            .show(ctx, |ui| {
                ui.heading("Sagitta Code Logs");
                ui.horizontal(|ui| {
                    if ui.button("Copy 10s").clicked() {
                        let logs = self.get_recent_logs(10);
                        ui.output_mut(|o| o.copied_text = logs);
                    }
                    if ui.button("Copy 30s").clicked() {
                        let logs = self.get_recent_logs(30);
                        ui.output_mut(|o| o.copied_text = logs);
                    }
                    if ui.button("Copy 60s").clicked() {
                        let logs = self.get_recent_logs(60);
                        ui.output_mut(|o| o.copied_text = logs);
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

/// System event structure
#[derive(Debug, Clone)]
pub struct SystemEvent {
    pub timestamp: std::time::SystemTime,
    pub event_type: SystemEventType,
    pub message: String,
}

/// Events panel for displaying system events and tool executions
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
    
    pub fn render(&mut self, ctx: &egui::Context, theme: AppTheme) {
        if !self.visible {
            return;
        }
        
        egui::Window::new("🔔 System Events")
            .default_width(400.0)
            .default_height(300.0)
            .resizable(true)
            .frame(egui::Frame::none().fill(theme.panel_background()))
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

/// Panel manager for coordinating all panels
pub struct PanelManager {
    pub active_panel: ActivePanel,
    pub preview_panel: PreviewPanel,
    pub logging_panel: LoggingPanel,
    pub events_panel: EventsPanel,
    pub analytics_panel: AnalyticsPanel,
    pub theme_customizer: ThemeCustomizer,
    pub model_selection_panel: ModelSelectionPanel,
}

impl PanelManager {
    pub fn new() -> Self {
        Self {
            active_panel: ActivePanel::None,
            preview_panel: PreviewPanel::new(),
            logging_panel: LoggingPanel::new(),
            events_panel: EventsPanel::new(),
            analytics_panel: AnalyticsPanel::new(),
            theme_customizer: ThemeCustomizer::new(),
            model_selection_panel: ModelSelectionPanel::new(),
        }
    }

    /// Create a new panel manager with model manager for enhanced model selection
    pub fn with_model_manager(model_manager: std::sync::Arc<crate::llm::openrouter::models::ModelManager>) -> Self {
        let mut manager = Self::new();
        manager.model_selection_panel = ModelSelectionPanel::with_model_manager(model_manager);
        manager
    }

    /// Set the current model in the model selection panel
    pub fn set_current_model(&mut self, model: String) {
        self.model_selection_panel.set_current_model(model);
    }

    /// Get the current model from the model selection panel
    pub fn get_current_model(&self) -> &str {
        self.model_selection_panel.get_current_model()
    }

    /// Render the model selection panel and return any selected model
    pub fn render_model_selection_panel(&mut self, ctx: &Context, theme: AppTheme) -> Option<String> {
        self.model_selection_panel.render(ctx, theme)
    }

    pub fn show_preview(&mut self, title: &str, content: &str) {
        log::debug!("show_preview called with title: {}", title);
        self.preview_panel.set_content(title, content);
        
        // Automatically open the preview panel if it's not already open
        if !self.preview_panel.visible {
            log::debug!("Opening preview panel");
            self.toggle_panel(ActivePanel::Preview);
        }
    }

    pub fn toggle_panel(&mut self, panel: ActivePanel) {
        match panel {
            ActivePanel::Repository => {
                // Repository panel is handled by the main app
                if matches!(self.active_panel, ActivePanel::Repository) {
                    self.active_panel = ActivePanel::None;
                } else {
                    self.close_all_panels();
                    self.active_panel = ActivePanel::Repository;
                }
            },
            ActivePanel::Preview => {
                if matches!(self.active_panel, ActivePanel::Preview) {
                    self.preview_panel.toggle(); // Close
                    self.active_panel = ActivePanel::None;
                } else {
                    self.close_all_panels();
                    self.preview_panel.toggle(); // Open
                    self.active_panel = ActivePanel::Preview;
                }
            },
            ActivePanel::Settings => {
                // Settings panel is handled by the main app
                if matches!(self.active_panel, ActivePanel::Settings) {
                    self.active_panel = ActivePanel::None;
                } else {
                    self.close_all_panels();
                    self.active_panel = ActivePanel::Settings;
                }
            },
            ActivePanel::Conversation => {
                if matches!(self.active_panel, ActivePanel::Conversation) {
                    self.active_panel = ActivePanel::None;
                } else {
                    self.close_all_panels();
                    self.active_panel = ActivePanel::Conversation;
                }
            },
            ActivePanel::Events => {
                if matches!(self.active_panel, ActivePanel::Events) {
                    self.events_panel.toggle(); // Close
                    self.active_panel = ActivePanel::None;
                } else {
                    self.close_all_panels();
                    self.events_panel.toggle(); // Open
                    self.active_panel = ActivePanel::Events;
                }
            },
            ActivePanel::Analytics => {
                if matches!(self.active_panel, ActivePanel::Analytics) {
                    self.analytics_panel.toggle(); // Close
                    self.active_panel = ActivePanel::None;
                } else {
                    self.close_all_panels();
                    self.analytics_panel.toggle(); // Open
                    self.active_panel = ActivePanel::Analytics;
                }
            },
            ActivePanel::ThemeCustomizer => {
                if matches!(self.active_panel, ActivePanel::ThemeCustomizer) {
                    self.theme_customizer.toggle(); // Close
                    self.active_panel = ActivePanel::None;
                } else {
                    self.close_all_panels();
                    self.theme_customizer.toggle(); // Open
                    self.active_panel = ActivePanel::ThemeCustomizer;
                }
            },
            ActivePanel::ModelSelection => {
                if matches!(self.active_panel, ActivePanel::ModelSelection) {
                    self.model_selection_panel.toggle(); // Close
                    self.active_panel = ActivePanel::None;
                } else {
                    self.close_all_panels();
                    self.model_selection_panel.toggle(); // Open
                    self.active_panel = ActivePanel::ModelSelection;
                }
            },
            ActivePanel::None => {
                self.close_all_panels();
            }
        }
    }

    pub fn close_all_panels(&mut self) {
        match self.active_panel {
            ActivePanel::Preview => {
                if self.preview_panel.visible {
                    self.preview_panel.toggle(); // Close
                }
            },
            ActivePanel::Events => {
                if self.events_panel.visible {
                    self.events_panel.toggle(); // Close
                }
            },
            ActivePanel::Analytics => {
                if self.analytics_panel.visible {
                    self.analytics_panel.toggle(); // Close
                }
            },
            ActivePanel::ThemeCustomizer => {
                if self.theme_customizer.is_open() {
                    self.theme_customizer.toggle(); // Close
                }
            },
            ActivePanel::ModelSelection => {
                if self.model_selection_panel.visible {
                    self.model_selection_panel.toggle(); // Close
                }
            },
            _ => {}
        }
        
        self.active_panel = ActivePanel::None;
    }
}

// Pricing constants (per 1 million tokens)
const GEMINI_1_5_FLASH_INPUT_COST_PER_MILLION_TOKENS: f64 = 0.075;
const GEMINI_1_5_FLASH_OUTPUT_COST_PER_MILLION_TOKENS: f64 = 0.30;

#[derive(Debug, Clone)]
pub struct TokenUsageEntry {
    pub timestamp: std::time::SystemTime,
    pub conversation_id: String,
    pub model_name: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cost: f64,
}

impl TokenUsageEntry {
    pub fn new(
        conversation_id: String,
        model_name: String,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Self {
        let input_cost = (input_tokens as f64 / 1_000_000.0) * GEMINI_1_5_FLASH_INPUT_COST_PER_MILLION_TOKENS;
        let output_cost = (output_tokens as f64 / 1_000_000.0) * GEMINI_1_5_FLASH_OUTPUT_COST_PER_MILLION_TOKENS;
        Self {
            timestamp: std::time::SystemTime::now(),
            conversation_id,
            model_name,
            input_tokens,
            output_tokens,
            cost: input_cost + output_cost,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConversationAnalytics {
    pub id: String,
    pub total_input_tokens: u32,
    pub total_output_tokens: u32,
    pub total_cost: f64,
    pub entry_count: usize,
    pub last_updated: std::time::SystemTime,
}

/// Analytics panel for displaying token usage and cost
pub struct AnalyticsPanel {
    pub visible: bool,
    pub all_usage_entries: Vec<TokenUsageEntry>,
    // For now, we'll manually manage conversation summaries.
    // In the future, this could be derived or stored more robustly.
    pub conversation_summaries: std::collections::HashMap<String, ConversationAnalytics>,
    pub time_filter: TimeFilter,
    pub selected_conversation_id: Option<String>,
    
    // Phase 7 enhancements
    /// Comprehensive analytics report from ConversationAnalyticsManager
    pub analytics_report: Option<AnalyticsReport>,
    
    /// Date range filter for analytics data
    pub date_range_filter: DateRangeFilter,
    
    /// Project filter for analytics data
    pub project_filter: ProjectFilter,
    
    /// Whether to show detailed success metrics
    pub show_success_details: bool,
    
    /// Active tab in the analytics dashboard
    pub active_tab: AnalyticsTab,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TimeFilter {
    LastHour,
    Last24Hours,
    Last7Days,
    AllTime,
}

/// Date range filter for analytics
#[derive(Debug, Clone, PartialEq)]
pub enum DateRangeFilter {
    Last7Days,
    Last30Days,
    Last90Days,
    AllTime,
}

/// Project filter for analytics
#[derive(Debug, Clone, PartialEq)]
pub enum ProjectFilter {
    All,
    Specific(ProjectType),
}

/// Analytics dashboard tabs
#[derive(Debug, Clone, PartialEq)]
pub enum AnalyticsTab {
    Overview,
    Success,
    Efficiency,
    Patterns,
    Projects,
    Trends,
}

/// Actions that can be triggered from the analytics panel
#[derive(Debug, Clone, PartialEq)]
pub enum AnalyticsAction {
    SwitchToSuccessMode,
    RefreshAnalytics,
    ExportReport,
    FilterByProject(ProjectType),
    FilterByDateRange(DateRangeFilter),
}

impl AnalyticsPanel {
    pub fn new() -> Self {
        Self {
            visible: false,
            all_usage_entries: Vec::new(),
            conversation_summaries: std::collections::HashMap::new(),
            time_filter: TimeFilter::AllTime,
            selected_conversation_id: None,
            analytics_report: None,
            date_range_filter: DateRangeFilter::Last30Days,
            project_filter: ProjectFilter::All,
            show_success_details: false,
            active_tab: AnalyticsTab::Overview,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Set the comprehensive analytics report
    pub fn set_analytics_report(&mut self, report: Option<AnalyticsReport>) {
        self.analytics_report = report;
    }

    /// Toggle success details display
    pub fn toggle_success_details(&mut self) {
        self.show_success_details = !self.show_success_details;
    }

    /// Handle success rate click to switch to success mode
    pub fn handle_success_rate_click(&self) -> Option<AnalyticsAction> {
        Some(AnalyticsAction::SwitchToSuccessMode)
    }

    /// Handle refresh request
    pub fn handle_refresh_request(&self) -> Option<AnalyticsAction> {
        Some(AnalyticsAction::RefreshAnalytics)
    }

    /// Handle export request
    pub fn handle_export_request(&self) -> Option<AnalyticsAction> {
        if self.analytics_report.is_some() {
            Some(AnalyticsAction::ExportReport)
        } else {
            None
        }
    }

    /// Reset filters to default values
    pub fn reset_filters(&mut self) {
        self.project_filter = ProjectFilter::All;
        // Keep date_range_filter as is since it has a reasonable default
    }

    pub fn add_usage_entry(&mut self, entry: TokenUsageEntry) {
        self.all_usage_entries.push(entry.clone());
        
        let summary = self.conversation_summaries
            .entry(entry.conversation_id.clone())
            .or_insert_with(|| ConversationAnalytics {
                id: entry.conversation_id.clone(),
                total_input_tokens: 0,
                total_output_tokens: 0,
                total_cost: 0.0,
                entry_count: 0,
                last_updated: entry.timestamp,
            });

        summary.total_input_tokens += entry.input_tokens;
        summary.total_output_tokens += entry.output_tokens;
        summary.total_cost += entry.cost;
        summary.entry_count += 1;
        summary.last_updated = entry.timestamp;

        // Keep all_usage_entries sorted by time for easier filtering, or sort when rendering
        self.all_usage_entries.sort_by_key(|e| e.timestamp);
    }
    
    fn get_filtered_entries(&self) -> Vec<&TokenUsageEntry> {
        let now = std::time::SystemTime::now();
        let filter_duration = match self.time_filter {
            TimeFilter::LastHour => std::time::Duration::from_secs(3600),
            TimeFilter::Last24Hours => std::time::Duration::from_secs(24 * 3600),
            TimeFilter::Last7Days => std::time::Duration::from_secs(7 * 24 * 3600),
            TimeFilter::AllTime => return self.all_usage_entries.iter().collect(),
        };

        self.all_usage_entries.iter()
            .filter(|entry| now.duration_since(entry.timestamp).unwrap_or_default() <= filter_duration)
            .collect()
    }

    pub fn render(&mut self, ctx: &egui::Context, theme: AppTheme) -> Option<AnalyticsAction> {
        if !self.visible {
            return None;
        }

        let mut action = None;

        // Use a panel instead of window
        egui::SidePanel::right("analytics_panel")
            .default_width(900.0)
            .min_width(600.0)
            .resizable(true)
            .frame(egui::Frame::none()
                .fill(theme.panel_background())
                .inner_margin(egui::Margin::same(10)))
            .show(ctx, |ui| {
                // Main scrollable area for the entire panel
                egui::ScrollArea::vertical()
                    .id_source("analytics_main_scroll")
                    .show(ui, |ui| {
                // Header with title and controls
                ui.horizontal(|ui| {
                    ui.heading("📊 Analytics Dashboard");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("❌").on_hover_text("Close analytics panel").clicked() {
                            self.visible = false;
                        }
                        ui.separator();
                        if ui.button("🔄 Refresh").on_hover_text("Refresh analytics data").clicked() {
                            action = self.handle_refresh_request();
                        }
                        if ui.button("📤 Export").on_hover_text("Export analytics report").clicked() {
                            action = self.handle_export_request();
                        }
                    });
                });
                
                ui.add_space(5.0);
                ui.separator();
                ui.add_space(5.0);

                // Filters section
                ui.horizontal(|ui| {
                    ui.label("Filters:");
                    
                    // Date range filter
                    ComboBox::from_label("Date Range")
                        .selected_text(format!("{:?}", self.date_range_filter))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.date_range_filter, DateRangeFilter::Last7Days, "Last 7 Days");
                            ui.selectable_value(&mut self.date_range_filter, DateRangeFilter::Last30Days, "Last 30 Days");
                            ui.selectable_value(&mut self.date_range_filter, DateRangeFilter::Last90Days, "Last 90 Days");
                            ui.selectable_value(&mut self.date_range_filter, DateRangeFilter::AllTime, "All Time");
                        });

                    // Project filter
                    ComboBox::from_label("Project")
                        .selected_text(match &self.project_filter {
                            ProjectFilter::All => "All Projects".to_string(),
                            ProjectFilter::Specific(project_type) => format!("{:?}", project_type),
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.project_filter, ProjectFilter::All, "All Projects");
                            ui.selectable_value(&mut self.project_filter, ProjectFilter::Specific(ProjectType::Rust), "Rust");
                            ui.selectable_value(&mut self.project_filter, ProjectFilter::Specific(ProjectType::Python), "Python");
                            ui.selectable_value(&mut self.project_filter, ProjectFilter::Specific(ProjectType::JavaScript), "JavaScript");
                            ui.selectable_value(&mut self.project_filter, ProjectFilter::Specific(ProjectType::TypeScript), "TypeScript");
                            ui.selectable_value(&mut self.project_filter, ProjectFilter::Specific(ProjectType::Go), "Go");
                            ui.selectable_value(&mut self.project_filter, ProjectFilter::Specific(ProjectType::Ruby), "Ruby");
                            ui.selectable_value(&mut self.project_filter, ProjectFilter::Specific(ProjectType::Markdown), "Markdown");
                            ui.selectable_value(&mut self.project_filter, ProjectFilter::Specific(ProjectType::Yaml), "YAML");
                            ui.selectable_value(&mut self.project_filter, ProjectFilter::Specific(ProjectType::Html), "HTML");
                        });

                    if ui.button("Reset Filters").clicked() {
                        self.reset_filters();
                    }
                });
                ui.separator();

                // Tab navigation
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.active_tab, AnalyticsTab::Overview, "📊 Overview");
                    ui.selectable_value(&mut self.active_tab, AnalyticsTab::Success, "🎯 Success");
                    ui.selectable_value(&mut self.active_tab, AnalyticsTab::Efficiency, "⚡ Efficiency");
                    ui.selectable_value(&mut self.active_tab, AnalyticsTab::Patterns, "🔍 Patterns");
                    ui.selectable_value(&mut self.active_tab, AnalyticsTab::Projects, "📁 Projects");
                    ui.selectable_value(&mut self.active_tab, AnalyticsTab::Trends, "📈 Trends");
                });
                ui.separator();

                // Content area with scrolling
                ScrollArea::vertical().show(ui, |ui| {
                    if let Some(report) = self.analytics_report.clone() {
                        match self.active_tab {
                            AnalyticsTab::Overview => {
                                if let Some(overview_action) = self.render_overview_tab(ui, &report, theme) {
                                    action = Some(overview_action);
                                }
                            },
                            AnalyticsTab::Success => {
                                if let Some(success_action) = self.render_success_tab(ui, &report, theme) {
                                    action = Some(success_action);
                                }
                            },
                            AnalyticsTab::Efficiency => {
                                self.render_efficiency_tab(ui, &report, theme);
                            },
                            AnalyticsTab::Patterns => {
                                self.render_patterns_tab(ui, &report, theme);
                            },
                            AnalyticsTab::Projects => {
                                self.render_projects_tab(ui, &report, theme);
                            },
                            AnalyticsTab::Trends => {
                                self.render_trends_tab(ui, &report, theme);
                            },
                        }
                    } else {
                        ui.vertical_centered(|ui| {
                            ui.add_space(50.0);
                            ui.label("📊 No analytics data available");
                            ui.label("Click 'Refresh' to generate analytics report");
                            ui.add_space(20.0);
                            if ui.button("🔄 Generate Analytics").clicked() {
                                action = self.handle_refresh_request();
                            }
                        });
                    }
                });

                // Legacy token usage section (collapsible)
                ui.separator();
                ui.collapsing("💰 Token Usage & Cost (Legacy)", |ui| {
                    self.render_legacy_token_usage(ui, theme);
                });
                    }); // End of ScrollArea
            }); // End of SidePanel

        action
    }

    /// Render the legacy token usage section
    fn render_legacy_token_usage(&mut self, ui: &mut egui::Ui, theme: AppTheme) {
        // Add some dummy data for now if empty
        if self.all_usage_entries.is_empty() && ui.button("Add Dummy Data").clicked() {
            self.add_usage_entry(TokenUsageEntry::new("conv_1".to_string(), "gemini-1.5-flash".to_string(), 1500, 3000));
            let mut entry2 = TokenUsageEntry::new("conv_1".to_string(), "gemini-1.5-pro".to_string(), 200, 500);
            entry2.timestamp = std::time::SystemTime::now() - std::time::Duration::from_secs(3700); // > 1 hour ago
            self.add_usage_entry(entry2);
            self.add_usage_entry(TokenUsageEntry::new("conv_2".to_string(), "gemini-1.5-flash".to_string(), 800, 1200));
        }
        
        ui.horizontal(|ui| {
            ui.label("Filter by time:");
            if ui.selectable_value(&mut self.time_filter, TimeFilter::LastHour, "Last Hour").clicked() {};
            if ui.selectable_value(&mut self.time_filter, TimeFilter::Last24Hours, "Last 24h").clicked() {};
            if ui.selectable_value(&mut self.time_filter, TimeFilter::Last7Days, "Last 7d").clicked() {};
            if ui.selectable_value(&mut self.time_filter, TimeFilter::AllTime, "All Time").clicked() {};
        });
        ui.separator();

        let filtered_entries = self.get_filtered_entries();
        let total_input_tokens: u32 = filtered_entries.iter().map(|e| e.input_tokens).sum();
        let total_output_tokens: u32 = filtered_entries.iter().map(|e| e.output_tokens).sum();
        let total_cost: f64 = filtered_entries.iter().map(|e| e.cost).sum();
        
        ui.heading("Overall Summary (Filtered)");
        ui.label(format!("Total API Calls: {}", filtered_entries.len()));
        ui.label(format!("Total Input Tokens: {}", total_input_tokens));
        ui.label(format!("Total Output Tokens: {}", total_output_tokens));
        ui.label(format!("Total Combined Tokens: {}", total_input_tokens + total_output_tokens));
        ui.label(format!("Estimated Total Cost: ${:.6}", total_cost));
        ui.label(RichText::new(format!("(Prices based on Gemini 1.5 Flash: Input ${}/1M, Output ${}/1M tokens)", GEMINI_1_5_FLASH_INPUT_COST_PER_MILLION_TOKENS, GEMINI_1_5_FLASH_OUTPUT_COST_PER_MILLION_TOKENS)).small().weak());
        
        ui.separator();

        let mut new_selected_id: Option<String> = None;
        let mut clear_selection = false;

        if let Some(selected_id_val) = &self.selected_conversation_id {
            let current_selected_id = selected_id_val.clone();

            // --- Detailed Conversation View ---
            ui.heading(format!("Details for Conversation: {}", current_selected_id));
            if ui.button("⬅ Back to Summary").clicked() {
                clear_selection = true;
            }
            ui.separator();

            let entries_for_selected_convo: Vec<&TokenUsageEntry> = self.all_usage_entries
                .iter()
                .filter(|e| e.conversation_id == current_selected_id)
                .collect();

            if entries_for_selected_convo.is_empty() {
                ui.label("No token usage entries found for this conversation.");
            } else {
                ScrollArea::vertical().show(ui, |ui| {
                    egui::Grid::new(format!("details_{}", current_selected_id)).striped(true).show(ui, |ui| {
                        // Header
                        ui.label(RichText::new("Time").strong());
                        ui.label(RichText::new("Model").strong());
                        ui.label(RichText::new("Input Tokens").strong());
                        ui.label(RichText::new("Output Tokens").strong());
                        ui.label(RichText::new("Cost").strong());
                        ui.end_row();

                        for entry in entries_for_selected_convo {
                            let timestamp_str = chrono::DateTime::<chrono::Utc>::from(entry.timestamp)
                                .format("%H:%M:%S").to_string();
                            ui.label(timestamp_str);
                            ui.label(&entry.model_name);
                            ui.label(entry.input_tokens.to_string());
                            ui.label(entry.output_tokens.to_string());
                            ui.label(format!("${:.6}", entry.cost));
                            ui.end_row();
                        }
                    });
                });
            }
        } else {
            // --- Conversation Summary View ---
            ui.heading("Conversations Summary (All Time)");
            
            ScrollArea::vertical().show(ui, |ui| {
                if self.conversation_summaries.is_empty() {
                    ui.label("No conversation data yet.");
                } else {
                    let mut conv_summaries_vec: Vec<_> = self.conversation_summaries.values().cloned().collect();
                    conv_summaries_vec.sort_by(|a,b| b.last_updated.cmp(&a.last_updated));

                    for summary in conv_summaries_vec {
                        ui.group(|ui| {
                            ui.label(RichText::new(format!("Conversation ID: {}", summary.id)).strong());
                            ui.label(format!("  Entries: {}", summary.entry_count));
                            ui.label(format!("  Total Input Tokens: {}", summary.total_input_tokens));
                            ui.label(format!("  Total Output Tokens: {}", summary.total_output_tokens));
                            ui.label(format!("  Total Cost: ${:.6}", summary.total_cost));
                            if ui.button("View Details").clicked() {
                                new_selected_id = Some(summary.id.clone());
                            }
                        });
                    }
                }
            });
        }

        if clear_selection {
            self.selected_conversation_id = None;
        } else if let Some(id_to_select) = new_selected_id {
            self.selected_conversation_id = Some(id_to_select);
        }

         ui.separator();
         // Placeholder for future: Most Used Models, Charts, etc.

        ui.add_space(10.0);
        ui.separator();
        ui.heading("Visualizations");
        ui.add_space(5.0);

        // --- Tokens Over Time Line Chart ---
        ui.collapsing("Tokens Over Time", |ui| {
            let filtered_entries = self.get_filtered_entries();
            let line_points: PlotPoints = filtered_entries.iter()
                .map(|entry| {
                    let timestamp_secs = entry.timestamp
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs_f64();
                    [timestamp_secs, (entry.input_tokens + entry.output_tokens) as f64]
                })
                .collect();
            
            let line = Line::new(line_points).name("Total Tokens");
            Plot::new("tokens_over_time_plot")
                .legend(Legend::default())
                .height(200.0)
                .x_axis_formatter(|mark, _bounds_or_ctx| {
                    // Format timestamp (seconds since epoch) to a readable time string
                    let timestamp_secs = mark.value;
                    let datetime = chrono::DateTime::<chrono::Utc>::from_timestamp(timestamp_secs as i64, 0)
                        .unwrap_or_default(); 
                    datetime.format("%H:%M").to_string()
                })
                .show(ui, |plot_ui| {
                    plot_ui.line(line);
                });
        });

        // --- Cost Per Conversation Bar Chart ---
        ui.collapsing("Cost Per Conversation", |ui| {
            let mut bars: Vec<Bar> = Vec::new();
            let mut conv_ids: Vec<String> = self.conversation_summaries.keys().cloned().collect();
            conv_ids.sort(); // Sort for consistent bar order

            for (i, conv_id) in conv_ids.iter().enumerate() {
                if let Some(summary) = self.conversation_summaries.get(conv_id) {
                    bars.push(Bar::new(i as f64 + 0.5, summary.total_cost).width(0.95).name(format!("{}", &conv_id[..conv_id.len().min(10)]))); // Truncate name for legend
                }
            }

            let chart = BarChart::new(bars)
                .color(theme.accent_color())
                .name("Conversation Cost");

            Plot::new("cost_per_conversation_plot")
                .legend(Legend::default())
                .height(200.0)
                .show(ui, |plot_ui| {
                    plot_ui.bar_chart(chart);
                });
        });
    }

    /// Render the overview tab
    fn render_overview_tab(&mut self, ui: &mut egui::Ui, report: &AnalyticsReport, theme: AppTheme) -> Option<AnalyticsAction> {
        let mut action = None;
        
        ui.heading("📊 Overview");
        ui.add_space(10.0);
        
        // Key metrics cards - use a grid layout for better responsiveness
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
            
            // Total conversations card
            ui.group(|ui| {
                ui.set_min_width(150.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("Total Conversations").strong());
                    ui.label(RichText::new(format!("{}", report.overall_metrics.total_conversations))
                        .size(24.0).color(theme.accent_color()));
                });
            });
            
            // Success rate card
            ui.group(|ui| {
                ui.set_min_width(150.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("Success Rate").strong());
                    let success_rate = report.success_metrics.overall_success_rate * 100.0;
                    let success_color = if success_rate >= 80.0 {
                        Color32::from_rgb(0, 200, 0)
                    } else if success_rate >= 60.0 {
                        Color32::from_rgb(255, 165, 0)
                    } else {
                        Color32::from_rgb(255, 100, 100)
                    };
                    
                    if ui.add(Button::new(RichText::new(format!("{:.1}%", success_rate))
                        .size(24.0).color(success_color))).clicked() {
                        action = self.handle_success_rate_click();
                    }
                });
            });
            
            // Completion rate card
            ui.group(|ui| {
                ui.set_min_width(150.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("Completion Rate").strong());
                    ui.label(RichText::new(format!("{:.1}%", report.overall_metrics.completion_rate * 100.0))
                        .size(24.0).color(theme.accent_color()));
                });
            });
            
            // Average duration card
            ui.group(|ui| {
                ui.set_min_width(150.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("Avg Duration").strong());
                    ui.label(RichText::new(format!("{:.1} min", report.overall_metrics.avg_duration_minutes))
                        .size(24.0).color(theme.accent_color()));
                });
            });
        });
        
        ui.add_space(20.0);
        
        // Activity overview
        ui.heading("Activity Overview");
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(format!("📝 Total Messages: {}", report.overall_metrics.total_messages));
                ui.label(format!("🌳 Total Branches: {}", report.overall_metrics.total_branches));
                ui.label(format!("📍 Total Checkpoints: {}", report.overall_metrics.total_checkpoints));
                ui.label(format!("💬 Avg Messages/Conv: {:.1}", report.overall_metrics.avg_messages_per_conversation));
            });
            
            ui.separator();
            
            ui.vertical(|ui| {
                ui.label("🕐 Peak Activity Hours:");
                let peak_hours: Vec<String> = report.overall_metrics.peak_activity_hours
                    .iter()
                    .map(|h| format!("{}:00", h))
                    .collect();
                ui.label(peak_hours.join(", "));
            });
        });
        
        ui.add_space(20.0);
        
        // Token usage overview
        ui.heading("🪙 Token Usage");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
            
            // Total tokens card
            ui.group(|ui| {
                ui.set_min_width(140.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("Total Tokens").strong());
                    ui.label(RichText::new(format!("{}", report.overall_metrics.total_tokens))
                        .size(20.0).color(theme.accent_color()));
                });
            });
            
            // Peak usage card
            ui.group(|ui| {
                ui.set_min_width(140.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("Peak Usage").strong());
                    ui.label(RichText::new(format!("{}", report.token_usage_metrics.peak_usage))
                        .size(20.0).color(theme.accent_color()));
                });
            });
            
            // Conversations hitting limit
            ui.group(|ui| {
                ui.set_min_width(140.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("Hit Token Limit").strong());
                    ui.label(RichText::new(format!("{}", report.token_usage_metrics.limit_reached_count))
                        .size(20.0).color(if report.token_usage_metrics.limit_reached_count > 0 {
                            Color32::from_rgb(255, 165, 0)
                        } else {
                            theme.accent_color()
                        }));
                });
            });
            
            // Estimated cost
            if let Some(cost) = report.token_usage_metrics.estimated_cost {
                ui.group(|ui| {
                    ui.set_min_width(140.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Est. Cost").strong());
                        ui.label(RichText::new(format!("${:.2}", cost))
                            .size(20.0).color(theme.accent_color()));
                    });
                });
            }
        });
        
        ui.horizontal(|ui| {
            ui.label(format!("📊 Avg Tokens/Conv: {}", report.overall_metrics.avg_tokens_per_conversation));
            ui.separator();
            ui.label(format!("💬 Avg Tokens/Msg: {}", report.overall_metrics.avg_tokens_per_message));
        });
        
        // Token usage by role
        if !report.token_usage_metrics.tokens_by_role.is_empty() {
            ui.add_space(10.0);
            ui.label("Token Distribution by Role:");
            for (role, tokens) in &report.token_usage_metrics.tokens_by_role {
                ui.horizontal(|ui| {
                    ui.label(format!("{}: ", role));
                    ui.label(RichText::new(format!("{} tokens", tokens)).color(theme.accent_color()));
                });
            }
        }
        
        ui.add_space(20.0);
        
        // Project distribution chart
        if !report.overall_metrics.project_type_distribution.is_empty() {
            ui.heading("Project Distribution");
            
            let mut bars: Vec<Bar> = Vec::new();
            let mut project_types: Vec<_> = report.overall_metrics.project_type_distribution.iter().collect();
            project_types.sort_by(|a, b| b.1.cmp(a.1)); // Sort by count descending
            
            for (i, (project_type, count)) in project_types.iter().enumerate() {
                bars.push(Bar::new(i as f64 + 0.5, **count as f64)
                    .width(0.8)
                    .name(format!("{:?}", project_type)));
            }
            
            let chart = BarChart::new(bars)
                .color(theme.accent_color())
                .name("Conversations by Project Type");
            
            Plot::new("project_distribution_plot")
                .legend(Legend::default())
                .height(200.0)
                .show(ui, |plot_ui| {
                    plot_ui.bar_chart(chart);
                });
        }
        
        action
    }

    /// Render the success tab
    fn render_success_tab(&mut self, ui: &mut egui::Ui, report: &AnalyticsReport, theme: AppTheme) -> Option<AnalyticsAction> {
        let mut action = None;
        
        ui.horizontal(|ui| {
            ui.heading("🎯 Success Metrics");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(if self.show_success_details { "Hide Details" } else { "Show Details" }).clicked() {
                    self.toggle_success_details();
                }
                if ui.button("📊 Switch to Success Mode").clicked() {
                    action = self.handle_success_rate_click();
                }
            });
        });
        ui.add_space(10.0);
        
        // Overall success metrics
        ui.group(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new("Overall Success Rate").strong());
                let success_rate = report.success_metrics.overall_success_rate * 100.0;
                ui.label(RichText::new(format!("{:.1}%", success_rate)).size(20.0));
                
                if self.show_success_details {
                    ui.separator();
                    ui.label("Success by conversation length:");
                    for (length, rate) in &report.success_metrics.success_by_length {
                        ui.label(format!("  {} messages: {:.1}%", length, rate * 100.0));
                    }
                }
            });
        });
        
        ui.add_space(15.0);
        
        // Success by project type
        if !report.success_metrics.success_by_project_type.is_empty() {
            ui.heading("Success by Project Type");
            
            let mut project_success: Vec<_> = report.success_metrics.success_by_project_type.iter().collect();
            project_success.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
            
            for (project_type, success_rate) in project_success {
                ui.horizontal(|ui| {
                    ui.label(format!("{:?}:", project_type));
                    let rate_percent = success_rate * 100.0;
                    let color = if rate_percent >= 80.0 {
                        Color32::from_rgb(0, 200, 0)
                    } else if rate_percent >= 60.0 {
                        Color32::from_rgb(255, 165, 0)
                    } else {
                        Color32::from_rgb(255, 100, 100)
                    };
                    ui.colored_label(color, format!("{:.1}%", rate_percent));
                });
            }
        }
        
        ui.add_space(15.0);
        
        // Success patterns
        if !report.success_metrics.successful_patterns.is_empty() {
            ui.heading("Successful Patterns");
            for pattern in &report.success_metrics.successful_patterns {
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&pattern.name).strong());
                        ui.label(&pattern.description);
                        ui.label(format!("Success Rate: {:.1}%", pattern.success_rate * 100.0));
                        ui.label(format!("Frequency: {}", pattern.frequency));
                    });
                });
            }
        }
        
        // Failure points
        if !report.success_metrics.failure_points.is_empty() {
            ui.add_space(15.0);
            ui.heading("Common Failure Points");
            for failure_point in &report.success_metrics.failure_points {
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.colored_label(Color32::from_rgb(255, 100, 100), &failure_point.description);
                        ui.label(format!("Frequency: {}", failure_point.frequency));
                        ui.label(format!("Context: {}", failure_point.context));
                        
                        if !failure_point.mitigations.is_empty() {
                            ui.label("Suggested mitigations:");
                            for mitigation in &failure_point.mitigations {
                                ui.label(format!("  • {}", mitigation));
                            }
                        }
                    });
                });
            }
        }
        
        action
    }

    /// Render the efficiency tab
    fn render_efficiency_tab(&mut self, ui: &mut egui::Ui, report: &AnalyticsReport, theme: AppTheme) {
        ui.heading("⚡ Efficiency Metrics");
        ui.add_space(10.0);
        
        // Key efficiency metrics
        ui.horizontal(|ui| {
            ui.group(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Avg Resolution Time").strong());
                    ui.label(RichText::new(format!("{:.1} min", report.efficiency_metrics.avg_resolution_time))
                        .size(18.0).color(theme.accent_color()));
                });
            });
            
            ui.group(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Branching Efficiency").strong());
                    ui.label(RichText::new(format!("{:.1}%", report.efficiency_metrics.branching_efficiency * 100.0))
                        .size(18.0).color(theme.accent_color()));
                });
            });
            
            ui.group(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Checkpoint Utilization").strong());
                    ui.label(RichText::new(format!("{:.1}%", report.efficiency_metrics.checkpoint_utilization * 100.0))
                        .size(18.0).color(theme.accent_color()));
                });
            });
        });
        
        ui.add_space(20.0);
        
        // Context switching metrics
        ui.heading("Context Management");
        ui.label(format!("Average context switches per conversation: {:.1}", 
            report.efficiency_metrics.context_switches_per_conversation));
        
        ui.add_space(15.0);
        
        // Resource utilization
        ui.heading("Resource Utilization");
        let resource_util = &report.efficiency_metrics.resource_utilization;
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(format!("Memory Usage: {:.1} MB", resource_util.avg_memory_usage));
                ui.label(format!("Storage Efficiency: {:.1}%", resource_util.storage_efficiency * 100.0));
                ui.label(format!("Clustering Efficiency: {:.1}%", resource_util.clustering_efficiency * 100.0));
            });
            
            ui.separator();
            
            ui.vertical(|ui| {
                ui.label("Search Performance:");
                ui.label(format!("  Avg Time: {:.1}ms", resource_util.search_performance.avg_search_time_ms));
                ui.label(format!("  Accuracy: {:.1}%", resource_util.search_performance.accuracy_rate * 100.0));
            });
        });
        
        // Efficient patterns
        if !report.efficiency_metrics.efficient_patterns.is_empty() {
            ui.add_space(15.0);
            ui.heading("Efficient Patterns");
            for pattern in &report.efficiency_metrics.efficient_patterns {
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&pattern.name).strong());
                        ui.label(format!("Efficiency Score: {:.1}%", pattern.efficiency_score * 100.0));
                        
                        if !pattern.characteristics.is_empty() {
                            ui.label("Characteristics:");
                            for characteristic in &pattern.characteristics {
                                ui.label(format!("  • {}", characteristic));
                            }
                        }
                    });
                });
            }
        }
    }

    /// Render the patterns tab
    fn render_patterns_tab(&mut self, ui: &mut egui::Ui, report: &AnalyticsReport, _theme: AppTheme) {
        ui.heading("🔍 Pattern Analysis");
        ui.add_space(10.0);
        
        // Common flows
        if !report.patterns.common_flows.is_empty() {
            ui.heading("Common Conversation Flows");
            for flow in &report.patterns.common_flows {
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&flow.name).strong());
                        ui.label(format!("Average Duration: {:.1} min", flow.avg_duration));
                        
                        if !flow.states.is_empty() {
                            ui.label("Flow States:");
                            for state in &flow.states {
                                ui.label(format!("  → {}", state));
                            }
                        }
                    });
                });
            }
        }
        
        // Recurring themes
        if !report.patterns.recurring_themes.is_empty() {
            ui.add_space(15.0);
            ui.heading("Recurring Themes");
            for theme in &report.patterns.recurring_themes {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&theme.name).strong());
                        ui.label(format!("({})", theme.frequency));
                    });
                    
                    if !theme.keywords.is_empty() {
                        ui.label(format!("Keywords: {}", theme.keywords.join(", ")));
                    }
                });
            }
        }
        
        // Temporal patterns
        if !report.patterns.temporal_patterns.is_empty() {
            ui.add_space(15.0);
            ui.heading("Temporal Patterns");
            for pattern in &report.patterns.temporal_patterns {
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&pattern.name).strong());
                        ui.label(&pattern.time_characteristics);
                        
                        if !pattern.peak_periods.is_empty() {
                            ui.label(format!("Peak Periods: {}", pattern.peak_periods.join(", ")));
                        }
                    });
                });
            }
        }
        
        // Behavior patterns
        if !report.patterns.behavior_patterns.is_empty() {
            ui.add_space(15.0);
            ui.heading("User Behavior Patterns");
            for pattern in &report.patterns.behavior_patterns {
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&pattern.name).strong());
                        ui.label(&pattern.description);
                        ui.label(format!("Frequency: {}", pattern.frequency));
                        ui.label(format!("Success Impact: {:.1}%", pattern.success_impact * 100.0));
                    });
                });
            }
        }
        
        // Anomalies
        if !report.patterns.anomalies.is_empty() {
            ui.add_space(15.0);
            ui.heading("Detected Anomalies");
            for anomaly in &report.patterns.anomalies {
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.colored_label(Color32::from_rgb(255, 165, 0), 
                            RichText::new(&anomaly.anomaly_type).strong());
                        ui.label(&anomaly.description);
                        ui.label(format!("Severity: {:?}", anomaly.severity));
                        
                        if !anomaly.investigation_steps.is_empty() {
                            ui.label("Investigation Steps:");
                            for step in &anomaly.investigation_steps {
                                ui.label(format!("  • {}", step));
                            }
                        }
                    });
                });
            }
        }
    }

    /// Render the projects tab
    fn render_projects_tab(&mut self, ui: &mut egui::Ui, report: &AnalyticsReport, theme: AppTheme) {
        ui.heading("📁 Project Insights");
        ui.add_space(10.0);
        
        if report.project_insights.is_empty() {
            ui.label("No project-specific insights available.");
            return;
        }
        
        for insight in &report.project_insights {
            ui.group(|ui| {
                ui.vertical(|ui| {
                    // Project header
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{:?}", insight.project_type)).strong().size(16.0));
                        ui.label(format!("({} conversations)", insight.conversation_count));
                        
                        let success_rate = insight.success_rate * 100.0;
                        let color = if success_rate >= 80.0 {
                            Color32::from_rgb(0, 200, 0)
                        } else if success_rate >= 60.0 {
                            Color32::from_rgb(255, 165, 0)
                        } else {
                            Color32::from_rgb(255, 100, 100)
                        };
                        ui.colored_label(color, format!("Success: {:.1}%", success_rate));
                    });
                    
                    ui.separator();
                    
                    // Common topics
                    if !insight.common_topics.is_empty() {
                        ui.label("Common Topics:");
                        ui.label(format!("  {}", insight.common_topics.join(", ")));
                    }
                    
                    // Typical patterns
                    if !insight.typical_patterns.is_empty() {
                        ui.label("Typical Patterns:");
                        for pattern in &insight.typical_patterns {
                            ui.label(format!("  • {}", pattern));
                        }
                    }
                    
                    // Recommendations
                    if !insight.recommendations.is_empty() {
                        ui.label("Recommendations:");
                        for recommendation in &insight.recommendations {
                            ui.colored_label(theme.accent_color(), format!("  → {}", recommendation));
                        }
                    }
                });
            });
            
            ui.add_space(10.0);
        }
    }

    /// Render the trends tab
    fn render_trends_tab(&mut self, ui: &mut egui::Ui, report: &AnalyticsReport, theme: AppTheme) {
        ui.heading("📈 Trending Topics");
        ui.add_space(10.0);
        
        if report.trending_topics.is_empty() {
            ui.label("No trending topics available.");
            return;
        }
        
        for topic in &report.trending_topics {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    // Topic name and trend indicator
                    ui.label(RichText::new(&topic.topic).strong().size(14.0));
                    
                    let (trend_icon, trend_color) = match topic.trend {
                        crate::agent::conversation::analytics::TrendDirection::Growing => ("📈", Color32::from_rgb(0, 200, 0)),
                        crate::agent::conversation::analytics::TrendDirection::Stable => ("➡️", Color32::from_rgb(100, 100, 100)),
                        crate::agent::conversation::analytics::TrendDirection::Declining => ("📉", Color32::from_rgb(255, 100, 100)),
                    };
                    ui.colored_label(trend_color, trend_icon);
                    
                    ui.label(format!("({} occurrences)", topic.frequency));
                    
                    // Success rate
                    let success_rate = topic.success_rate * 100.0;
                    let success_color = if success_rate >= 80.0 {
                        Color32::from_rgb(0, 200, 0)
                    } else if success_rate >= 60.0 {
                        Color32::from_rgb(255, 165, 0)
                    } else {
                        Color32::from_rgb(255, 100, 100)
                    };
                    ui.colored_label(success_color, format!("Success: {:.1}%", success_rate));
                });
                
                // Associated project types
                if !topic.project_types.is_empty() {
                    ui.label(format!("Projects: {}", 
                        topic.project_types.iter()
                            .map(|pt| format!("{:?}", pt))
                            .collect::<Vec<_>>()
                            .join(", ")));
                }
            });
            
            ui.add_space(8.0);
        }
        
        ui.add_space(15.0);
        
        // Recommendations
        if !report.recommendations.is_empty() {
            ui.heading("💡 Recommendations");
            
            for recommendation in &report.recommendations {
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        // Priority and category
                        ui.horizontal(|ui| {
                            let priority_color = match recommendation.priority {
                                crate::agent::conversation::analytics::Priority::High => Color32::from_rgb(255, 100, 100),
                                crate::agent::conversation::analytics::Priority::Medium => Color32::from_rgb(255, 165, 0),
                                crate::agent::conversation::analytics::Priority::Low => Color32::from_rgb(100, 100, 100),
                                crate::agent::conversation::analytics::Priority::Critical => Color32::from_rgb(255, 0, 0),
                            };
                            ui.colored_label(priority_color, format!("{:?} Priority", recommendation.priority));
                            ui.label(format!("({:?})", recommendation.category));
                        });
                        
                        // Description and impact
                        ui.label(RichText::new(&recommendation.description).strong());
                        ui.label(format!("Expected Impact: {}", recommendation.expected_impact));
                        ui.label(format!("Difficulty: {:?}", recommendation.difficulty));
                        
                        // Evidence
                        if !recommendation.evidence.is_empty() {
                            ui.label("Evidence:");
                            for evidence in &recommendation.evidence {
                                ui.label(format!("  • {}", evidence));
                            }
                        }
                    });
                });
                
                ui.add_space(8.0);
            }
        }
    }
}

/// Model selection panel for choosing OpenRouter models
pub struct ModelSelectionPanel {
    pub visible: bool,
    pub current_model: String,
    pub available_models: Vec<crate::llm::openrouter::api::ModelInfo>,
    pub filtered_models: Vec<crate::llm::openrouter::api::ModelInfo>,
    pub favorites: Vec<String>,
    pub search_query: String,
    pub selected_provider: Option<String>,
    pub selected_category: Option<crate::llm::openrouter::models::ModelCategory>,
    pub price_range: (f64, f64), // min, max price per million tokens
    pub min_context_length: u64,
    pub show_only_tool_capable: bool,
    pub show_only_reasoning: bool,
    pub show_only_vision: bool,
    pub sort_by: ModelSortBy,
    pub view_mode: ModelViewMode,
    pub loading: bool,
    pub error_message: Option<String>,
    pub model_manager: Option<std::sync::Arc<crate::llm::openrouter::models::ModelManager>>,
    pub last_refresh: std::time::Instant,
    pub selected_for_comparison: Vec<String>, // Model IDs for comparison
    pub show_comparison: bool,
    // Channel for receiving async model loading results
    pub pending_refresh_receiver: Option<std::sync::mpsc::Receiver<Result<Vec<crate::llm::openrouter::api::ModelInfo>, crate::llm::openrouter::error::OpenRouterError>>>,
    // Manual model entry
    pub manual_model_entry: String,
    pub show_manual_entry: bool,
    // Render loop optimization
    pub last_pending_check: std::time::Instant,
}

/// How to sort the model list
#[derive(Debug, Clone, PartialEq)]
pub enum ModelSortBy {
    Name,
    Provider,
    Price,
    ContextLength,
    Created,
    Popularity,
}

/// How to display the models
#[derive(Debug, Clone, PartialEq)]
pub enum ModelViewMode {
    List,
    Cards,
    Table,
}

impl ModelSelectionPanel {
    pub fn new() -> Self {
        let mut panel = Self {
            visible: false,
            current_model: String::new(),
            available_models: Vec::new(),
            filtered_models: Vec::new(),
            favorites: Vec::new(),
            search_query: String::new(),
            selected_provider: None,
            selected_category: None,
            price_range: (0.0, 1.0), // $0 to $1 per million tokens
            min_context_length: 0,
            show_only_tool_capable: false,
            show_only_reasoning: false,
            show_only_vision: false,
            sort_by: ModelSortBy::Created, // Changed default to show newest first
            view_mode: ModelViewMode::Table, // Changed default to Table
            loading: false,
            error_message: None,
            model_manager: None,
            last_refresh: std::time::Instant::now() - std::time::Duration::from_secs(3600),
            selected_for_comparison: Vec::new(),
            show_comparison: false,
            pending_refresh_receiver: None,
            manual_model_entry: String::new(),
            show_manual_entry: false,
            last_pending_check: std::time::Instant::now(),
        };
        
        // Add some default popular models to avoid empty panel
        panel.add_default_models();
        panel
    }

    pub fn with_model_manager(model_manager: std::sync::Arc<crate::llm::openrouter::models::ModelManager>) -> Self {
        let mut panel = Self::new();
        panel.model_manager = Some(model_manager);
        panel
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        
        // Auto-load models when panel is first opened
        if self.visible && self.should_refresh() {
            self.refresh_models();
        }
    }

    pub fn set_current_model(&mut self, model: String) {
        self.current_model = model;
    }

    pub fn get_current_model(&self) -> &str {
        &self.current_model
    }

    fn add_default_models(&mut self) {
        // Add only recent code-capable models with tool support (last 4 months)
        // These are high-quality models suitable for coding tasks
        use crate::llm::openrouter::api::{ModelInfo, Pricing, Architecture, TopProvider};
        
        let four_months_ago = 1725148800; // September 1, 2024 (approximate)
        
        let default_models = vec![
            // Claude 3.5 Sonnet - Excellent for coding with tool support
            ModelInfo {
                id: "anthropic/claude-3.5-sonnet".to_string(),
                name: "Claude 3.5 Sonnet".to_string(),
                description: "Anthropic's most intelligent model with excellent coding capabilities and tool support".to_string(),
                pricing: Pricing {
                    prompt: "0.000003".to_string(),
                    completion: "0.000015".to_string(),
                    request: Some("0.0".to_string()),
                    image: Some("0.0".to_string()),
                    input_cache_read: None,
                    input_cache_write: None,
                    web_search: None,
                    internal_reasoning: None,
                },
                context_length: 200000,
                architecture: Architecture {
                    input_modalities: vec!["text".to_string(), "image".to_string()],
                    output_modalities: vec!["text".to_string()],
                    tokenizer: "claude".to_string(),
                },
                top_provider: TopProvider {
                    is_moderated: false,
                },
                created: 1719792000, // June 2024
                hugging_face_id: None,
                per_request_limits: None,
                supported_parameters: None,
            },
            // GPT-4o - Strong coding model with tool support
            ModelInfo {
                id: "openai/gpt-4o".to_string(),
                name: "GPT-4o".to_string(),
                description: "OpenAI's flagship model with excellent coding capabilities and comprehensive tool support".to_string(),
                pricing: Pricing {
                    prompt: "0.000005".to_string(),
                    completion: "0.000015".to_string(),
                    request: Some("0.0".to_string()),
                    image: Some("0.0".to_string()),
                    input_cache_read: None,
                    input_cache_write: None,
                    web_search: None,
                    internal_reasoning: None,
                },
                context_length: 128000,
                architecture: Architecture {
                    input_modalities: vec!["text".to_string(), "image".to_string()],
                    output_modalities: vec!["text".to_string()],
                    tokenizer: "gpt".to_string(),
                },
                top_provider: TopProvider {
                    is_moderated: false,
                },
                created: 1715040000, // May 2024
                hugging_face_id: None,
                per_request_limits: None,
                supported_parameters: None,
            },
            // GPT-4o Mini - Fast and efficient with tool support
            ModelInfo {
                id: "openai/gpt-4o-mini".to_string(),
                name: "GPT-4o Mini".to_string(),
                description: "Fast, efficient coding model with tool support and excellent value".to_string(),
                pricing: Pricing {
                    prompt: "0.00000015".to_string(),
                    completion: "0.0000006".to_string(),
                    request: Some("0.0".to_string()),
                    image: Some("0.0".to_string()),
                    input_cache_read: None,
                    input_cache_write: None,
                    web_search: None,
                    internal_reasoning: None,
                },
                context_length: 128000,
                architecture: Architecture {
                    input_modalities: vec!["text".to_string(), "image".to_string()],
                    output_modalities: vec!["text".to_string()],
                    tokenizer: "gpt".to_string(),
                },
                top_provider: TopProvider {
                    is_moderated: false,
                },
                created: 1721260800, // July 2024
                hugging_face_id: None,
                per_request_limits: None,
                supported_parameters: None,
            },
            // DeepSeek Coder V2 - Specialized coding model
            ModelInfo {
                id: "deepseek/deepseek-coder".to_string(),
                name: "DeepSeek Coder V2".to_string(),
                description: "Specialized coding model with excellent programming capabilities and tool support".to_string(),
                pricing: Pricing {
                    prompt: "0.00000014".to_string(),
                    completion: "0.00000028".to_string(),
                    request: Some("0.0".to_string()),
                    image: Some("0.0".to_string()),
                    input_cache_read: None,
                    input_cache_write: None,
                    web_search: None,
                    internal_reasoning: None,
                },
                context_length: 64000,
                architecture: Architecture {
                    input_modalities: vec!["text".to_string()],
                    output_modalities: vec!["text".to_string()],
                    tokenizer: "deepseek".to_string(),
                },
                top_provider: TopProvider {
                    is_moderated: false,
                },
                created: 1730419200, // November 2024
                hugging_face_id: None,
                per_request_limits: None,
                supported_parameters: None,
            },
            // Llama 3.3 70B - Recent open-source model with tool support
            ModelInfo {
                id: "meta-llama/llama-3.3-70b-instruct".to_string(),
                name: "Llama 3.3 70B Instruct".to_string(),
                description: "Meta's latest open-source model with strong coding performance and tool support".to_string(),
                pricing: Pricing {
                    prompt: "0.00000059".to_string(),
                    completion: "0.00000079".to_string(),
                    request: Some("0.0".to_string()),
                    image: Some("0.0".to_string()),
                    input_cache_read: None,
                    input_cache_write: None,
                    web_search: None,
                    internal_reasoning: None,
                },
                context_length: 128000,
                architecture: Architecture {
                    input_modalities: vec!["text".to_string()],
                    output_modalities: vec!["text".to_string()],
                    tokenizer: "llama".to_string(),
                },
                top_provider: TopProvider {
                    is_moderated: false,
                },
                created: 1733097600, // December 2024
                hugging_face_id: None,
                per_request_limits: None,
                supported_parameters: None,
            },
        ];
        
        // Filter to only include models released in the last 4 months
        self.available_models = default_models.into_iter()
            .filter(|model| model.created >= four_months_ago)
            .collect();
        
        self.apply_filters();
    }

    fn should_refresh(&self) -> bool {
        // Only suggest refresh if we have very few models or it's been a long time
        self.available_models.len() < 3 || 
        self.last_refresh.elapsed() > std::time::Duration::from_secs(1800) // 30 minutes
    }

    fn refresh_models(&mut self) {
        if let Some(ref model_manager) = self.model_manager {
            self.loading = true;
            self.error_message = None;
            
            // Clone the model manager for the async task
            let model_manager_clone = model_manager.clone();
            let (sender, receiver) = std::sync::mpsc::channel();
            
            // Spawn async task to fetch models in a separate thread
            std::thread::spawn(move || {
                // Create a new runtime in this separate thread
                let rt = match tokio::runtime::Runtime::new() {
                    Ok(rt) => rt,
                    Err(e) => {
                        let _ = sender.send(Err(crate::llm::openrouter::error::OpenRouterError::ConfigError(
                            format!("Failed to create async runtime: {}", e)
                        )));
                        return;
                    }
                };
                
                let result = rt.block_on(model_manager_clone.get_available_models(None));
                let _ = sender.send(result);
            });
            
            // Try to receive the result immediately (non-blocking)
            match receiver.try_recv() {
                Ok(Ok(models)) => {
                    log::info!("Successfully loaded {} models from OpenRouter API", models.len());
                    self.available_models = models;
                    self.loading = false;
                    self.error_message = None;
                }
                Ok(Err(e)) => {
                    log::error!("Failed to load models from OpenRouter API: {}", e);
                    self.error_message = Some("Unable to connect to OpenRouter API, falling back to default models".to_string());
                    self.loading = false;
                    // Keep existing default models as fallback
                }
                Err(_) => {
                    // Channel is empty, async task is still running
                    // Store the receiver for checking in render loop
                    log::info!("Model refresh in progress...");
                    self.error_message = Some("Loading models from OpenRouter API...".to_string());
                    self.pending_refresh_receiver = Some(receiver);
                }
            }
            
            self.apply_filters();
            self.last_refresh = std::time::Instant::now();
        } else {
            self.error_message = Some("No model manager available. Check OpenRouter API configuration.".to_string());
        }
    }

    /// Check for completed async model loading operations (optimized to prevent excessive calls)
    fn check_pending_refresh(&mut self) {
        // Throttle checking to once every 100ms to prevent excessive render loop calls
        if self.last_pending_check.elapsed() < std::time::Duration::from_millis(100) {
            return;
        }
        self.last_pending_check = std::time::Instant::now();
        
        // Only check if we actually have a pending operation
        if let Some(receiver) = self.pending_refresh_receiver.take() {
            match receiver.try_recv() {
                Ok(Ok(models)) => {
                    log::info!("Successfully loaded {} models from OpenRouter API (async)", models.len());
                    self.available_models = models;
                    self.loading = false;
                    self.error_message = None;
                    self.apply_filters();
                }
                Ok(Err(e)) => {
                    log::error!("Failed to load models from OpenRouter API (async): {}", e);
                    self.error_message = Some("Unable to connect to OpenRouter API, falling back to default models".to_string());
                    self.loading = false;
                    // Keep existing default models as fallback
                }
                Err(_) => {
                    // Still waiting, put the receiver back
                    self.pending_refresh_receiver = Some(receiver);
                }
            }
        }
    }

    /// Force refresh models synchronously (for button clicks)
    fn force_refresh_models(&mut self) {
        if let Some(ref model_manager) = self.model_manager {
            self.loading = true;
            self.error_message = None;
            
            // Use a blocking approach for immediate refresh
            let model_manager_clone = model_manager.clone();
            
            // Use spawn_blocking with a separate thread that creates its own runtime
            // This avoids the "Cannot start a runtime from within a runtime" error
            let (sender, receiver) = std::sync::mpsc::channel();
            
            std::thread::spawn(move || {
                // Create a new runtime in this separate thread
                let rt = match tokio::runtime::Runtime::new() {
                    Ok(rt) => rt,
                    Err(e) => {
                        let _ = sender.send(Err(crate::llm::openrouter::error::OpenRouterError::ConfigError(
                            format!("Failed to create async runtime: {}", e)
                        )));
                        return;
                    }
                };
                
                let result = rt.block_on(model_manager_clone.get_available_models(None));
                let _ = sender.send(result);
            });
            
            // Wait for the result with a timeout
            match receiver.recv_timeout(std::time::Duration::from_secs(10)) {
                Ok(Ok(models)) => {
                    log::info!("Successfully loaded {} models from OpenRouter API", models.len());
                    self.available_models = models;
                    self.error_message = None;
                }
                Ok(Err(e)) => {
                    log::error!("Failed to load models from OpenRouter API: {}", e);
                    self.error_message = Some("Unable to connect to OpenRouter API, falling back to default models".to_string());
                    // Keep existing default models as fallback
                }
                Err(_) => {
                    log::error!("Timeout waiting for models from OpenRouter API");
                    self.error_message = Some("Timeout loading models. Please try again.".to_string());
                    // Keep existing default models as fallback
                }
            }
            
            self.loading = false;
            self.apply_filters();
            self.last_refresh = std::time::Instant::now();
        } else {
            self.error_message = Some("No model manager available. Check OpenRouter API configuration.".to_string());
        }
    }

    fn is_programming_capable(&self, model: &crate::llm::openrouter::api::ModelInfo) -> bool {
        // Check if model is suitable for programming tasks
        let model_id = model.id.to_lowercase();
        let description = model.description.to_lowercase();
        
        // Include models that are good for coding or general purpose
        model_id.contains("code") || 
        model_id.contains("gpt") ||
        model_id.contains("claude") ||
        model_id.contains("gemini") ||
        model_id.contains("llama") ||
        model_id.contains("mistral") ||
        model_id.contains("deepseek") ||
        description.contains("code") ||
        description.contains("programming") ||
        description.contains("general") ||
        description.contains("assistant")
    }

    fn apply_filters(&mut self) {
        let mut models = self.available_models.clone();

        // Search filter
        if !self.search_query.is_empty() {
            let query = self.search_query.to_lowercase();
            models.retain(|model| {
                model.id.to_lowercase().contains(&query) ||
                model.name.to_lowercase().contains(&query) ||
                model.description.to_lowercase().contains(&query)
            });
        }

        // Provider filter
        if let Some(ref provider) = self.selected_provider {
            models.retain(|model| model.id.starts_with(&format!("{}/", provider)));
        }

        // Category filter
        if let Some(ref category) = self.selected_category {
            models.retain(|model| self.model_matches_category(model, category));
        }

        // Price filter
        models.retain(|model| {
            if let Ok(price) = model.pricing.prompt.parse::<f64>() {
                price >= self.price_range.0 && price <= self.price_range.1
            } else {
                true // Include models with unparseable pricing
            }
        });

        // Context length filter
        if self.min_context_length > 0 {
            models.retain(|model| model.context_length >= self.min_context_length);
        }

        // Capability filters
        if self.show_only_tool_capable {
            models.retain(|model| self.is_tool_capable(model));
        }

        if self.show_only_vision {
            models.retain(|model| self.is_vision_capable(model));
        }

        if self.show_only_reasoning {
            models.retain(|model| self.is_reasoning_capable(model));
        }

        // Sort models
        self.sort_models(&mut models);

        self.filtered_models = models;
    }

    fn model_matches_category(&self, model: &crate::llm::openrouter::api::ModelInfo, category: &crate::llm::openrouter::models::ModelCategory) -> bool {
        let model_id = model.id.to_lowercase();
        let description = model.description.to_lowercase();

        match category {
            crate::llm::openrouter::models::ModelCategory::Code => {
                model_id.contains("code") || description.contains("code") || description.contains("programming")
            }
            crate::llm::openrouter::models::ModelCategory::Vision => {
                model.architecture.input_modalities.contains(&"image".to_string()) ||
                model_id.contains("vision") || description.contains("vision")
            }
            crate::llm::openrouter::models::ModelCategory::Reasoning => {
                model_id.contains("reasoning") || model_id.contains("think") || 
                model_id.contains("o1") || description.contains("reasoning")
            }
            crate::llm::openrouter::models::ModelCategory::Creative => {
                description.contains("creative") || description.contains("writing")
            }
            crate::llm::openrouter::models::ModelCategory::Function => {
                // Most modern models support function calling
                true
            }
            crate::llm::openrouter::models::ModelCategory::Chat => {
                // Default category for general conversation models
                true
            }
        }
    }

    fn sort_models(&self, models: &mut Vec<crate::llm::openrouter::api::ModelInfo>) {
        match self.sort_by {
            ModelSortBy::Name => {
                models.sort_by(|a, b| a.id.cmp(&b.id));
            }
            ModelSortBy::Provider => {
                models.sort_by(|a, b| {
                    let a_provider = a.id.split('/').next().unwrap_or("");
                    let b_provider = b.id.split('/').next().unwrap_or("");
                    a_provider.cmp(b_provider)
                });
            }
            ModelSortBy::Price => {
                models.sort_by(|a, b| {
                    let a_price = a.pricing.prompt.parse::<f64>().unwrap_or(0.0);
                    let b_price = b.pricing.prompt.parse::<f64>().unwrap_or(0.0);
                    a_price.partial_cmp(&b_price).unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            ModelSortBy::ContextLength => {
                models.sort_by(|a, b| b.context_length.cmp(&a.context_length));
            }
            ModelSortBy::Created => {
                models.sort_by(|a, b| b.created.cmp(&a.created));
            }
            ModelSortBy::Popularity => {
                // Sort favorites first, then by name
                models.sort_by(|a, b| {
                    let a_fav = self.favorites.contains(&a.id);
                    let b_fav = self.favorites.contains(&b.id);
                    match (a_fav, b_fav) {
                        (true, false) => std::cmp::Ordering::Less,
                        (false, true) => std::cmp::Ordering::Greater,
                        _ => a.id.cmp(&b.id),
                    }
                });
            }
        }
    }

    fn toggle_favorite(&mut self, model_id: &str) {
        if let Some(pos) = self.favorites.iter().position(|id| id == model_id) {
            self.favorites.remove(pos);
        } else {
            self.favorites.push(model_id.to_string());
        }
        self.apply_filters();
    }

    fn toggle_comparison(&mut self, model_id: &str) {
        if let Some(pos) = self.selected_for_comparison.iter().position(|id| id == model_id) {
            self.selected_for_comparison.remove(pos);
        } else if self.selected_for_comparison.len() < 3 {
            self.selected_for_comparison.push(model_id.to_string());
        }
    }

    pub fn render(&mut self, ctx: &Context, theme: AppTheme) -> Option<String> {
        if !self.visible {
            return None;
        }

        // Check for completed async model loading operations
        self.check_pending_refresh();

        let mut selected_model = None;

        egui::SidePanel::right("model_selection_panel")
            .resizable(true)
            .default_width(600.0)
            .frame(egui::Frame::none().fill(theme.panel_background()))
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    // Header
                    ui.horizontal(|ui| {
                        ui.heading("🤖 Model Selection");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("×").clicked() {
                                self.visible = false;
                            }
                            if ui.button("🔄 Refresh").clicked() {
                                self.force_refresh_models();
                            }
                        });
                    });
                    ui.separator();

                    // Current model display
                    if !self.current_model.is_empty() {
                        ui.horizontal(|ui| {
                            ui.label("Current:");
                            ui.colored_label(theme.accent_color(), &self.current_model);
                        });
                        ui.separator();
                    }

                    // Error display
                    if let Some(ref error) = self.error_message {
                        ui.colored_label(theme.error_color(), format!("Error: {}", error));
                        ui.separator();
                    }

                    // Loading indicator
                    if self.loading {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Loading models...");
                        });
                        ui.separator();
                    }

                    // Filters and controls
                    ui.collapsing("🔍 Filters & Search", |ui| {
                        // Search
                        ui.horizontal(|ui| {
                            ui.label("Search:");
                            if ui.add(TextEdit::singleline(&mut self.search_query)
                                .hint_text("Search models...")).changed() {
                                self.apply_filters();
                            }
                        });

                        ui.horizontal(|ui| {
                            // Provider filter
                            ComboBox::from_label("Provider")
                                .selected_text(self.selected_provider.as_deref().unwrap_or("All"))
                                .show_ui(ui, |ui| {
                                    if ui.selectable_value(&mut self.selected_provider, None, "All").changed() {
                                        self.apply_filters();
                                    }
                                    let providers = self.get_available_providers();
                                    for provider in providers {
                                        if ui.selectable_value(&mut self.selected_provider, Some(provider.clone()), &provider).changed() {
                                            self.apply_filters();
                                        }
                                    }
                                });

                            // Category filter
                            ComboBox::from_label("Category")
                                .selected_text(match &self.selected_category {
                                    Some(cat) => format!("{:?}", cat),
                                    None => "All".to_string(),
                                })
                                .show_ui(ui, |ui| {
                                    if ui.selectable_value(&mut self.selected_category, None, "All").changed() {
                                        self.apply_filters();
                                    }
                                    use crate::llm::openrouter::models::ModelCategory;
                                    for category in [ModelCategory::Code, ModelCategory::Chat, ModelCategory::Vision, ModelCategory::Reasoning, ModelCategory::Creative, ModelCategory::Function] {
                                        if ui.selectable_value(&mut self.selected_category, Some(category.clone()), format!("{:?}", category)).changed() {
                                            self.apply_filters();
                                        }
                                    }
                                });
                        });

                        // Capability filters
                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut self.show_only_tool_capable, "Tools/Functions").changed() {
                                self.apply_filters();
                            }
                            if ui.checkbox(&mut self.show_only_vision, "Vision").changed() {
                                self.apply_filters();
                            }
                            if ui.checkbox(&mut self.show_only_reasoning, "Reasoning").changed() {
                                self.apply_filters();
                            }
                        });

                        // Sort and view options
                        ui.horizontal(|ui| {
                            ComboBox::from_label("Sort by")
                                .selected_text(format!("{:?}", self.sort_by))
                                .show_ui(ui, |ui| {
                                    for sort_option in [ModelSortBy::Name, ModelSortBy::Provider, ModelSortBy::Price, ModelSortBy::ContextLength, ModelSortBy::Created, ModelSortBy::Popularity] {
                                        if ui.selectable_value(&mut self.sort_by, sort_option.clone(), format!("{:?}", sort_option)).changed() {
                                            self.apply_filters();
                                        }
                                    }
                                });

                            ComboBox::from_label("View")
                                .selected_text(format!("{:?}", self.view_mode))
                                .show_ui(ui, |ui| {
                                    for view_option in [ModelViewMode::Cards, ModelViewMode::List, ModelViewMode::Table] {
                                        ui.selectable_value(&mut self.view_mode, view_option.clone(), format!("{:?}", view_option));
                                    }
                                });
                        });
                    });

                    ui.separator();

                    // Model list/cards
                    ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if self.filtered_models.is_empty() && !self.loading {
                                ui.centered_and_justified(|ui| {
                                    ui.label("No models found. Try adjusting your filters or refresh the list.");
                                });
                            } else {
                                match self.view_mode {
                                    ModelViewMode::Cards => {
                                        selected_model = self.render_model_cards(ui, theme);
                                    }
                                    ModelViewMode::List => {
                                        selected_model = self.render_model_list(ui, theme);
                                    }
                                    ModelViewMode::Table => {
                                        selected_model = self.render_model_table(ui, theme);
                                    }
                                }
                            }
                        });

                    // Footer with model count
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(format!("Showing {} of {} models", 
                            self.filtered_models.len(), 
                            self.available_models.len()));
                        
                        if !self.selected_for_comparison.is_empty() {
                            ui.separator();
                            ui.label(format!("Compare ({})", self.selected_for_comparison.len()));
                            if ui.button("Compare Selected").clicked() {
                                self.show_comparison = true;
                            }
                        }
                    });

                    ui.separator();

                    // Manual model entry section
                    ui.collapsing("✏️ Manual Model Entry", |ui| {
                        ui.label("Enter any OpenRouter model ID directly:");
                        ui.horizontal(|ui| {
                            ui.add(TextEdit::singleline(&mut self.manual_model_entry)
                                .hint_text("e.g., anthropic/claude-3.5-sonnet, openai/gpt-4o"));
                            
                            let use_enabled = !self.manual_model_entry.trim().is_empty();
                            if ui.add_enabled(use_enabled, Button::new("Use Model")).clicked() {
                                selected_model = Some(self.manual_model_entry.trim().to_string());
                            }
                        });
                        
                        ui.label(RichText::new("💡 Tip: You can use any model from OpenRouter's catalog, even if it's not in the list above.")
                            .small().weak());
                        
                        // Quick access to popular models not in default list
                        ui.horizontal_wrapped(|ui| {
                            ui.label("Quick access:");
                            let quick_models = [
                                "google/gemini-1.5-pro",
                                "google/gemini-1.5-flash", 
                                "mistralai/mistral-large",
                                "anthropic/claude-3-haiku",
                                "openai/o1-preview",
                                "openai/o1-mini"
                            ];
                            
                            for model in quick_models {
                                if ui.small_button(model).clicked() {
                                    self.manual_model_entry = model.to_string();
                                }
                            }
                        });
                    });

                    ui.separator();
                });
            });

        selected_model
    }

    fn get_available_providers(&self) -> Vec<String> {
        let mut providers: std::collections::HashSet<String> = std::collections::HashSet::new();
        for model in &self.available_models {
            if let Some(provider) = model.id.split('/').next() {
                providers.insert(provider.to_string());
            }
        }
        let mut provider_list: Vec<String> = providers.into_iter().collect();
        provider_list.sort();
        provider_list
    }

    fn render_model_cards(&mut self, ui: &mut egui::Ui, theme: AppTheme) -> Option<String> {
        let mut selected_model = None;
        
        // Clone the filtered models to avoid borrowing issues
        let models = self.filtered_models.clone();
        let current_model = self.current_model.clone();
        let favorites = self.favorites.clone();
        let selected_for_comparison = self.selected_for_comparison.clone();
        
        for model in &models {
            ui.group(|ui| {
                ui.vertical(|ui| {
                    // Model header
                    ui.horizontal(|ui| {
                        // Model name
                        if ui.add(Button::new(&model.id)
                            .fill(if current_model == model.id { 
                                theme.accent_color() 
                            } else { 
                                Color32::TRANSPARENT 
                            })).clicked() {
                            selected_model = Some(model.id.clone());
                        }
                        
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Comparison toggle
                            let compare_text = if selected_for_comparison.contains(&model.id) { "📊" } else { "📈" };
                            if ui.small_button(compare_text).on_hover_text("Add to comparison").clicked() {
                                // We'll handle this after the loop
                            }
                            
                            // Favorite toggle
                            let star_text = if favorites.contains(&model.id) { "⭐" } else { "☆" };
                            if ui.small_button(star_text).on_hover_text("Toggle favorite").clicked() {
                                // We'll handle this after the loop
                            }
                        });
                    });
                    
                    // Model details
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            // Creation date
                            let creation_date = self.format_creation_date(model.created);
                            let date_color = if model.created > (std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs() - 30 * 24 * 60 * 60) { // Last 30 days
                                Color32::from_rgb(0, 180, 0) // Green for recent
                            } else if model.created > (std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs() - 365 * 24 * 60 * 60) { // Last year
                                Color32::from_rgb(255, 165, 0) // Orange for somewhat recent
                            } else {
                                Color32::from_rgb(128, 128, 128) // Gray for old
                            };
                            ui.colored_label(date_color, format!("📅 Added {}", creation_date));

                            // Pricing - show per million tokens for readability
                            if let (Ok(prompt_price), Ok(completion_price)) = (
                                model.pricing.prompt.parse::<f64>(),
                                model.pricing.completion.parse::<f64>()
                            ) {
                                ui.label(format!("💰 ${:.3} / ${:.3} per 1M tokens", 
                                    prompt_price * 1_000_000.0, 
                                    completion_price * 1_000_000.0));
                            }
                            
                            // Context length
                            ui.label(format!("📏 {}k context", model.context_length / 1000));
                            
                            // Capabilities
                            let capabilities = self.get_model_capabilities(model);
                            if !capabilities.is_empty() {
                                ui.horizontal_wrapped(|ui| {
                                    for capability in &capabilities {
                                        ui.small(capability);
                                    }
                                });
                            }
                        });
                    });
                    
                    // Description
                    if !model.description.is_empty() {
                        ui.label(RichText::new(&model.description).small().weak());
                    }
                });
            });
            ui.add_space(4.0);
        }
        
        selected_model
    }

    fn render_model_list(&mut self, ui: &mut egui::Ui, theme: AppTheme) -> Option<String> {
        let mut selected_model = None;
        
        // Clone the filtered models to avoid borrowing issues
        let models = self.filtered_models.clone();
        let current_model = self.current_model.clone();
        let favorites = self.favorites.clone();
        
        for model in &models {
            ui.horizontal(|ui| {
                // Model selection button
                if ui.add(Button::new(&model.id)
                    .fill(if current_model == model.id { 
                        theme.accent_color() 
                    } else { 
                        Color32::TRANSPARENT 
                    })).clicked() {
                    selected_model = Some(model.id.clone());
                }
                
                // Creation date (compact)
                let creation_date = self.format_creation_date(model.created);
                let date_color = if model.created > (std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() - 30 * 24 * 60 * 60) { // Last 30 days
                    Color32::from_rgb(0, 180, 0) // Green for recent
                } else if model.created > (std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() - 365 * 24 * 60 * 60) { // Last year
                    Color32::from_rgb(255, 165, 0) // Orange for somewhat recent
                } else {
                    Color32::from_rgb(128, 128, 128) // Gray for old
                };
                ui.colored_label(date_color, creation_date);
                
                // Quick info - show pricing per million tokens
                if let (Ok(prompt_price), Ok(completion_price)) = (
                    model.pricing.prompt.parse::<f64>(),
                    model.pricing.completion.parse::<f64>()
                ) {
                    ui.label(format!("${:.3}/${:.3}/1M", 
                        prompt_price * 1_000_000.0, 
                        completion_price * 1_000_000.0));
                } else {
                    ui.label("N/A");
                }
                
                ui.label(format!("{}k", model.context_length / 1000));
                
                // Show key capabilities
                let capabilities = self.get_model_capabilities(model);
                if !capabilities.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        for capability in capabilities.iter().take(3) { // Show first 3 capabilities
                            ui.small(capability);
                        }
                        if capabilities.len() > 3 {
                            ui.small("...");
                        }
                    });
                }
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Favorite toggle
                    let star_text = if favorites.contains(&model.id) { "⭐" } else { "☆" };
                    if ui.small_button(star_text).clicked() {
                        // We'll handle this after the loop
                    }
                });
            });
        }
        
        selected_model
    }

    fn render_model_table(&mut self, ui: &mut egui::Ui, theme: AppTheme) -> Option<String> {
        let mut selected_model = None;
        
        // Clone the filtered models to avoid borrowing issues
        let models = self.filtered_models.clone();
        let current_model = self.current_model.clone();
        let favorites = self.favorites.clone();
        
        egui::Grid::new("model_table")
            .striped(true)
            .show(ui, |ui| {
                // Header
                ui.label(RichText::new("Model").strong());
                ui.label(RichText::new("Provider").strong());
                ui.label(RichText::new("Created").strong());
                ui.label(RichText::new("Price (per 1M)").strong());
                ui.label(RichText::new("Context").strong());
                ui.label(RichText::new("Capabilities").strong());
                ui.label(RichText::new("Actions").strong());
                ui.end_row();
                
                // Rows
                for model in &models {
                    // Model name
                    if ui.add(Button::new(&model.name)
                        .fill(if current_model == model.id { 
                            theme.accent_color() 
                        } else { 
                            Color32::TRANSPARENT 
                        })).clicked() {
                        selected_model = Some(model.id.clone());
                    }
                    
                    // Provider
                    ui.label(model.id.split('/').next().unwrap_or("Unknown"));
                    
                    // Creation date
                    let creation_date = self.format_creation_date(model.created);
                    let date_color = if model.created > (std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() - 30 * 24 * 60 * 60) { // Last 30 days
                        Color32::from_rgb(0, 180, 0) // Green for recent
                    } else if model.created > (std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() - 365 * 24 * 60 * 60) { // Last year
                        Color32::from_rgb(255, 165, 0) // Orange for somewhat recent
                    } else {
                        Color32::from_rgb(128, 128, 128) // Gray for old
                    };
                    ui.colored_label(date_color, creation_date);
                    
                    // Price (show both prompt and completion)
                    if let (Ok(prompt_price), Ok(completion_price)) = (
                        model.pricing.prompt.parse::<f64>(),
                        model.pricing.completion.parse::<f64>()
                    ) {
                        ui.label(format!("${:.3} / ${:.3}", 
                            prompt_price * 1_000_000.0, 
                            completion_price * 1_000_000.0));
                    } else {
                        ui.label("N/A");
                    }
                    
                    // Context
                    ui.label(format!("{}k", model.context_length / 1000));
                    
                    // Capabilities
                    let capabilities = self.get_model_capabilities(model);
                    if capabilities.is_empty() {
                        ui.label("Basic");
                    } else {
                        ui.horizontal_wrapped(|ui| {
                            for capability in &capabilities {
                                ui.small(capability);
                            }
                        });
                    }
                    
                    // Actions
                    ui.horizontal(|ui| {
                        let star_text = if favorites.contains(&model.id) { "⭐" } else { "☆" };
                        if ui.small_button(star_text).clicked() {
                            // We'll handle this after the loop
                        }
                    });
                    
                    ui.end_row();
                }
            });
        
        selected_model
    }

    /// Check if a model is tool/function calling capable
    fn is_tool_capable(&self, model: &crate::llm::openrouter::api::ModelInfo) -> bool {
        // Check supported_parameters for "tools" capability
        if let Some(ref params) = model.supported_parameters {
            return params.iter().any(|p| p == "tools" || p == "tool_choice");
        }
        
        // Fallback to heuristics for models without supported_parameters data
        let id = model.id.to_lowercase();
        let description = model.description.to_lowercase();
        
        // Modern models that definitely support tools
        id.contains("gpt-4") || 
        id.contains("gpt-3.5-turbo") ||
        id.contains("claude-3") || 
        id.contains("claude-2") ||
        id.contains("gemini") ||
        id.contains("mistral") ||
        (id.contains("llama") && (id.contains("3.") || id.contains("-3"))) ||
        id.contains("deepseek") ||
        description.contains("function") ||
        description.contains("tool") ||
        model.created > 1672531200 // After Jan 1, 2023
    }

    /// Check if a model supports reasoning mode
    fn is_reasoning_capable(&self, model: &crate::llm::openrouter::api::ModelInfo) -> bool {
        if let Some(ref params) = model.supported_parameters {
            return params.iter().any(|p| p == "reasoning" || p == "include_reasoning");
        }
        
        // Fallback heuristics
        let id = model.id.to_lowercase();
        id.contains("o1") || 
        id.contains("reasoning") ||
        id.contains("think")
    }

    /// Check if a model supports vision/image inputs
    fn is_vision_capable(&self, model: &crate::llm::openrouter::api::ModelInfo) -> bool {
        // Check architecture for image input modality
        if model.architecture.input_modalities.contains(&"image".to_string()) {
            return true;
        }
        
        // Check supported_parameters (some models may support images without explicit architecture)
        if let Some(ref params) = model.supported_parameters {
            if params.iter().any(|p| p.contains("image")) {
                return true;
            }
        }
        
        // Fallback heuristics
        let id = model.id.to_lowercase();
        let description = model.description.to_lowercase();
        id.contains("vision") || 
        id.contains("gpt-4o") || 
        id.contains("claude-3") ||
        id.contains("gemini") ||
        description.contains("vision") ||
        description.contains("image")
    }

    /// Get a user-friendly list of supported capabilities for a model
    fn get_model_capabilities(&self, model: &crate::llm::openrouter::api::ModelInfo) -> Vec<String> {
        let mut capabilities = Vec::new();
        
        if let Some(ref params) = model.supported_parameters {
            // Map supported parameters to user-friendly capability names
            for param in params {
                match param.as_str() {
                    "tools" | "tool_choice" => {
                        if !capabilities.contains(&"🔧 Tools".to_string()) {
                            capabilities.push("🔧 Tools".to_string());
                        }
                    }
                    "reasoning" | "include_reasoning" => {
                        if !capabilities.contains(&"🧠 Reasoning".to_string()) {
                            capabilities.push("🧠 Reasoning".to_string());
                        }
                    }
                    "structured_outputs" | "response_format" => {
                        if !capabilities.contains(&"📋 Structured".to_string()) {
                            capabilities.push("📋 Structured".to_string());
                        }
                    }
                    "max_tokens" => capabilities.push("📏 Length Control".to_string()),
                    "temperature" => capabilities.push("🌡️ Temperature".to_string()),
                    "top_p" => capabilities.push("🎯 Top-P".to_string()),
                    "stop" => capabilities.push("🛑 Stop Sequences".to_string()),
                    "frequency_penalty" => capabilities.push("🔄 Frequency Penalty".to_string()),
                    "presence_penalty" => capabilities.push("🎭 Presence Penalty".to_string()),
                    "seed" => capabilities.push("🎲 Deterministic".to_string()),
                    _ => {} // Skip unknown parameters
                }
            }
        }
        
        // Add vision capability if supported
        if self.is_vision_capable(model) {
            capabilities.insert(0, "👁️ Vision".to_string());
        }
        
        // If no supported_parameters, fall back to basic heuristics
        if capabilities.is_empty() {
            if self.is_tool_capable(model) {
                capabilities.push("🔧 Tools".to_string());
            }
            if self.is_reasoning_capable(model) {
                capabilities.push("🧠 Reasoning".to_string());
            }
            if self.is_vision_capable(model) {
                capabilities.push("👁️ Vision".to_string());
            }
        }
        
        capabilities
    }

    /// Format the model creation date in a user-friendly way
    fn format_creation_date(&self, created_timestamp: u64) -> String {
        use std::time::{SystemTime, UNIX_EPOCH, Duration};
        
        let created_time = UNIX_EPOCH + Duration::from_secs(created_timestamp);
        let now = SystemTime::now();
        
        if let Ok(duration_since) = now.duration_since(created_time) {
            let days_ago = duration_since.as_secs() / (24 * 60 * 60);
            
            if days_ago == 0 {
                "Today".to_string()
            } else if days_ago == 1 {
                "Yesterday".to_string()
            } else if days_ago < 7 {
                format!("{} days ago", days_ago)
            } else if days_ago < 30 {
                let weeks_ago = days_ago / 7;
                if weeks_ago == 1 {
                    "1 week ago".to_string()
                } else {
                    format!("{} weeks ago", weeks_ago)
                }
            } else if days_ago < 365 {
                let months_ago = days_ago / 30;
                if months_ago == 1 {
                    "1 month ago".to_string()
                } else {
                    format!("{} months ago", months_ago)
                }
            } else {
                let years_ago = days_ago / 365;
                if years_ago == 1 {
                    "1 year ago".to_string()
                } else {
                    format!("{} years ago", years_ago)
                }
            }
        } else {
            // If timestamp is in the future, try to format as date
            use chrono::{DateTime, Utc, TimeZone};
            if let Some(datetime) = Utc.timestamp_opt(created_timestamp as i64, 0).single() {
                datetime.format("%Y-%m-%d").to_string()
            } else {
                "Unknown".to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;


    #[test]
    fn test_panel_manager_close_all_includes_model_selection() {
        let mut panel_manager = PanelManager::new();
        
        // Open model selection panel
        panel_manager.toggle_panel(ActivePanel::ModelSelection);
        assert!(panel_manager.model_selection_panel.visible);
        
        // Close all panels
        panel_manager.close_all_panels();
        assert!(!panel_manager.model_selection_panel.visible);
        assert_eq!(panel_manager.active_panel, ActivePanel::None);
    }

    #[test]
    fn test_model_selection_panel_model_management() {
        let mut panel_manager = PanelManager::new();
        
        // Test setting and getting current model
        panel_manager.set_current_model("test-model".to_string());
        assert_eq!(panel_manager.get_current_model(), "test-model");
        
        // Test model selection panel state
        assert!(!panel_manager.model_selection_panel.visible);
        assert_eq!(panel_manager.model_selection_panel.current_model, "test-model");
    }

    // ===== Phase 7 Analytics Dashboard Tests =====

    #[test]
    fn test_analytics_panel_enhanced_initialization() {
        let panel = AnalyticsPanel::new();
        assert!(!panel.visible);
        assert!(panel.all_usage_entries.is_empty());
        assert!(panel.conversation_summaries.is_empty());
        assert_eq!(panel.time_filter, TimeFilter::AllTime);
        assert!(panel.selected_conversation_id.is_none());
        
        // Test new fields for Phase 7
        assert!(panel.analytics_report.is_none());
        assert_eq!(panel.date_range_filter, DateRangeFilter::Last30Days);
        assert_eq!(panel.project_filter, ProjectFilter::All);
        assert!(!panel.show_success_details);
        assert_eq!(panel.active_tab, AnalyticsTab::Overview);
    }

    #[test]
    fn test_analytics_panel_set_analytics_report() {
        let mut panel = AnalyticsPanel::new();
        
        let report = create_test_analytics_report();
        panel.set_analytics_report(Some(report.clone()));
        
        assert!(panel.analytics_report.is_some());
        let stored_report = panel.analytics_report.as_ref().unwrap();
        assert_eq!(stored_report.overall_metrics.total_conversations, 10);
        assert_eq!(stored_report.success_metrics.overall_success_rate, 0.75);
    }

    #[test]
    fn test_analytics_panel_date_range_filtering() {
        let mut panel = AnalyticsPanel::new();
        
        // Test different date range filters
        panel.date_range_filter = DateRangeFilter::Last7Days;
        assert_eq!(panel.date_range_filter, DateRangeFilter::Last7Days);
        
        panel.date_range_filter = DateRangeFilter::Last30Days;
        assert_eq!(panel.date_range_filter, DateRangeFilter::Last30Days);
        
        panel.date_range_filter = DateRangeFilter::Last90Days;
        assert_eq!(panel.date_range_filter, DateRangeFilter::Last90Days);
        
        panel.date_range_filter = DateRangeFilter::AllTime;
        assert_eq!(panel.date_range_filter, DateRangeFilter::AllTime);
    }

    #[test]
    fn test_analytics_panel_project_filtering() {
        let mut panel = AnalyticsPanel::new();
        
        // Test different project filters
        panel.project_filter = ProjectFilter::All;
        assert_eq!(panel.project_filter, ProjectFilter::All);
        
        panel.project_filter = ProjectFilter::Specific(ProjectType::Rust);
        assert_eq!(panel.project_filter, ProjectFilter::Specific(ProjectType::Rust));
        
        panel.project_filter = ProjectFilter::Specific(ProjectType::Python);
        assert_eq!(panel.project_filter, ProjectFilter::Specific(ProjectType::Python));
    }

    #[test]
    fn test_analytics_panel_tab_switching() {
        let mut panel = AnalyticsPanel::new();
        
        // Test tab switching
        panel.active_tab = AnalyticsTab::Overview;
        assert_eq!(panel.active_tab, AnalyticsTab::Overview);
        
        panel.active_tab = AnalyticsTab::Success;
        assert_eq!(panel.active_tab, AnalyticsTab::Success);
        
        panel.active_tab = AnalyticsTab::Efficiency;
        assert_eq!(panel.active_tab, AnalyticsTab::Efficiency);
        
        panel.active_tab = AnalyticsTab::Patterns;
        assert_eq!(panel.active_tab, AnalyticsTab::Patterns);
        
        panel.active_tab = AnalyticsTab::Projects;
        assert_eq!(panel.active_tab, AnalyticsTab::Projects);
        
        panel.active_tab = AnalyticsTab::Trends;
        assert_eq!(panel.active_tab, AnalyticsTab::Trends);
    }

    #[test]
    fn test_analytics_panel_success_details_toggle() {
        let mut panel = AnalyticsPanel::new();
        
        assert!(!panel.show_success_details);
        
        panel.toggle_success_details();
        assert!(panel.show_success_details);
        
        panel.toggle_success_details();
        assert!(!panel.show_success_details);
    }

    #[test]
    fn test_analytics_panel_link_to_success_mode() {
        let panel = AnalyticsPanel::new();
        
        let action = panel.handle_success_rate_click();
        assert_eq!(action, Some(AnalyticsAction::SwitchToSuccessMode));
    }

    #[test]
    fn test_analytics_panel_refresh_analytics() {
        let panel = AnalyticsPanel::new();
        
        let action = panel.handle_refresh_request();
        assert_eq!(action, Some(AnalyticsAction::RefreshAnalytics));
    }

    #[test]
    fn test_analytics_panel_export_report() {
        let mut panel = AnalyticsPanel::new();
        
        let report = create_test_analytics_report();
        panel.set_analytics_report(Some(report));
        
        let action = panel.handle_export_request();
        assert_eq!(action, Some(AnalyticsAction::ExportReport));
    }

    #[test]
    fn test_analytics_panel_filter_combinations() {
        let mut panel = AnalyticsPanel::new();
        
        // Test combining different filters
        panel.date_range_filter = DateRangeFilter::Last30Days;
        panel.project_filter = ProjectFilter::Specific(ProjectType::Rust);
        
        assert_eq!(panel.date_range_filter, DateRangeFilter::Last30Days);
        assert_eq!(panel.project_filter, ProjectFilter::Specific(ProjectType::Rust));
        
        // Test filter reset
        panel.reset_filters();
        assert_eq!(panel.date_range_filter, DateRangeFilter::Last30Days); // Default
        assert_eq!(panel.project_filter, ProjectFilter::All);
    }

    #[test]
    fn test_analytics_panel_metrics_display() {
        let mut panel = AnalyticsPanel::new();
        let report = create_test_analytics_report();
        panel.set_analytics_report(Some(report));
        
        // Test that metrics are properly accessible
        let report = panel.analytics_report.as_ref().unwrap();
        assert_eq!(report.overall_metrics.total_conversations, 10);
        assert_eq!(report.overall_metrics.total_messages, 150);
        assert_eq!(report.overall_metrics.completion_rate, 0.8);
        assert_eq!(report.success_metrics.overall_success_rate, 0.75);
        assert_eq!(report.efficiency_metrics.avg_resolution_time, 25.5);
    }

    #[test]
    fn test_analytics_panel_project_insights_display() {
        let mut panel = AnalyticsPanel::new();
        let report = create_test_analytics_report();
        panel.set_analytics_report(Some(report));
        
        let report = panel.analytics_report.as_ref().unwrap();
        assert_eq!(report.project_insights.len(), 2);
        assert_eq!(report.project_insights[0].project_type, ProjectType::Rust);
        assert_eq!(report.project_insights[0].conversation_count, 6);
        assert_eq!(report.project_insights[0].success_rate, 0.8);
    }

    #[test]
    fn test_analytics_panel_trending_topics_display() {
        let mut panel = AnalyticsPanel::new();
        let report = create_test_analytics_report();
        panel.set_analytics_report(Some(report));
        
        let report = panel.analytics_report.as_ref().unwrap();
        assert_eq!(report.trending_topics.len(), 2);
        assert_eq!(report.trending_topics[0].topic, "Async Programming");
        assert_eq!(report.trending_topics[0].frequency, 15);
    }

    #[test]
    fn test_analytics_panel_recommendations_display() {
        let mut panel = AnalyticsPanel::new();
        let report = create_test_analytics_report();
        panel.set_analytics_report(Some(report));
        
        let report = panel.analytics_report.as_ref().unwrap();
        assert_eq!(report.recommendations.len(), 2);
        assert!(report.recommendations[0].description.contains("completion rate"));
    }

    // Helper function to create test analytics report
    fn create_test_analytics_report() -> AnalyticsReport {
        use crate::agent::conversation::analytics::*;
        use std::collections::HashMap;
        use chrono::{Utc, Duration};
        
        let now = Utc::now();
        let period = (now - Duration::days(30), now);
        
        let mut project_type_distribution = HashMap::new();
        project_type_distribution.insert(ProjectType::Rust, 6);
        project_type_distribution.insert(ProjectType::Python, 4);
        
        let overall_metrics = OverallMetrics {
            total_conversations: 10,
            total_messages: 150,
            avg_messages_per_conversation: 15.0,
            total_branches: 25,
            total_checkpoints: 12,
            completion_rate: 0.8,
            avg_duration_minutes: 45.0,
            peak_activity_hours: vec![9, 10, 14, 15],
            project_type_distribution,
        };
        
        let mut success_by_project_type = HashMap::new();
        success_by_project_type.insert(ProjectType::Rust, 0.8);
        success_by_project_type.insert(ProjectType::Python, 0.7);
        
        let success_metrics = SuccessMetrics {
            overall_success_rate: 0.75,
            success_by_project_type,
            success_by_length: vec![(5, 0.6), (10, 0.75), (15, 0.85)],
            successful_patterns: vec![],
            failure_points: vec![],
            success_indicators: vec![],
        };
        
        let efficiency_metrics = EfficiencyMetrics {
            avg_resolution_time: 25.5,
            branching_efficiency: 0.68,
            checkpoint_utilization: 0.45,
            context_switches_per_conversation: 2.3,
            efficient_patterns: vec![],
            resource_utilization: ResourceUtilization {
                avg_memory_usage: 1024.0,
                storage_efficiency: 0.85,
                search_performance: SearchPerformance {
                    avg_search_time_ms: 150.0,
                    accuracy_rate: 0.92,
                    common_patterns: vec!["keyword search".to_string()],
                },
                clustering_efficiency: 0.78,
            },
        };
        
        let patterns = PatternAnalysis {
            common_flows: vec![],
            recurring_themes: vec![],
            temporal_patterns: vec![],
            behavior_patterns: vec![],
            anomalies: vec![],
        };
        
        let project_insights = vec![
            ProjectInsight {
                project_type: ProjectType::Rust,
                conversation_count: 6,
                success_rate: 0.8,
                common_topics: vec!["async".to_string(), "ownership".to_string()],
                typical_patterns: vec!["error handling".to_string()],
                recommendations: vec!["more examples".to_string()],
            },
            ProjectInsight {
                project_type: ProjectType::Python,
                conversation_count: 4,
                success_rate: 0.7,
                common_topics: vec!["data science".to_string(), "web dev".to_string()],
                typical_patterns: vec!["debugging".to_string()],
                recommendations: vec!["better docs".to_string()],
            },
        ];
        
        let trending_topics = vec![
            TrendingTopic {
                topic: "Async Programming".to_string(),
                frequency: 15,
                trend: TrendDirection::Growing,
                project_types: vec![ProjectType::Rust, ProjectType::JavaScript],
                success_rate: 0.82,
            },
            TrendingTopic {
                topic: "Error Handling".to_string(),
                frequency: 12,
                trend: TrendDirection::Stable,
                project_types: vec![ProjectType::Rust, ProjectType::Python],
                success_rate: 0.78,
            },
        ];
        
        let recommendations = vec![
            Recommendation {
                category: RecommendationCategory::UserExperience,
                priority: Priority::High,
                description: "Improve conversation completion rate".to_string(),
                expected_impact: "Increase user satisfaction".to_string(),
                difficulty: Difficulty::Medium,
                evidence: vec!["Current rate: 80%".to_string()],
            },
            Recommendation {
                category: RecommendationCategory::Efficiency,
                priority: Priority::Medium,
                description: "Optimize branching efficiency".to_string(),
                expected_impact: "Faster problem resolution".to_string(),
                difficulty: Difficulty::Hard,
                evidence: vec!["Current efficiency: 68%".to_string()],
            },
        ];
        
        AnalyticsReport {
            generated_at: now,
            period,
            overall_metrics,
            success_metrics,
            efficiency_metrics,
            patterns,
            project_insights,
            trending_topics,
            recommendations,
        }
    }
} 