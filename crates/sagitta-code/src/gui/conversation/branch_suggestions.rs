use anyhow::Result;
use egui::{Align, Color32, Layout, RichText, Ui, Vec2, Stroke, Rect, Pos2, Shape};
use uuid::Uuid;

use crate::agent::conversation::branching::{BranchSuggestion, BranchReason, ConversationState};
use crate::agent::conversation::types::Conversation;
use crate::gui::theme::AppTheme;

/// Action that can be triggered from the branch suggestions UI
#[derive(Debug, Clone)]
pub enum BranchSuggestionAction {
    /// Create a new branch from a suggestion
    CreateBranch {
        conversation_id: Uuid,
        suggestion: BranchSuggestion,
    },
    /// Dismiss a suggestion
    DismissSuggestion {
        conversation_id: Uuid,
        message_id: Uuid,
    },
    /// Show branch details
    ShowDetails {
        suggestion: BranchSuggestion,
    },
    /// Refresh suggestions
    RefreshSuggestions {
        conversation_id: Uuid,
    },
}

/// UI component for displaying branch suggestions
#[derive(Clone)]
pub struct BranchSuggestionsUI {
    /// Currently visible suggestions
    suggestions: Vec<BranchSuggestion>,
    
    /// Dismissed suggestions (to avoid showing them again)
    dismissed_suggestions: std::collections::HashSet<Uuid>,
    
    /// Whether to show suggestion details
    show_details: bool,
    
    /// Currently selected suggestion for details
    selected_suggestion: Option<BranchSuggestion>,
    
    /// Configuration for the UI
    config: BranchSuggestionsConfig,
}

/// Configuration for branch suggestions UI
#[derive(Debug, Clone)]
pub struct BranchSuggestionsConfig {
    /// Minimum confidence to show suggestions
    pub min_confidence: f32,
    
    /// Maximum number of suggestions to show
    pub max_suggestions: usize,
    
    /// Whether to show confidence scores
    pub show_confidence: bool,
    
    /// Whether to show success predictions
    pub show_success_prediction: bool,
    
    /// Whether to auto-dismiss low confidence suggestions
    pub auto_dismiss_low_confidence: bool,
}

impl Default for BranchSuggestionsConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.6,
            max_suggestions: 3,
            show_confidence: true,
            show_success_prediction: true,
            auto_dismiss_low_confidence: false,
        }
    }
}

impl BranchSuggestionsUI {
    /// Create a new branch suggestions UI
    pub fn new() -> Self {
        Self {
            suggestions: Vec::new(),
            dismissed_suggestions: std::collections::HashSet::new(),
            show_details: false,
            selected_suggestion: None,
            config: BranchSuggestionsConfig::default(),
        }
    }
    
    /// Create with custom configuration
    pub fn with_config(config: BranchSuggestionsConfig) -> Self {
        Self {
            suggestions: Vec::new(),
            dismissed_suggestions: std::collections::HashSet::new(),
            show_details: false,
            selected_suggestion: None,
            config,
        }
    }
    
