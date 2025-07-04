use super::{TagSuggestion, TagAction, TagSuggestionWithAction, TagSource, SuggestionConfidence};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::Utc;

/// State for the tag management UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagUIState {
    /// Current suggestions being displayed
    pub suggestions: Vec<TagSuggestionWithAction>,
    /// Whether the tag panel is expanded
    pub expanded: bool,
    /// Filter for suggestion confidence
    pub confidence_filter: Option<SuggestionConfidence>,
    /// Whether to show only pending suggestions
    pub show_only_pending: bool,
    /// Whether to auto-apply high confidence suggestions
    pub auto_apply_enabled: bool,
    /// Custom tag being typed by user
    pub custom_tag_input: String,
    /// Whether the custom tag input is visible
    pub show_custom_input: bool,
}

impl Default for TagUIState {
    fn default() -> Self {
        Self {
            suggestions: Vec::new(),
            expanded: false,
            confidence_filter: None,
            show_only_pending: true,
            auto_apply_enabled: true,
            custom_tag_input: String::new(),
            show_custom_input: false,
        }
    }
}

/// Actions that can be performed in the tag UI
#[derive(Debug, Clone, PartialEq)]
pub enum TagUIAction {
    /// Accept a suggestion
    AcceptSuggestion { suggestion_index: usize },
    /// Reject a suggestion
    RejectSuggestion { suggestion_index: usize },
    /// Modify a suggestion
    ModifySuggestion { suggestion_index: usize, new_tag: String },
    /// Add a custom tag
    AddCustomTag { tag: String },
    /// Remove an existing tag
    RemoveTag { tag: String },
    /// Toggle the tag panel
    TogglePanel,
    /// Set confidence filter
    SetConfidenceFilter { filter: Option<SuggestionConfidence> },
    /// Toggle show only pending
    ToggleShowOnlyPending,
    /// Toggle auto-apply
    ToggleAutoApply,
    /// Show custom tag input
    ShowCustomInput,
    /// Hide custom tag input
    HideCustomInput,
    /// Update custom tag input
    UpdateCustomInput { text: String },
    /// Refresh suggestions
    RefreshSuggestions,
}

/// Tag management UI component
pub struct TagManagementUI {
    state: TagUIState,
    conversation_id: Option<Uuid>,
    pending_actions: Vec<TagUIAction>,
}

impl TagManagementUI {
    /// Create a new tag management UI
    pub fn new() -> Self {
        Self {
            state: TagUIState::default(),
            conversation_id: None,
            pending_actions: Vec::new(),
        }
    }

    /// Set the conversation being managed
    pub fn set_conversation(&mut self, conversation_id: Uuid) {
        self.conversation_id = Some(conversation_id);
        self.state.suggestions.clear();
    }

    /// Update suggestions for the current conversation
    pub fn update_suggestions(&mut self, suggestions: Vec<TagSuggestion>) {
        self.state.suggestions = suggestions
            .into_iter()
            .map(|suggestion| TagSuggestionWithAction::new(suggestion))
            .collect();
    }

