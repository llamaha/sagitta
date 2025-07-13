// Panel management for the Sagitta Code application

use egui::{Context, ScrollArea, RichText, Color32, ComboBox, Button, TextFormat};
use egui_plot::{Line, Plot, Bar, BarChart, Legend, PlotPoints};
use super::super::theme::AppTheme;
use super::super::theme_customizer::ThemeCustomizer;
use super::super::git_history::GitHistoryModal;
use crate::agent::conversation::types::ProjectType;
use crate::agent::conversation::analytics::AnalyticsReport;
use std::path::PathBuf;
use syntect::{
    highlighting::{ThemeSet, Style as SyntectStyle},
    parsing::SyntaxSet,
    easy::HighlightLines,
    util::LinesWithEndings,
};
use std::sync::OnceLock;
use std::path::Path;

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

fn get_syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn get_theme_set() -> &'static ThemeSet {
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

/// Get file extension from path
fn get_file_extension(file_path: &str) -> Option<&str> {
    Path::new(file_path)
        .extension()
        .and_then(|ext| ext.to_str())
}

/// Convert syntect style to egui color
fn syntect_style_to_color(style: &SyntectStyle) -> Color32 {
    Color32::from_rgb(
        style.foreground.r, 
        style.foreground.g, 
        style.foreground.b
    )
}

/// Panel type enum for tracking which panel is currently open
#[derive(Debug, Clone, PartialEq)]
pub enum ActivePanel {
    None,
    Repository,
    Preview,
    Settings,
    Task,
    Conversation,
    Events,
    Analytics,
    ThemeCustomizer,
    ModelSelection,
    GitHistory,
}


/// Preview panel for tool outputs and code changes
pub struct PreviewPanel {
    pub visible: bool,
    pub content: String,
    pub title: String,
}