    /// Update suggestions for a conversation
    pub fn update_suggestions(&mut self, suggestions: Vec<BranchSuggestion>) {
        // Filter suggestions based on configuration
        self.suggestions = suggestions
            .into_iter()
            .filter(|s| {
                s.confidence >= self.config.min_confidence &&
                !self.dismissed_suggestions.contains(&s.message_id)
            })
            .take(self.config.max_suggestions)
            .collect();
        
        // Sort by confidence (highest first)
        self.suggestions.sort_by(|a, b| {
            b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal)
        });
    }
    
    /// Dismiss a suggestion
    pub fn dismiss_suggestion(&mut self, message_id: Uuid) {
        self.dismissed_suggestions.insert(message_id);
        self.suggestions.retain(|s| s.message_id != message_id);
    }
    
    /// Clear all dismissed suggestions
    pub fn clear_dismissed(&mut self) {
        self.dismissed_suggestions.clear();
    }
    
    /// Check if there are any suggestions to show
    pub fn has_suggestions(&self) -> bool {
        !self.suggestions.is_empty()
    }
    
    /// Render the branch suggestions UI
    pub fn render(
        &mut self,
        ui: &mut Ui,
        conversation_id: Uuid,
        theme: &AppTheme,
    ) -> Result<Option<BranchSuggestionAction>> {
        if self.suggestions.is_empty() {
            return Ok(None);
        }
        
        let mut action = None;
        
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("🌳 Branch Suggestions").strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.small_button("🔄").on_hover_text("Refresh suggestions").clicked() {
                        action = Some(BranchSuggestionAction::RefreshSuggestions { conversation_id });
                    }
                    if ui.small_button("ℹ").on_hover_text("Show details").clicked() {
                        self.show_details = !self.show_details;
                    }
                });
            });
            
            ui.separator();
            
            // Clone suggestions to avoid borrow checker issues
            let suggestions_clone = self.suggestions.clone();
            for suggestion in &suggestions_clone {
                if let Ok(Some(suggestion_action)) = self.render_suggestion(ui, conversation_id, suggestion, theme) {
                    action = Some(suggestion_action);
                }
            }
            
            if self.show_details {
                if let Some(ref selected) = self.selected_suggestion {
                    let _ = self.render_suggestion_details(ui, selected, theme);
                }
            }
        });
        
        Ok(action)
    }
    
    /// Render a single suggestion
    fn render_suggestion(
        &mut self,
        ui: &mut Ui,
        conversation_id: Uuid,
        suggestion: &BranchSuggestion,
        theme: &AppTheme,
    ) -> Result<Option<BranchSuggestionAction>> {
        let mut action = None;
        
        ui.horizontal(|ui| {
            // Branch reason icon
            let (icon, color) = self.get_reason_icon_and_color(&suggestion.reason, theme);
            ui.colored_label(color, icon);
            
            // Suggestion title
            ui.label(&suggestion.suggested_title);
            
            // Confidence indicator
            if self.config.show_confidence {
                let confidence_color = self.get_confidence_color(suggestion.confidence, theme);
                ui.colored_label(
                    confidence_color,
                    format!("{:.0}%", suggestion.confidence * 100.0)
                );
            }
            
            // Success prediction
            if self.config.show_success_prediction {
                if let Some(success_prob) = suggestion.success_probability {
                    let success_color = self.get_success_color(success_prob, theme);
                    ui.colored_label(
                        success_color,
                        format!("📈 {:.0}%", success_prob * 100.0)
                    );
                }
            }
            
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // Dismiss button
                if ui.small_button("✕").on_hover_text("Dismiss suggestion").clicked() {
                    action = Some(BranchSuggestionAction::DismissSuggestion {
                        conversation_id,
                        message_id: suggestion.message_id,
                    });
                }
                
                // Details button
                if ui.small_button("ℹ").on_hover_text("Show details").clicked() {
                    self.selected_suggestion = Some(suggestion.clone());
                    action = Some(BranchSuggestionAction::ShowDetails {
                        suggestion: suggestion.clone(),
                    });
                }
                
                // Create branch button
                if ui.button("🌿 Branch").on_hover_text("Create branch").clicked() {
                    action = Some(BranchSuggestionAction::CreateBranch {
                        conversation_id,
                        suggestion: suggestion.clone(),
                    });
                }
            });
        });
        
        // Show reason description
        ui.indent("suggestion_reason", |ui| {
            ui.label(RichText::new(self.get_reason_description(&suggestion.reason)).small().weak());
        });
        
        ui.add_space(4.0);
        
        Ok(action)
    }
    
    /// Render detailed information about a suggestion
    fn render_suggestion_details(
        &self,
        ui: &mut Ui,
        suggestion: &BranchSuggestion,
        theme: &AppTheme,
    ) -> Result<()> {
        ui.collapsing("Suggestion Details", |ui| {
            ui.label(format!("Message ID: {}", suggestion.message_id));
            ui.label(format!("Confidence: {:.2}", suggestion.confidence));
            ui.label(format!("Reason: {:?}", suggestion.reason));
            
            if let Some(success_prob) = suggestion.success_probability {
                ui.label(format!("Success Probability: {:.2}", success_prob));
            }
            
            ui.label(format!("Conversation State: {:?}", suggestion.context.conversation_state));
            
            if !suggestion.context.trigger_keywords.is_empty() {
                ui.label("Trigger Keywords:");
                ui.indent("keywords", |ui| {
                    for keyword in &suggestion.context.trigger_keywords {
                        ui.label(format!("• {}", keyword));
                    }
                });
            }
            
            if !suggestion.context.mentioned_tools.is_empty() {
                ui.label("Mentioned Tools:");
                ui.indent("tools", |ui| {
                    for tool in &suggestion.context.mentioned_tools {
                        ui.label(format!("• {}", tool));
                    }
                });
            }
            
            if let Some(ref project) = suggestion.context.project_context {
                ui.label(format!("Project Context: {}", project));
            }
        });
        
        Ok(())
    }
    
    /// Get icon and color for a branch reason
    fn get_reason_icon_and_color(&self, reason: &BranchReason, theme: &AppTheme) -> (&'static str, Color32) {
        match reason {
            BranchReason::MultipleSolutions => ("🔀", theme.accent_color()),
            BranchReason::ErrorRecovery => ("🔧", Color32::from_rgb(255, 165, 0)), // Orange
            BranchReason::UserUncertainty => ("❓", Color32::from_rgb(255, 255, 0)), // Yellow
            BranchReason::ComplexProblem => ("🧩", Color32::from_rgb(128, 0, 128)), // Purple
            BranchReason::AlternativeApproach => ("🔄", Color32::from_rgb(0, 191, 255)), // Deep sky blue
            BranchReason::ExperimentalApproach => ("🧪", Color32::from_rgb(255, 20, 147)), // Deep pink
            BranchReason::UserRequested => ("👤", Color32::from_rgb(0, 255, 0)), // Lime
        }
    }
    
    /// Get description for a branch reason
    fn get_reason_description(&self, reason: &BranchReason) -> &'static str {
        match reason {
            BranchReason::MultipleSolutions => "Multiple solution approaches detected",
            BranchReason::ErrorRecovery => "Error detected, alternative approach needed",
            BranchReason::UserUncertainty => "User expressed uncertainty or asked for alternatives",
            BranchReason::ComplexProblem => "Complex problem that could benefit from parallel exploration",
            BranchReason::AlternativeApproach => "Different tool or approach could be more effective",
            BranchReason::ExperimentalApproach => "Experimental or risky approach suggested",
            BranchReason::UserRequested => "User explicitly requested branching",
        }
    }
    
    /// Get color for confidence level
    fn get_confidence_color(&self, confidence: f32, theme: &AppTheme) -> Color32 {
        if confidence >= 0.8 {
            Color32::from_rgb(0, 255, 0) // Green
        } else if confidence >= 0.6 {
            Color32::from_rgb(255, 255, 0) // Yellow
        } else {
            Color32::from_rgb(255, 165, 0) // Orange
        }
    }
    
    /// Get color for success probability
    fn get_success_color(&self, success_prob: f32, theme: &AppTheme) -> Color32 {
        if success_prob >= 0.7 {
            Color32::from_rgb(0, 255, 0) // Green
        } else if success_prob >= 0.5 {
            Color32::from_rgb(255, 255, 0) // Yellow
        } else {
            Color32::from_rgb(255, 0, 0) // Red
        }
    }
    
    /// Render a branch suggestion badge for a message
    pub fn render_branch_badge(
        ui: &mut Ui,
        suggestion: &BranchSuggestion,
        theme: &AppTheme,
    ) -> bool {
        let (icon, color) = match suggestion.reason {
            BranchReason::MultipleSolutions => ("🔀", theme.accent_color()),
            BranchReason::ErrorRecovery => ("🔧", Color32::from_rgb(255, 165, 0)),
            BranchReason::UserUncertainty => ("❓", Color32::from_rgb(255, 255, 0)),
            BranchReason::ComplexProblem => ("🧩", Color32::from_rgb(128, 0, 128)),
            BranchReason::AlternativeApproach => ("🔄", Color32::from_rgb(0, 191, 255)),
            BranchReason::ExperimentalApproach => ("🧪", Color32::from_rgb(255, 20, 147)),
            BranchReason::UserRequested => ("👤", Color32::from_rgb(0, 255, 0)),
        };
        
        // Create a small badge with the branch icon
        let response = ui.add_sized(
            Vec2::new(20.0, 20.0),
            egui::Button::new(RichText::new("🌳").small())
                .fill(color.gamma_multiply(0.3))
                .stroke(Stroke::new(1.0, color))
        );
        
        let clicked = response.clicked();
        
        if response.hovered() {
            response.on_hover_text(format!(
                "Branch suggestion: {}\nConfidence: {:.0}%\nClick to create branch",
                suggestion.suggested_title,
                suggestion.confidence * 100.0
            ));
        }
        
        clicked
    }
    
    /// Get configuration
    pub fn get_config(&self) -> &BranchSuggestionsConfig {
        &self.config
    }
    
    /// Update configuration
    pub fn update_config(&mut self, config: BranchSuggestionsConfig) {
        self.config = config;
    }
    
    /// Get current suggestions
    pub fn get_suggestions(&self) -> &[BranchSuggestion] {
        &self.suggestions
    }
}

