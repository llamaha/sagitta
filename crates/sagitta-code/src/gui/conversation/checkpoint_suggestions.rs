// Smart checkpoint suggestions UI component

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use egui::{Align, Color32, Frame, Layout, RichText, ScrollArea, Stroke, Ui};

use crate::agent::conversation::checkpoints::{CheckpointSuggestion, CheckpointReason};
use crate::gui::theme::AppTheme;

/// Configuration for checkpoint suggestions UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointSuggestionsConfig {
    /// Show confidence indicators
    pub show_confidence: bool,
    
    /// Minimum confidence threshold to display
    pub min_confidence_threshold: f32,
    
    /// Maximum number of suggestions to show
    pub max_suggestions: usize,
    
    /// Auto-refresh interval in seconds
    pub auto_refresh_interval: u64,
    
    /// Show suggestion reasons
    pub show_reasons: bool,
    
    /// Enable suggestion dismissal
    pub allow_dismiss: bool,
}

impl Default for CheckpointSuggestionsConfig {
    fn default() -> Self {
        Self {
            show_confidence: true,
            min_confidence_threshold: 0.5,
            max_suggestions: 5,
            auto_refresh_interval: 30,
            show_reasons: true,
            allow_dismiss: true,
        }
    }
}

/// Actions that can be taken on checkpoint suggestions
#[derive(Debug, Clone)]
pub enum CheckpointSuggestionAction {
    /// Create a checkpoint from a suggestion
    CreateCheckpoint {
        conversation_id: Uuid,
        suggestion: CheckpointSuggestion,
    },
    
    /// Dismiss a checkpoint suggestion
    DismissSuggestion {
        conversation_id: Uuid,
        message_id: Uuid,
    },
    
    /// Show details for a checkpoint suggestion
    ShowDetails {
        suggestion: CheckpointSuggestion,
    },
    
    /// Refresh checkpoint suggestions
    RefreshSuggestions {
        conversation_id: Uuid,
    },
    
    /// Jump to the message associated with a suggestion
    JumpToMessage {
        conversation_id: Uuid,
        message_id: Uuid,
    },
}

/// Checkpoint suggestions UI component
#[derive(Clone)]
pub struct CheckpointSuggestionsUI {
    /// Configuration
    config: CheckpointSuggestionsConfig,
    
    /// Current suggestions
    suggestions: Vec<CheckpointSuggestion>,
    
    /// Dismissed suggestions (by message ID)
    dismissed_suggestions: std::collections::HashSet<Uuid>,
    
    /// Last refresh time
    last_refresh: Option<DateTime<Utc>>,
    
    /// Show suggestions panel
    visible: bool,
    
    /// Expanded suggestion details
    expanded_details: std::collections::HashSet<Uuid>,
}

impl CheckpointSuggestionsUI {
    /// Create a new checkpoint suggestions UI
    pub fn new(config: CheckpointSuggestionsConfig) -> Self {
        Self {
            config,
            suggestions: Vec::new(),
            dismissed_suggestions: std::collections::HashSet::new(),
            last_refresh: None,
            visible: false,
            expanded_details: std::collections::HashSet::new(),
        }
    }
    
    /// Create with default configuration
    pub fn with_default_config() -> Self {
        Self::new(CheckpointSuggestionsConfig::default())
    }
    
    /// Update suggestions
    pub fn update_suggestions(&mut self, suggestions: Vec<CheckpointSuggestion>) {
        self.suggestions = suggestions;
        self.last_refresh = Some(Utc::now());
    }
    
    /// Dismiss a suggestion
    pub fn dismiss_suggestion(&mut self, message_id: Uuid) {
        self.dismissed_suggestions.insert(message_id);
    }
    
    /// Clear dismissed suggestions
    pub fn clear_dismissed(&mut self) {
        self.dismissed_suggestions.clear();
    }
    
    /// Set visibility
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
    
    /// Check if visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }
    
    /// Toggle visibility
    pub fn toggle_visibility(&mut self) {
        self.visible = !self.visible;
    }
    
    /// Get filtered suggestions (above threshold, not dismissed)
    fn get_filtered_suggestions(&self) -> Vec<&CheckpointSuggestion> {
        self.suggestions
            .iter()
            .filter(|s| {
                s.importance >= self.config.min_confidence_threshold
                    && !self.dismissed_suggestions.contains(&s.message_id)
            })
            .take(self.config.max_suggestions)
            .collect()
    }
    