impl Default for PreviewPanel {
    fn default() -> Self {
        Self::new()
    }
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
            .frame(egui::Frame::NONE.fill(theme.panel_background()))
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
                            // Check if this is a file read tool result
                            if self.title.contains("Read") || self.title.contains("read_file") || self.title.contains("view_file") {
                                // Try to extract file path from the formatted content
                                let file_path = self.extract_file_path_from_content();
                                
                                if let Some(path) = file_path {
                                    // Apply syntax highlighting
                                    self.render_with_syntax_highlighting(ui, &path, theme);
                                } else {
                                    // Fallback to plain rendering
                                    self.render_formatted_content(ui, theme);
                                }
                            } else {
                                // Not a file read result, use formatted rendering
                                self.render_formatted_content(ui, theme);
                            }
                        });
                });
            });
    }
    
    /// Extract file path from the formatted content
    fn extract_file_path_from_content(&self) -> Option<String> {
        // Look for **File:** pattern in the formatted content
        if let Some(file_line_start) = self.content.find("**File:** ") {
            let start = file_line_start + 10; // Length of "**File:** "
            if let Some(end) = self.content[start..].find('\n') {
                return Some(self.content[start..start + end].to_string());
            }
        }
        None
    }
    
    /// Extract the actual file content from the formatted output
    fn extract_file_content(&self) -> Option<String> {
        // Look for the content between ``` (with optional language) and \n```
        if let Some(code_block_start) = self.content.find("```") {
            // Find the end of the first line after ```
            let first_line_end = self.content[code_block_start..].find('\n')?;
            let content_start = code_block_start + first_line_end + 1;
            
            // Find the closing ```
            if let Some(content_end) = self.content[content_start..].rfind("\n```") {
                return Some(self.content[content_start..content_start + content_end].to_string());
            }
        }
        None
    }
    
    /// Render formatted content (for non-file content or fallback)
    fn render_formatted_content(&self, ui: &mut egui::Ui, _theme: AppTheme) {
        // Use monospace font for better readability
        let font_id = egui::FontId::monospace(12.0);
        ui.label(egui::RichText::new(&self.content).font(font_id));
    }
    
    /// Render content with syntax highlighting based on file extension
    fn render_with_syntax_highlighting(&self, ui: &mut egui::Ui, file_path: &str, theme: AppTheme) {
        // Extract the actual file content from the formatted output
        let content_to_highlight = self.extract_file_content()
            .unwrap_or_else(|| self.content.clone());
        
        let syntax_set = get_syntax_set();
        let theme_set = get_theme_set();
        
        // Get the appropriate syntect theme based on app theme
        let syntect_theme = match theme {
            AppTheme::Dark | AppTheme::Custom => theme_set.themes.get("base16-ocean.dark"),
            AppTheme::Light => theme_set.themes.get("base16-ocean.light"),
        }.unwrap_or(&theme_set.themes["base16-ocean.dark"]);
        
        // Get file extension
        let extension = get_file_extension(file_path);
        
        // Find appropriate syntax with fallbacks for languages not in syntect
        let syntax = extension
            .and_then(|ext| {
                match ext {
                    // TypeScript/JSX fallback to JavaScript
                    "ts" | "tsx" => syntax_set.find_syntax_by_extension("js")
                        .or_else(|| syntax_set.find_syntax_by_name("JavaScript")),
                    "jsx" => syntax_set.find_syntax_by_extension("js")
                        .or_else(|| syntax_set.find_syntax_by_name("JavaScript")),
                    // Handle TOML fallback
                    "toml" => syntax_set.find_syntax_by_extension("toml")
                        .or_else(|| syntax_set.find_syntax_by_extension("ini"))
                        .or_else(|| Some(syntax_set.find_syntax_plain_text())),
                    // Golang
                    "go" => syntax_set.find_syntax_by_extension("go")
                        .or_else(|| syntax_set.find_syntax_by_name("Go")),
                    // HTML
                    "html" | "htm" => syntax_set.find_syntax_by_extension("html")
                        .or_else(|| syntax_set.find_syntax_by_name("HTML")),
                    // YAML
                    "yaml" | "yml" => syntax_set.find_syntax_by_extension("yaml")
                        .or_else(|| syntax_set.find_syntax_by_name("YAML")),
                    // Python
                    "py" | "pyw" => syntax_set.find_syntax_by_extension("py")
                        .or_else(|| syntax_set.find_syntax_by_name("Python")),
                    // Ruby
                    "rb" | "rake" | "gemspec" => syntax_set.find_syntax_by_extension("rb")
                        .or_else(|| syntax_set.find_syntax_by_name("Ruby")),
                    // Rust
                    "rs" => syntax_set.find_syntax_by_extension("rs")
                        .or_else(|| syntax_set.find_syntax_by_name("Rust")),
                    // Markdown
                    "md" | "markdown" => syntax_set.find_syntax_by_extension("md")
                        .or_else(|| syntax_set.find_syntax_by_name("Markdown")),
                    // JSON
                    "json" => syntax_set.find_syntax_by_extension("json")
                        .or_else(|| syntax_set.find_syntax_by_name("JSON")),
                    // Default case
                    _ => syntax_set.find_syntax_by_extension(ext),
                }
            })
            .unwrap_or_else(|| syntax_set.find_syntax_plain_text());
        
        let mut highlighter = HighlightLines::new(syntax, syntect_theme);
        
        // Use a monospace font for code
        let font_id = egui::FontId::monospace(12.0);
        
        // Show file path header
        ui.label(egui::RichText::new(format!("File: {file_path}"))
            .font(font_id.clone())
            .color(theme.accent_color()));
        ui.separator();
        
        // Process each line
        for line in LinesWithEndings::from(&content_to_highlight) {
            if let Ok(ranges) = highlighter.highlight_line(line, syntax_set) {
                ui.horizontal(|ui| {
                    let mut job = egui::text::LayoutJob::default();
                    
                    for (style, text) in ranges {
                        let color = syntect_style_to_color(&style);
                        job.append(
                            text,
                            0.0,
                            TextFormat {
                                font_id: font_id.clone(),
                                color,
                                ..Default::default()
                            }
                        );
                    }
                    
                    ui.label(job);
                });
            } else {
                // Fallback to plain text if highlighting fails
                ui.label(egui::RichText::new(line).font(font_id.clone()));
            }
        }
    }
}