impl Default for BranchSuggestionsUI {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::conversation::branching::{BranchContext, ConversationState};
    use chrono::Utc;
    
    fn create_test_suggestion() -> BranchSuggestion {
        BranchSuggestion {
            message_id: Uuid::new_v4(),
            confidence: 0.8,
            reason: BranchReason::AlternativeApproach,
            suggested_title: "Alternative Solution".to_string(),
            success_probability: Some(0.7),
            context: BranchContext {
                relevant_messages: vec![],
                trigger_keywords: vec!["alternative".to_string()],
                conversation_state: ConversationState::SolutionDevelopment,
                project_context: None,
                mentioned_tools: vec!["git".to_string()],
            },
        }
    }
    
    #[test]
    fn test_branch_suggestions_ui_creation() {
        let ui = BranchSuggestionsUI::new();
        assert!(!ui.has_suggestions());
        assert_eq!(ui.get_config().min_confidence, 0.6);
    }
    
    #[test]
    fn test_update_suggestions() {
        let mut ui = BranchSuggestionsUI::new();
        let suggestions = vec![create_test_suggestion()];
        
        ui.update_suggestions(suggestions);
        assert!(ui.has_suggestions());
        assert_eq!(ui.get_suggestions().len(), 1);
    }
    
    #[test]
    fn test_dismiss_suggestion() {
        let mut ui = BranchSuggestionsUI::new();
        let suggestion = create_test_suggestion();
        let message_id = suggestion.message_id;
        
        ui.update_suggestions(vec![suggestion]);
        assert!(ui.has_suggestions());
        
        ui.dismiss_suggestion(message_id);
        assert!(!ui.has_suggestions());
    }
    