    /// Handle a UI action
    pub fn handle_action(&mut self, action: TagUIAction) -> Vec<String> {
        let mut applied_tags = Vec::new();
        
        match action {
            TagUIAction::AcceptSuggestion { suggestion_index } => {
                if let Some(suggestion_with_action) = self.state.suggestions.get_mut(suggestion_index) {
                    suggestion_with_action.action = TagAction::Accept;
                    suggestion_with_action.action_timestamp = Some(Utc::now());
                    applied_tags.push(suggestion_with_action.suggestion.tag.clone());
                }
            },
            TagUIAction::RejectSuggestion { suggestion_index } => {
                if let Some(suggestion_with_action) = self.state.suggestions.get_mut(suggestion_index) {
                    suggestion_with_action.action = TagAction::Reject;
                    suggestion_with_action.action_timestamp = Some(Utc::now());
                }
            },
            TagUIAction::ModifySuggestion { suggestion_index, new_tag } => {
                if let Some(suggestion_with_action) = self.state.suggestions.get_mut(suggestion_index) {
                    suggestion_with_action.action = TagAction::Modify { new_tag: new_tag.clone() };
                    suggestion_with_action.action_timestamp = Some(Utc::now());
                    applied_tags.push(new_tag);
                }
            },
            TagUIAction::AddCustomTag { tag } => {
                if !tag.trim().is_empty() {
                    applied_tags.push(tag.trim().to_string());
                    self.state.custom_tag_input.clear();
                    self.state.show_custom_input = false;
                }
            },
            TagUIAction::TogglePanel => {
                self.state.expanded = !self.state.expanded;
            },
            TagUIAction::SetConfidenceFilter { filter } => {
                self.state.confidence_filter = filter;
            },
            TagUIAction::ToggleShowOnlyPending => {
                self.state.show_only_pending = !self.state.show_only_pending;
            },
            TagUIAction::ToggleAutoApply => {
                self.state.auto_apply_enabled = !self.state.auto_apply_enabled;
            },
            TagUIAction::ShowCustomInput => {
                self.state.show_custom_input = true;
            },
            TagUIAction::HideCustomInput => {
                self.state.show_custom_input = false;
                self.state.custom_tag_input.clear();
            },
            TagUIAction::UpdateCustomInput { text } => {
                self.state.custom_tag_input = text;
            },
            TagUIAction::RefreshSuggestions => {
                // This would trigger a refresh in the parent component
            },
            _ => {
                // Store action for later processing
                self.pending_actions.push(action);
            }
        }
        
        applied_tags
    }

    /// Get filtered suggestions based on current UI state
    pub fn get_filtered_suggestions(&self) -> Vec<&TagSuggestionWithAction> {
        self.state.suggestions
            .iter()
            .filter(|suggestion_with_action| {
                // Filter by confidence if set
                if let Some(confidence_filter) = &self.state.confidence_filter {
                    if suggestion_with_action.suggestion.confidence_level() < *confidence_filter {
                        return false;
                    }
                }
                
                // Filter by pending status if enabled
                if self.state.show_only_pending {
                    matches!(suggestion_with_action.action, TagAction::Pending)
                } else {
                    true
                }
            })
            .collect()
    }

    /// Get suggestions that should be auto-applied
    pub fn get_auto_apply_suggestions(&self) -> Vec<&TagSuggestionWithAction> {
        if !self.state.auto_apply_enabled {
            return Vec::new();
        }
        
        self.state.suggestions
            .iter()
            .filter(|suggestion_with_action| {
                matches!(suggestion_with_action.action, TagAction::Pending) &&
                suggestion_with_action.suggestion.is_high_confidence()
            })
            .collect()
    }

    /// Auto-apply high confidence suggestions
    pub fn auto_apply_suggestions(&mut self) -> Vec<String> {
        let mut applied_tags = Vec::new();
        
        if self.state.auto_apply_enabled {
            for suggestion_with_action in &mut self.state.suggestions {
                if matches!(suggestion_with_action.action, TagAction::Pending) &&
                   suggestion_with_action.suggestion.is_high_confidence() {
                    suggestion_with_action.action = TagAction::Accept;
                    suggestion_with_action.action_timestamp = Some(Utc::now());
                    applied_tags.push(suggestion_with_action.suggestion.tag.clone());
                }
            }
        }
        
        applied_tags
    }

    /// Get the current UI state
    pub fn get_state(&self) -> &TagUIState {
        &self.state
    }

    /// Get mutable UI state
    pub fn get_state_mut(&mut self) -> &mut TagUIState {
        &mut self.state
    }

    /// Get pending actions and clear them
    pub fn take_pending_actions(&mut self) -> Vec<TagUIAction> {
        std::mem::take(&mut self.pending_actions)
    }

