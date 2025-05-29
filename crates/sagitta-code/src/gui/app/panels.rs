// Panel management for the Sagitta Code application

use std::sync::Arc;
use egui::{Context, ScrollArea, Window, RichText, Color32};
use egui_plot::{Line, Plot, Bar, BarChart, Legend, PlotPoints};
use super::super::theme::AppTheme;
use super::super::theme_customizer::ThemeCustomizer;

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
        }
    }

    pub fn show_preview(&mut self, title: &str, content: &str) {
        self.preview_panel.set_content(title, content);
        
        // Automatically open the preview panel if it's not already open
        if !self.preview_panel.visible {
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
}

#[derive(Debug, Clone, PartialEq)]
pub enum TimeFilter {
    LastHour,
    Last24Hours,
    Last7Days,
    AllTime,
}

impl AnalyticsPanel {
    pub fn new() -> Self {
        Self {
            visible: false,
            all_usage_entries: Vec::new(),
            conversation_summaries: std::collections::HashMap::new(),
            time_filter: TimeFilter::AllTime,
            selected_conversation_id: None,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
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

    pub fn render(&mut self, ctx: &egui::Context, theme: AppTheme) {
        if !self.visible {
            return;
        }

        Window::new("📊 Analytics Panel")
            .default_width(600.0)
            .default_height(400.0)
            .resizable(true)
            .frame(egui::Frame::none().fill(theme.panel_background()))
            .show(ctx, |ui| {
                ui.vertical_centered_justified(|ui| {
                     ui.heading("Token Usage & Cost Analytics");
                });
                ui.separator();

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

            });
    }
} 