    #[test]
    fn test_confidence_filtering() {
        let mut ui = BranchSuggestionsUI::with_config(BranchSuggestionsConfig {
            min_confidence: 0.9,
            ..Default::default()
        });
        
        let low_confidence_suggestion = BranchSuggestion {
            confidence: 0.5,
            ..create_test_suggestion()
        };
        
        ui.update_suggestions(vec![low_confidence_suggestion]);
        assert!(!ui.has_suggestions()); // Should be filtered out
    }
    
    #[test]
    fn test_max_suggestions_limit() {
        let mut ui = BranchSuggestionsUI::with_config(BranchSuggestionsConfig {
            max_suggestions: 2,
            ..Default::default()
        });
        
        let suggestions = vec![
            create_test_suggestion(),
            create_test_suggestion(),
            create_test_suggestion(),
        ];
        
        ui.update_suggestions(suggestions);
        assert_eq!(ui.get_suggestions().len(), 2); // Should be limited to 2
    }
    
    #[test]
    fn test_reason_icon_and_color() {
        let ui = BranchSuggestionsUI::new();
        let theme = AppTheme::default();
        
        let (icon, _) = ui.get_reason_icon_and_color(&BranchReason::ErrorRecovery, &theme);
        assert_eq!(icon, "🔧");
        
        let (icon, _) = ui.get_reason_icon_and_color(&BranchReason::UserUncertainty, &theme);
        assert_eq!(icon, "❓");
    }
    
    #[test]
    fn test_reason_description() {
        let ui = BranchSuggestionsUI::new();
        
        let desc = ui.get_reason_description(&BranchReason::ErrorRecovery);
        assert!(desc.contains("Error detected"));
        
        let desc = ui.get_reason_description(&BranchReason::ComplexProblem);
        assert!(desc.contains("Complex problem"));
    }
    
    #[test]
    fn test_confidence_color() {
        let ui = BranchSuggestionsUI::new();
        let theme = AppTheme::default();
        
        let high_confidence_color = ui.get_confidence_color(0.9, &theme);
        let medium_confidence_color = ui.get_confidence_color(0.7, &theme);
        let low_confidence_color = ui.get_confidence_color(0.5, &theme);
        
        // Colors should be different for different confidence levels
        assert_ne!(high_confidence_color, medium_confidence_color);
        assert_ne!(medium_confidence_color, low_confidence_color);
    }
} 