    /// Get statistics about current suggestions
    pub fn get_suggestion_stats(&self) -> TagSuggestionStats {
        let total = self.state.suggestions.len();
        let pending = self.state.suggestions.iter()
            .filter(|s| matches!(s.action, TagAction::Pending))
            .count();
        let accepted = self.state.suggestions.iter()
            .filter(|s| matches!(s.action, TagAction::Accept))
            .count();
        let rejected = self.state.suggestions.iter()
            .filter(|s| matches!(s.action, TagAction::Reject))
            .count();
        let modified = self.state.suggestions.iter()
            .filter(|s| matches!(s.action, TagAction::Modify { .. }))
            .count();
        
        let high_confidence = self.state.suggestions.iter()
            .filter(|s| s.suggestion.is_high_confidence())
            .count();
        
        let by_source = self.state.suggestions.iter()
            .fold(HashMap::new(), |mut acc, s| {
                let source_type = match &s.suggestion.source {
                    TagSource::Embedding { .. } => "Embedding",
                    TagSource::Rule { .. } => "Rule",
                    TagSource::Manual => "Manual",
                    TagSource::Content { .. } => "Content",
                };
                *acc.entry(source_type.to_string()).or_insert(0) += 1;
                acc
            });

        TagSuggestionStats {
            total,
            pending,
            accepted,
            rejected,
            modified,
            high_confidence,
            by_source,
        }
    }

    /// Clear all suggestions
    pub fn clear_suggestions(&mut self) {
        self.state.suggestions.clear();
    }

    /// Get accepted tags
    pub fn get_accepted_tags(&self) -> Vec<String> {
        self.state.suggestions
            .iter()
            .filter_map(|s| match &s.action {
                TagAction::Accept => Some(s.suggestion.tag.clone()),
                TagAction::Modify { new_tag } => Some(new_tag.clone()),
                _ => None,
            })
            .collect()
    }

    /// Get rejected tags for learning
    pub fn get_rejected_tags(&self) -> Vec<String> {
        self.state.suggestions
            .iter()
            .filter_map(|s| match &s.action {
                TagAction::Reject => Some(s.suggestion.tag.clone()),
                _ => None,
            })
            .collect()
    }

    /// Export suggestion history for analysis
    pub fn export_suggestion_history(&self) -> Vec<TagSuggestionWithAction> {
        self.state.suggestions.clone()
    }

    /// Import suggestion history
    pub fn import_suggestion_history(&mut self, history: Vec<TagSuggestionWithAction>) {
        self.state.suggestions = history;
    }
}

/// Statistics about tag suggestions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagSuggestionStats {
    pub total: usize,
    pub pending: usize,
    pub accepted: usize,
    pub rejected: usize,
    pub modified: usize,
    pub high_confidence: usize,
    pub by_source: HashMap<String, usize>,
}

impl TagSuggestionStats {
    /// Get acceptance rate
    pub fn acceptance_rate(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.accepted + self.modified) as f32 / self.total as f32
        }
    }

    /// Get rejection rate
    pub fn rejection_rate(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.rejected as f32 / self.total as f32
        }
    }

    /// Get high confidence rate
    pub fn high_confidence_rate(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.high_confidence as f32 / self.total as f32
        }
    }
}

/// Helper functions for rendering tag suggestions in egui
pub mod egui_helpers {
    use super::*;
    use egui::{Color32, RichText, Ui};

    /// Render a tag suggestion with action buttons
    pub fn render_tag_suggestion(
        ui: &mut Ui,
        suggestion_with_action: &TagSuggestionWithAction,
        index: usize,
    ) -> Option<TagUIAction> {
        let mut action = None;
        
        ui.horizontal(|ui| {
            // Tag name with confidence color
            let confidence_color = match suggestion_with_action.suggestion.confidence_level() {
                SuggestionConfidence::VeryHigh => Color32::from_rgb(0, 150, 0),
                SuggestionConfidence::High => Color32::from_rgb(100, 150, 0),
                SuggestionConfidence::Medium => Color32::from_rgb(200, 150, 0),
                SuggestionConfidence::Low => Color32::from_rgb(150, 100, 0),
            };
            
            ui.label(
                RichText::new(&suggestion_with_action.suggestion.tag)
                    .color(confidence_color)
                    .strong()
            );
            
            // Confidence score
            ui.label(format!("{:.1}%", suggestion_with_action.suggestion.confidence * 100.0));
            
            // Source indicator
            let source_text = match &suggestion_with_action.suggestion.source {
                TagSource::Embedding { similarity_score } => format!("🧠 {:.2}", similarity_score),
                TagSource::Rule { rule_name } => format!("📋 {}", rule_name),
                TagSource::Manual => "👤 Manual".to_string(),
                TagSource::Content { keywords } => format!("📝 {}", keywords.len()),
            };
            ui.label(RichText::new(source_text).small());
            
            // Action buttons based on current state
            match &suggestion_with_action.action {
                TagAction::Pending => {
                    if ui.small_button("✓").clicked() {
                        action = Some(TagUIAction::AcceptSuggestion { suggestion_index: index });
                    }
                    if ui.small_button("✗").clicked() {
                        action = Some(TagUIAction::RejectSuggestion { suggestion_index: index });
                    }
                    if ui.small_button("✏").clicked() {
                        // This would open a text input dialog
                        action = Some(TagUIAction::ModifySuggestion { 
                            suggestion_index: index, 
                            new_tag: suggestion_with_action.suggestion.tag.clone() 
                        });
                    }
                },
                TagAction::Accept => {
                    ui.label(RichText::new("✓ Accepted").color(Color32::GREEN));
                },
                TagAction::Reject => {
                    ui.label(RichText::new("✗ Rejected").color(Color32::RED));
                },
                TagAction::Modify { new_tag } => {
                    ui.label(RichText::new(format!("✏ → {}", new_tag)).color(Color32::BLUE));
                },
            }
        });
        
        // Show reasoning on hover or in a collapsible section
        if ui.small_button("?").on_hover_text(&suggestion_with_action.suggestion.reasoning).clicked() {
            // Could expand to show full reasoning
        }
        
        action
    }

