// Helper for managing collapsing headers with global and individual states

use egui::{CollapsingHeader, Ui};
use std::collections::HashMap;

/// Creates a collapsing header that respects both global and individual collapse states
pub fn create_controlled_collapsing_header<R>(
    ui: &mut Ui,
    id: egui::Id,
    header: impl Into<egui::WidgetText>,
    should_be_open: bool,
    has_individual_override: bool,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> egui::collapsing_header::CollapsingResponse<R> {
    // Create the header with proper ID - let egui handle interaction naturally
    let header = CollapsingHeader::new(header)
        .id_salt(id)
        .default_open(should_be_open);
    
    // Make the collapsing header respond to clicks immediately
    if has_individual_override {
        // User has manually toggled - let egui handle state naturally
        header.show(ui, add_contents)
    } else {
        // No manual override - force the global state
        // Note: open() method takes Option<bool> not bool
        header.open(Some(should_be_open)).show(ui, add_contents)
    }
}

/// Helper to determine if a tool card should be open and whether it has an override
pub fn get_tool_card_state(
    tool_card_id: &str,
    global_collapsed: bool,
    individual_states: &HashMap<String, bool>,
) -> (bool, bool) {
    if let Some(&individual_state) = individual_states.get(tool_card_id) {
        // Individual state overrides global state
        let should_be_open = !individual_state; // individual_state true means collapsed
        (should_be_open, true) // has override
    } else {
        // Use global state
        let should_be_open = !global_collapsed; // global_collapsed true means collapsed
        (should_be_open, false) // no override
    }
}