/// Logging panel for displaying Sagitta Code logs
pub struct LoggingPanel {
    pub visible: bool,
    pub logs: Vec<(std::time::SystemTime, String)>, // (timestamp, log line)
    pub filter_sagitta_code_only: bool,
}

impl Default for LoggingPanel {
    fn default() -> Self {
        Self::new()
    }
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
            .frame(egui::Frame::NONE.fill(theme.panel_background()))
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

impl Default for EventsPanel {
    fn default() -> Self {
        Self::new()
    }
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
            .frame(egui::Frame::NONE.fill(theme.panel_background()))
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
                                        format!("{elapsed}s ago")
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
    pub git_history_modal: GitHistoryModal,
}

impl Default for PanelManager {
    fn default() -> Self {
        Self::new()
    }
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
            git_history_modal: GitHistoryModal::new(),
        }
    }


    /// Set the current model in the model selection panel
    pub fn set_current_model(&mut self, model: String) {
        self.model_selection_panel.set_current_model(model);
    }

    /// Get the current model from the model selection panel
    pub fn get_current_model(&self) -> &str {
        self.model_selection_panel.get_current_model()
    }

    /// Set the repository path for git history modal
    pub fn set_git_repository(&mut self, path: PathBuf) {
        self.git_history_modal.set_repository(path);
    }

    /// Render the model selection panel and return any selected model
    pub fn render_model_selection_panel(&mut self, ctx: &Context, theme: AppTheme) -> Option<String> {
        let mut selected_model = None;
        
        if self.model_selection_panel.visible {
            egui::Window::new("Model Selection")
                .resizable(true)
                .show(ctx, |ui| {
                    if self.model_selection_panel.render(ui, &theme) {
                        selected_model = Some(self.model_selection_panel.get_current_model().to_string());
                    }
                });
        }
        
        selected_model
    }

    pub fn show_preview(&mut self, title: &str, content: &str) {
        log::debug!("show_preview called with title: {title}");
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
                    self.preview_panel.visible = false;
                    self.active_panel = ActivePanel::None;
                } else {
                    self.close_all_panels();
                    self.preview_panel.visible = true;
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
            ActivePanel::Task => {
                // Task panel is handled by the main app
                if matches!(self.active_panel, ActivePanel::Task) {
                    self.active_panel = ActivePanel::None;
                } else {
                    self.close_all_panels();
                    self.active_panel = ActivePanel::Task;
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
            ActivePanel::GitHistory => {
                if matches!(self.active_panel, ActivePanel::GitHistory) {
                    self.git_history_modal.toggle(); // Close
                    self.active_panel = ActivePanel::None;
                } else {
                    self.close_all_panels();
                    self.git_history_modal.toggle(); // Open
                    self.active_panel = ActivePanel::GitHistory;
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
            ActivePanel::GitHistory => {
                if self.git_history_modal.visible {
                    self.git_history_modal.toggle(); // Close
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

impl Default for AnalyticsPanel {
    fn default() -> Self {
        Self::new()
    }
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
            .frame(egui::Frame::NONE
                .fill(theme.panel_background())
                .inner_margin(egui::Margin::same(10)))
            .show(ctx, |ui| {
                // Main scrollable area for the entire panel
                egui::ScrollArea::vertical()
                    .id_salt("analytics_main_scroll")
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
                            ProjectFilter::Specific(project_type) => format!("{project_type:?}"),
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
        ui.label(format!("Total Input Tokens: {total_input_tokens}"));
        ui.label(format!("Total Output Tokens: {total_output_tokens}"));
        ui.label(format!("Total Combined Tokens: {}", total_input_tokens + total_output_tokens));
        ui.label(format!("Estimated Total Cost: ${total_cost:.6}"));
        ui.label(RichText::new(format!("(Prices based on Gemini 1.5 Flash: Input ${GEMINI_1_5_FLASH_INPUT_COST_PER_MILLION_TOKENS}/1M, Output ${GEMINI_1_5_FLASH_OUTPUT_COST_PER_MILLION_TOKENS}/1M tokens)")).small().weak());
        
        ui.separator();

        let mut new_selected_id: Option<String> = None;
        let mut clear_selection = false;

        if let Some(selected_id_val) = &self.selected_conversation_id {
            let current_selected_id = selected_id_val.clone();

            // --- Detailed Conversation View ---
            ui.heading(format!("Details for Conversation: {current_selected_id}"));
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
                    egui::Grid::new(format!("details_{current_selected_id}")).striped(true).show(ui, |ui| {
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
                    bars.push(Bar::new(i as f64 + 0.5, summary.total_cost).width(0.95).name(conv_id[..conv_id.len().min(10)].to_string())); // Truncate name for legend
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
                    
                    if ui.add(Button::new(RichText::new(format!("{success_rate:.1}%"))
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
                    .map(|h| format!("{h}:00"))
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
                        ui.label(RichText::new(format!("${cost:.2}"))
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
                    ui.label(format!("{role}: "));
                    ui.label(RichText::new(format!("{tokens} tokens")).color(theme.accent_color()));
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
                    .name(format!("{project_type:?}")));
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
    fn render_success_tab(&mut self, ui: &mut egui::Ui, report: &AnalyticsReport, _theme: AppTheme) -> Option<AnalyticsAction> {
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
                ui.label(RichText::new(format!("{success_rate:.1}%")).size(20.0));
                
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
                    ui.label(format!("{project_type:?}:"));
                    let rate_percent = success_rate * 100.0;
                    let color = if rate_percent >= 80.0 {
                        Color32::from_rgb(0, 200, 0)
                    } else if rate_percent >= 60.0 {
                        Color32::from_rgb(255, 165, 0)
                    } else {
                        Color32::from_rgb(255, 100, 100)
                    };
                    ui.colored_label(color, format!("{rate_percent:.1}%"));
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
                                ui.label(format!("  • {mitigation}"));
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
                                ui.label(format!("  • {characteristic}"));
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
                                ui.label(format!("  → {state}"));
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
                                ui.label(format!("  • {step}"));
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
                        ui.colored_label(color, format!("Success: {success_rate:.1}%"));
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
                            ui.label(format!("  • {pattern}"));
                        }
                    }
                    
                    // Recommendations
                    if !insight.recommendations.is_empty() {
                        ui.label("Recommendations:");
                        for recommendation in &insight.recommendations {
                            ui.colored_label(theme.accent_color(), format!("  → {recommendation}"));
                        }
                    }
                });
            });
            
            ui.add_space(10.0);
        }
    }

    /// Render the trends tab
    fn render_trends_tab(&mut self, ui: &mut egui::Ui, report: &AnalyticsReport, _theme: AppTheme) {
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
                    ui.colored_label(success_color, format!("Success: {success_rate:.1}%"));
                });
                
                // Associated project types
                if !topic.project_types.is_empty() {
                    ui.label(format!("Projects: {}", 
                        topic.project_types.iter()
                            .map(|pt| format!("{pt:?}"))
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
                                ui.label(format!("  • {evidence}"));
                            }
                        }
                    });
                });
                
                ui.add_space(8.0);
            }
        }
    }
}