    /// Render the tag management panel
    pub fn render_tag_panel(ui: &mut Ui, tag_ui: &mut TagManagementUI) -> Vec<TagUIAction> {
        let mut actions = Vec::new();
        
        ui.collapsing("🏷️ Tag Suggestions", |ui| {
            // Controls
            ui.horizontal(|ui| {
                if ui.button("Refresh").clicked() {
                    actions.push(TagUIAction::RefreshSuggestions);
                }
                
                ui.checkbox(&mut tag_ui.state.auto_apply_enabled, "Auto-apply high confidence");
                ui.checkbox(&mut tag_ui.state.show_only_pending, "Show only pending");
            });
            
            // Confidence filter
            ui.horizontal(|ui| {
                ui.label("Min confidence:");
                render_confidence_filter(ui, &mut tag_ui.state.confidence_filter);
            });
            
            // Statistics
            let stats = tag_ui.get_suggestion_stats();
            ui.horizontal(|ui| {
                ui.label(format!("Total: {}", stats.total));
                ui.label(format!("Pending: {}", stats.pending));
                ui.label(format!("Accepted: {}", stats.accepted));
                ui.label(format!("Acceptance: {:.1}%", stats.acceptance_rate() * 100.0));
            });
            
            ui.separator();
            
            // Suggestions list
            let filtered_suggestions = tag_ui.get_filtered_suggestions();
            if filtered_suggestions.is_empty() {
                ui.label("No suggestions available");
            } else {
                for (display_index, suggestion_with_action) in filtered_suggestions.iter().enumerate() {
                    // Find the actual index in the full list
                    let actual_index = tag_ui.state.suggestions.iter()
                        .position(|s| std::ptr::eq(s, *suggestion_with_action))
                        .unwrap_or(display_index);
                    
                    if let Some(action) = render_tag_suggestion(ui, suggestion_with_action, actual_index) {
                        actions.push(action);
                    }
                    ui.separator();
                }
            }
            
            // Custom tag input
            ui.horizontal(|ui| {
                if !tag_ui.state.show_custom_input {
                    if ui.button("+ Add custom tag").clicked() {
                        actions.push(TagUIAction::ShowCustomInput);
                    }
                } else {
                    ui.label("Custom tag:");
                    let response = ui.text_edit_singleline(&mut tag_ui.state.custom_tag_input);
                    
                    if ui.button("Add").clicked() || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                        actions.push(TagUIAction::AddCustomTag { 
                            tag: tag_ui.state.custom_tag_input.clone() 
                        });
                    }
                    
                    if ui.button("Cancel").clicked() {
                        actions.push(TagUIAction::HideCustomInput);
                    }
                }
            });
        });
        