    /// Render the checkpoint suggestions UI
    pub fn render(
        &mut self,
        ui: &mut Ui,
        conversation_id: Uuid,
        theme: &AppTheme,
    ) -> Result<Option<CheckpointSuggestionAction>> {
        if !self.visible {
            return Ok(None);
        }
        
        let mut action = None;
        
        // Header
        ui.horizontal(|ui| {
            ui.label(RichText::new("📍 Checkpoint Suggestions").strong());
            
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.small_button("🔄").on_hover_text("Refresh suggestions").clicked() {
                    action = Some(CheckpointSuggestionAction::RefreshSuggestions { conversation_id });
                }
                
                if ui.small_button("✖").on_hover_text("Hide suggestions").clicked() {
                    self.visible = false;
                }
            });
        });
        
        ui.separator();
        
        let filtered_suggestions = self.get_filtered_suggestions();
        let filtered_count = filtered_suggestions.len();
        let total_count = self.suggestions.len();
        
        if filtered_count == 0 {
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new("No checkpoint suggestions available").weak());
            });
            return Ok(action);
        }
        
        // Clone suggestions to avoid borrowing issues
        let suggestions_to_render: Vec<CheckpointSuggestion> = filtered_suggestions.into_iter().cloned().collect();
        
        // Suggestions list
        ScrollArea::vertical()
            .max_height(200.0)
            .show(ui, |ui| {
                for suggestion in &suggestions_to_render {
                    if let Some(suggestion_action) = self.render_suggestion(ui, conversation_id, suggestion, theme)? {
                        action = Some(suggestion_action);
                    }
                }
                Ok::<(), anyhow::Error>(())
            })
            .inner?;
        
        // Statistics
        if total_count > filtered_count {
            ui.add_space(4.0);
            ui.label(RichText::new(format!(
                "Showing {} of {} suggestions",
                filtered_count,
                total_count
            )).small().weak());
        }
        
        Ok(action)
    }
    
    /// Render a single checkpoint suggestion
    fn render_suggestion(
        &mut self,
        ui: &mut Ui,
        conversation_id: Uuid,
        suggestion: &CheckpointSuggestion,
        theme: &AppTheme,
    ) -> Result<Option<CheckpointSuggestionAction>> {
        let mut action = None;
        
        let frame_color = self.get_reason_color(&suggestion.reason, theme);
        
        Frame::none()
            .fill(theme.panel_background())
            .stroke(Stroke::new(1.0, frame_color))
            .rounding(4.0)
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Reason icon
                    let (icon, icon_color) = self.get_reason_icon_and_color(&suggestion.reason, theme);
                    ui.label(RichText::new(icon).color(icon_color).size(16.0));
                    
                    ui.vertical(|ui| {
                        // Title
                        ui.label(RichText::new(&suggestion.suggested_title).strong());
                        
                        // Confidence and reason
                        ui.horizontal(|ui| {
                            if self.config.show_confidence {
                                let confidence_color = self.get_confidence_color(suggestion.importance, theme);
                                ui.label(RichText::new(format!("{}%", (suggestion.importance * 100.0) as u8))
                                    .color(confidence_color)
                                    .small());
                            }
                            
                            if self.config.show_reasons {
                                ui.label(RichText::new(format!("• {}", self.format_reason(&suggestion.reason)))
                                    .small()
                                    .weak());
                            }
                        });
                    });
                    
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // Action buttons
                        if ui.small_button("📍").on_hover_text("Create checkpoint").clicked() {
                            action = Some(CheckpointSuggestionAction::CreateCheckpoint {
                                conversation_id,
                                suggestion: suggestion.clone(),
                            });
                        }
                        
                        if ui.small_button("👁").on_hover_text("Jump to message").clicked() {
                            action = Some(CheckpointSuggestionAction::JumpToMessage {
                                conversation_id,
                                message_id: suggestion.message_id,
                            });
                        }
                        
                        if self.config.allow_dismiss {
                            if ui.small_button("✖").on_hover_text("Dismiss suggestion").clicked() {
                                action = Some(CheckpointSuggestionAction::DismissSuggestion {
                                    conversation_id,
                                    message_id: suggestion.message_id,
                                });
                            }
                        }
                        
                        // Details toggle
                        let details_expanded = self.expanded_details.contains(&suggestion.message_id);
                        let details_icon = if details_expanded { "▼" } else { "▶" };
                        if ui.small_button(details_icon).on_hover_text("Toggle details").clicked() {
                            if details_expanded {
                                self.expanded_details.remove(&suggestion.message_id);
                            } else {
                                self.expanded_details.insert(suggestion.message_id);
                            }
                        }
                    });
                });
                
                // Expanded details
                if self.expanded_details.contains(&suggestion.message_id) {
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Details:").small().strong());
                        
                        ui.label(RichText::new(format!("Restoration Value: {:.1}%", suggestion.restoration_value * 100.0)).small());
                        
                        if !suggestion.context.trigger_keywords.is_empty() {
                            ui.label(RichText::new(format!("Keywords: {}", suggestion.context.trigger_keywords.join(", "))).small());
                        }
                        
                        if !suggestion.context.executed_tools.is_empty() {
                            ui.label(RichText::new(format!("Tools: {}", suggestion.context.executed_tools.join(", "))).small());
                        }
                        
                        ui.label(RichText::new(format!("Phase: {:?}", suggestion.context.conversation_phase)).small());
                    });
                }
            });
        
        ui.add_space(4.0);
        
        Ok(action)
    }
    
    /// Get color for checkpoint reason
    fn get_reason_color(&self, reason: &CheckpointReason, theme: &AppTheme) -> Color32 {
        match reason {
            CheckpointReason::MajorMilestone => theme.success_color(),
            CheckpointReason::SuccessfulSolution => theme.success_color(),
            CheckpointReason::BeforeRiskyOperation => theme.warning_color(),
            CheckpointReason::ContextChange => theme.accent_color(),
            CheckpointReason::BeforeRefactoring => theme.warning_color(),
            CheckpointReason::TaskCompletion => theme.success_color(),
            CheckpointReason::UserRequested => theme.accent_color(),
            CheckpointReason::PeriodicAutomatic => theme.hint_text_color(),
            CheckpointReason::BeforeBranching => theme.accent_color(),
        }
    }
    
    /// Get icon and color for checkpoint reason
    fn get_reason_icon_and_color(&self, reason: &CheckpointReason, theme: &AppTheme) -> (&str, Color32) {
        match reason {
            CheckpointReason::MajorMilestone => ("🏆", theme.success_color()),
            CheckpointReason::SuccessfulSolution => ("✅", theme.success_color()),
            CheckpointReason::BeforeRiskyOperation => ("⚠️", theme.warning_color()),
            CheckpointReason::ContextChange => ("🔄", theme.accent_color()),
            CheckpointReason::BeforeRefactoring => ("🔧", theme.warning_color()),
            CheckpointReason::TaskCompletion => ("🎯", theme.success_color()),
            CheckpointReason::UserRequested => ("👤", theme.accent_color()),
            CheckpointReason::PeriodicAutomatic => ("🤖", theme.hint_text_color()),
            CheckpointReason::BeforeBranching => ("🌳", theme.accent_color()),
        }
    }
    
    /// Format reason for display
    fn format_reason(&self, reason: &CheckpointReason) -> String {
        match reason {
            CheckpointReason::MajorMilestone => "Major Milestone".to_string(),
            CheckpointReason::SuccessfulSolution => "Successful Solution".to_string(),
            CheckpointReason::BeforeRiskyOperation => "Before Risky Operation".to_string(),
            CheckpointReason::ContextChange => "Context Change".to_string(),
            CheckpointReason::BeforeRefactoring => "Before Refactoring".to_string(),
            CheckpointReason::TaskCompletion => "Task Completion".to_string(),
            CheckpointReason::UserRequested => "User Requested".to_string(),
            CheckpointReason::PeriodicAutomatic => "Automatic".to_string(),
            CheckpointReason::BeforeBranching => "Before Branching".to_string(),
        }
    }
    
    /// Get confidence color
    fn get_confidence_color(&self, confidence: f32, theme: &AppTheme) -> Color32 {
        if confidence >= 0.8 {
            theme.success_color()
        } else if confidence >= 0.6 {
            theme.warning_color()
        } else {
            theme.error_color()
        }
    }
    
    /// Get configuration
    pub fn get_config(&self) -> &CheckpointSuggestionsConfig {
        &self.config
    }
    
    /// Update configuration
    pub fn update_config(&mut self, config: CheckpointSuggestionsConfig) {
        self.config = config;
    }
    
    /// Get suggestion count
    pub fn suggestion_count(&self) -> usize {
        self.get_filtered_suggestions().len()
    }
    
    /// Check if auto-refresh is needed
    pub fn needs_refresh(&self) -> bool {
        if let Some(last_refresh) = self.last_refresh {
            let elapsed = Utc::now().signed_duration_since(last_refresh);
            elapsed.num_seconds() >= self.config.auto_refresh_interval as i64
        } else {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::conversation::checkpoints::{CheckpointContext, ConversationPhase};
    use std::path::PathBuf;

    fn create_test_suggestion(reason: CheckpointReason, importance: f32) -> CheckpointSuggestion {
        CheckpointSuggestion {
            message_id: Uuid::new_v4(),
            importance,
            reason,
            suggested_title: "Test Checkpoint".to_string(),
            context: CheckpointContext {
                relevant_messages: vec![],
                trigger_keywords: vec!["test".to_string()],
                conversation_phase: ConversationPhase::Implementation,
                modified_files: vec![PathBuf::from("test.rs")],
                executed_tools: vec!["cargo".to_string()],
                success_indicators: vec!["working".to_string()],
            },
            restoration_value: 0.8,
        }
    }

    #[test]
    fn test_checkpoint_suggestions_ui_creation() {
        let ui = CheckpointSuggestionsUI::with_default_config();
        assert!(!ui.is_visible());
        assert_eq!(ui.suggestion_count(), 0);
    }

    #[test]
    fn test_suggestion_filtering() {
        let mut ui = CheckpointSuggestionsUI::with_default_config();
        
        let suggestions = vec![
            create_test_suggestion(CheckpointReason::SuccessfulSolution, 0.9),
            create_test_suggestion(CheckpointReason::BeforeRiskyOperation, 0.3), // Below threshold
            create_test_suggestion(CheckpointReason::MajorMilestone, 0.8),
        ];
        
        ui.update_suggestions(suggestions);
        
        // Should filter out the low-confidence suggestion
        assert_eq!(ui.suggestion_count(), 2);
    }

    #[test]
    fn test_suggestion_dismissal() {
        let mut ui = CheckpointSuggestionsUI::with_default_config();
        
        let suggestion = create_test_suggestion(CheckpointReason::SuccessfulSolution, 0.9);
        let message_id = suggestion.message_id;
        
        ui.update_suggestions(vec![suggestion]);
        assert_eq!(ui.suggestion_count(), 1);
        
        ui.dismiss_suggestion(message_id);
        assert_eq!(ui.suggestion_count(), 0);
    }

    #[test]
    fn test_visibility_toggle() {
        let mut ui = CheckpointSuggestionsUI::with_default_config();
        
        assert!(!ui.is_visible());
        ui.toggle_visibility();
        assert!(ui.is_visible());
        ui.toggle_visibility();
        assert!(!ui.is_visible());
    }

    #[test]
    fn test_reason_formatting() {
        let ui = CheckpointSuggestionsUI::with_default_config();
        
        assert_eq!(ui.format_reason(&CheckpointReason::SuccessfulSolution), "Successful Solution");
        assert_eq!(ui.format_reason(&CheckpointReason::BeforeRiskyOperation), "Before Risky Operation");
        assert_eq!(ui.format_reason(&CheckpointReason::MajorMilestone), "Major Milestone");
    }

    #[test]
    fn test_configuration_update() {
        let mut ui = CheckpointSuggestionsUI::with_default_config();
        
        let mut config = CheckpointSuggestionsConfig::default();
        config.min_confidence_threshold = 0.8;
        config.max_suggestions = 3;
        
        ui.update_config(config.clone());
        
        assert_eq!(ui.get_config().min_confidence_threshold, 0.8);
        assert_eq!(ui.get_config().max_suggestions, 3);
    }

    #[test]
    fn test_auto_refresh_timing() {
        let mut ui = CheckpointSuggestionsUI::with_default_config();
        
        // Should need refresh initially
        assert!(ui.needs_refresh());
        
        // After updating suggestions, should not need refresh immediately
        ui.update_suggestions(vec![]);
        assert!(!ui.needs_refresh());
    }

    #[test]
    fn test_expanded_details_management() {
        let mut ui = CheckpointSuggestionsUI::with_default_config();
        let message_id = Uuid::new_v4();
        
        assert!(!ui.expanded_details.contains(&message_id));
        
        ui.expanded_details.insert(message_id);
        assert!(ui.expanded_details.contains(&message_id));
        
        ui.expanded_details.remove(&message_id);
        assert!(!ui.expanded_details.contains(&message_id));
    }
} 