/// Model selection panel for choosing Claude models
pub struct ModelSelectionPanel {
    pub visible: bool,
    pub current_model: String,
}


impl Default for ModelSelectionPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelSelectionPanel {
    pub fn new() -> Self {
        Self {
            visible: false,
            current_model: "claude-sonnet-4-20250514".to_string(),
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn set_current_model(&mut self, model: String) {
        self.current_model = model;
    }

    pub fn get_current_model(&self) -> &str {
        &self.current_model
    }

    pub fn render(&mut self, ui: &mut egui::Ui, _theme: &crate::gui::theme::AppTheme) -> bool {
        let mut model_changed = false;
        
        if self.visible {
            ui.heading("Claude Model Selection");
            ui.separator();
            
            egui::ComboBox::from_label("Model")
                .selected_text(&self.current_model)
                .show_ui(ui, |ui| {
                    for model in crate::llm::claude_code::models::CLAUDE_CODE_MODELS {
                        if ui.selectable_value(&mut self.current_model, model.id.to_string(), model.name).clicked() {
                            model_changed = true;
                        }
                    }
                });
            
            ui.separator();
            if ui.button("Close").clicked() {
                self.visible = false;
            }
        }
        
        model_changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_extract_file_path_from_content() {
        let mut panel = PreviewPanel::new();
        
        // Test case with file path
        panel.content = "FILE: File Content\n\n**File:** /home/user/test.rs\n\n**Content:**\n```\nfn main() {}\n```".to_string();
        assert_eq!(panel.extract_file_path_from_content(), Some("/home/user/test.rs".to_string()));
        
        // Test case without file path
        panel.content = "Some other content without file path".to_string();
        assert_eq!(panel.extract_file_path_from_content(), None);
        
        // Test case with file path at different position
        panel.content = "Some header\n**File:** /path/to/file.ts\nMore content".to_string();
        assert_eq!(panel.extract_file_path_from_content(), Some("/path/to/file.ts".to_string()));
    }
    
    #[test]
    fn test_extract_file_content() {
        let mut panel = PreviewPanel::new();
        
        // Test case with code block
        panel.content = "FILE: File Content\n\n**File:** /test.rs\n\n**Content:**\n```\nfn main() {\n    println!(\"Hello\");\n}\n```\n".to_string();
        assert_eq!(
            panel.extract_file_content(),
            Some("fn main() {\n    println!(\"Hello\");\n}".to_string())
        );
        
        // Test case without code block
        panel.content = "No code block here".to_string();
        assert_eq!(panel.extract_file_content(), None);
        
        // Test case with empty code block
        panel.content = "```\n```".to_string();
        // Since there's no newline before the closing ```, rfind won't find "\n```"
        assert_eq!(panel.extract_file_content(), None);
    }
    
    #[test]
    fn test_file_extension_detection() {
        assert_eq!(get_file_extension("/path/to/file.rs"), Some("rs"));
        assert_eq!(get_file_extension("/path/to/file.ts"), Some("ts"));
        assert_eq!(get_file_extension("/path/to/file.tsx"), Some("tsx"));
        assert_eq!(get_file_extension("/path/to/file.json"), Some("json"));
        assert_eq!(get_file_extension("/path/to/file.md"), Some("md"));
        assert_eq!(get_file_extension("/path/to/file"), None);
        assert_eq!(get_file_extension("/path/to/.gitignore"), None); // .gitignore has no extension
    }
    
    #[test]
    fn test_syntax_highlighting_support() {
        let syntax_set = get_syntax_set();
        
        // Test that we have syntax support for common languages
        assert!(syntax_set.find_syntax_by_extension("rs").is_some(), "Rust syntax should be supported");
        assert!(syntax_set.find_syntax_by_extension("js").is_some(), "JavaScript syntax should be supported");
        assert!(syntax_set.find_syntax_by_extension("json").is_some(), "JSON syntax should be supported");
        assert!(syntax_set.find_syntax_by_extension("md").is_some(), "Markdown syntax should be supported");
        
        // TypeScript might not be directly supported, but should fall back to JavaScript
        let ts_syntax = syntax_set.find_syntax_by_extension("ts")
            .or_else(|| syntax_set.find_syntax_by_extension("js"));
        assert!(ts_syntax.is_some(), "TypeScript should fall back to JavaScript syntax");
    }
}