        actions
    }

    /// Helper function to render confidence filter dropdown
    fn render_confidence_filter(ui: &mut egui::Ui, filter: &mut Option<SuggestionConfidence>) {
        let mut filter_index = match filter {
            None => 0,
            Some(SuggestionConfidence::Low) => 1,
            Some(SuggestionConfidence::Medium) => 2,
            Some(SuggestionConfidence::High) => 3,
            Some(SuggestionConfidence::VeryHigh) => 4,
        };

        egui::ComboBox::from_label("Min Confidence")
            .selected_text(match filter_index {
                0 => "All",
                1 => "Low",
                2 => "Medium", 
                3 => "High",
                4 => "Very High",
                _ => "All",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut filter_index, 0, "All");
                ui.selectable_value(&mut filter_index, 1, "Low");
                ui.selectable_value(&mut filter_index, 2, "Medium");
                ui.selectable_value(&mut filter_index, 3, "High");
                ui.selectable_value(&mut filter_index, 4, "Very High");
            });

        *filter = match filter_index {
            0 => None,
            1 => Some(SuggestionConfidence::Low),
            2 => Some(SuggestionConfidence::Medium),
            3 => Some(SuggestionConfidence::High),
            4 => Some(SuggestionConfidence::VeryHigh),
            _ => None,
        };
    }
}

impl Default for TagManagementUI {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::conversation::types::Conversation;

    fn create_test_suggestion() -> TagSuggestion {
        TagSuggestion::new(
            "rust".to_string(),
            0.8,
            "High confidence Rust detection".to_string(),
            TagSource::Embedding { similarity_score: 0.8 },
        )
    }

    #[test]
    fn test_tag_ui_creation() {
        let ui = TagManagementUI::new();
        assert!(!ui.state.expanded);
        assert!(ui.state.suggestions.is_empty());
        assert!(ui.conversation_id.is_none());
    }

    #[test]
    fn test_update_suggestions() {
        let mut ui = TagManagementUI::new();
        let conversation_id = Uuid::new_v4();
        ui.set_conversation(conversation_id);
        
        let suggestions = vec![create_test_suggestion()];
        ui.update_suggestions(suggestions);
        
        assert_eq!(ui.state.suggestions.len(), 1);
        assert_eq!(ui.state.suggestions[0].suggestion.tag, "rust");
        assert!(matches!(ui.state.suggestions[0].action, TagAction::Pending));
    }

    #[test]
    fn test_accept_suggestion() {
        let mut ui = TagManagementUI::new();
        ui.set_conversation(Uuid::new_v4());
        ui.update_suggestions(vec![create_test_suggestion()]);
        
        let applied_tags = ui.handle_action(TagUIAction::AcceptSuggestion { suggestion_index: 0 });
        
        assert_eq!(applied_tags.len(), 1);
        assert_eq!(applied_tags[0], "rust");
        assert!(matches!(ui.state.suggestions[0].action, TagAction::Accept));
        assert!(ui.state.suggestions[0].action_timestamp.is_some());
    }

    #[test]
    fn test_reject_suggestion() {
        let mut ui = TagManagementUI::new();
        ui.set_conversation(Uuid::new_v4());
        ui.update_suggestions(vec![create_test_suggestion()]);
        
        let applied_tags = ui.handle_action(TagUIAction::RejectSuggestion { suggestion_index: 0 });
        
        assert!(applied_tags.is_empty());
        assert!(matches!(ui.state.suggestions[0].action, TagAction::Reject));
        assert!(ui.state.suggestions[0].action_timestamp.is_some());
    }

    #[test]
    fn test_modify_suggestion() {
        let mut ui = TagManagementUI::new();
        ui.set_conversation(Uuid::new_v4());
        ui.update_suggestions(vec![create_test_suggestion()]);
        
        let applied_tags = ui.handle_action(TagUIAction::ModifySuggestion { 
            suggestion_index: 0, 
            new_tag: "rust-lang".to_string() 
        });
        
        assert_eq!(applied_tags.len(), 1);
        assert_eq!(applied_tags[0], "rust-lang");
        assert!(matches!(ui.state.suggestions[0].action, TagAction::Modify { .. }));
    }

    #[test]
    fn test_custom_tag() {
        let mut ui = TagManagementUI::new();
        
        let applied_tags = ui.handle_action(TagUIAction::AddCustomTag { 
            tag: "  custom-tag  ".to_string() 
        });
        
        assert_eq!(applied_tags.len(), 1);
        assert_eq!(applied_tags[0], "custom-tag");
    }

    #[test]
    fn test_auto_apply() {
        let mut ui = TagManagementUI::new();
        ui.set_conversation(Uuid::new_v4());
        ui.state.auto_apply_enabled = true;
        
        // High confidence suggestion
        let high_conf_suggestion = TagSuggestion::new(
            "rust".to_string(),
            0.9,
            "Very high confidence".to_string(),
            TagSource::Embedding { similarity_score: 0.9 },
        );
        
        // Low confidence suggestion
        let low_conf_suggestion = TagSuggestion::new(
            "maybe".to_string(),
            0.3,
            "Low confidence".to_string(),
            TagSource::Content { keywords: vec![] },
        );
        
        ui.update_suggestions(vec![high_conf_suggestion, low_conf_suggestion]);
        
        let applied_tags = ui.auto_apply_suggestions();
        
        assert_eq!(applied_tags.len(), 1);
        assert_eq!(applied_tags[0], "rust");
        assert!(matches!(ui.state.suggestions[0].action, TagAction::Accept));
        assert!(matches!(ui.state.suggestions[1].action, TagAction::Pending));
    }

    #[test]
    fn test_filtering() {
        let mut ui = TagManagementUI::new();
        ui.set_conversation(Uuid::new_v4());
        
        let high_conf = TagSuggestion::new(
            "rust".to_string(),
            0.9,
            "High confidence".to_string(),
            TagSource::Embedding { similarity_score: 0.9 },
        );
        
        let low_conf = TagSuggestion::new(
            "maybe".to_string(),
            0.3,
            "Low confidence".to_string(),
            TagSource::Content { keywords: vec![] },
        );
        
        ui.update_suggestions(vec![high_conf, low_conf]);
        
        // Test confidence filter
        ui.state.confidence_filter = Some(SuggestionConfidence::High);
        let filtered = ui.get_filtered_suggestions();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].suggestion.tag, "rust");
        
        // Test pending filter
        ui.handle_action(TagUIAction::AcceptSuggestion { suggestion_index: 0 });
        ui.state.show_only_pending = true;
        let filtered = ui.get_filtered_suggestions();
        assert_eq!(filtered.len(), 0); // High conf was accepted, low conf filtered out by confidence
        
        ui.state.confidence_filter = None;
        let filtered = ui.get_filtered_suggestions();
        assert_eq!(filtered.len(), 1); // Only the low confidence pending one
        assert_eq!(filtered[0].suggestion.tag, "maybe");
    }

    #[test]
    fn test_statistics() {
        let mut ui = TagManagementUI::new();
        ui.set_conversation(Uuid::new_v4());
        
        let suggestions = vec![
            TagSuggestion::new("rust".to_string(), 0.9, "High".to_string(), TagSource::Embedding { similarity_score: 0.9 }),
            TagSuggestion::new("python".to_string(), 0.7, "Medium".to_string(), TagSource::Rule { rule_name: "lang".to_string() }),
            TagSuggestion::new("debug".to_string(), 0.5, "Low".to_string(), TagSource::Content { keywords: vec![] }),
        ];
        
        ui.update_suggestions(suggestions);
        
        ui.handle_action(TagUIAction::AcceptSuggestion { suggestion_index: 0 });
        ui.handle_action(TagUIAction::RejectSuggestion { suggestion_index: 1 });
        ui.handle_action(TagUIAction::ModifySuggestion { suggestion_index: 2, new_tag: "debugging".to_string() });
        
        let stats = ui.get_suggestion_stats();
        
        assert_eq!(stats.total, 3);
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.accepted, 1);
        assert_eq!(stats.rejected, 1);
        assert_eq!(stats.modified, 1);
        assert_eq!(stats.high_confidence, 2); // rust (0.9) and python (0.7) are both >= 0.6
        assert_eq!(stats.acceptance_rate(), 2.0 / 3.0); // accepted + modified
        assert_eq!(stats.rejection_rate(), 1.0 / 3.0);
        
        assert_eq!(stats.by_source.get("Embedding"), Some(&1));
        assert_eq!(stats.by_source.get("Rule"), Some(&1));
        assert_eq!(stats.by_source.get("Content"), Some(&1));
    }
} 