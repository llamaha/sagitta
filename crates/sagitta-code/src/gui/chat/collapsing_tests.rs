// Tests for tool card collapsing behavior

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use super::super::view::collapsing_header_helper::{create_controlled_collapsing_header, get_tool_card_state};
    
    /// Test helper to determine if a tool card should be open (logic-only)
    fn should_tool_card_be_open(
        tool_card_id: &str,
        global_collapsed: bool,
        individual_states: &HashMap<String, bool>,
    ) -> bool {
        if let Some(&individual_state) = individual_states.get(tool_card_id) {
            // Individual state overrides global state
            !individual_state // individual_state true means collapsed, so we invert for open
        } else {
            // Use global state
            !global_collapsed // global_collapsed true means collapsed, so we invert for open
        }
    }

    /// Mock tool card for testing
    #[derive(Clone)]
    struct MockToolCard {
        id: String,
        name: String,
    }

    /// Test state manager that tracks what should happen
    struct CollapseBehaviorTester {
        global_collapsed: bool,
        individual_states: HashMap<String, bool>,
        tool_cards: Vec<MockToolCard>,
    }

    impl CollapseBehaviorTester {
        fn new() -> Self {
            Self {
                global_collapsed: false,
                individual_states: HashMap::new(),
                tool_cards: vec![
                    MockToolCard { id: "tool1".to_string(), name: "Tool 1".to_string() },
                    MockToolCard { id: "tool2".to_string(), name: "Tool 2".to_string() },
                    MockToolCard { id: "tool3".to_string(), name: "Tool 3".to_string() },
                ],
            }
        }

        /// Simulate clicking the global collapse/expand button
        fn click_global_toggle(&mut self) {
            self.global_collapsed = !self.global_collapsed;
            // Key behavior: global toggle should clear individual overrides
            // so ALL cards follow the new global state
            self.individual_states.clear();
        }

        /// Simulate clicking on an individual tool card header
        fn click_individual_card(&mut self, card_id: &str) {
            let (should_be_open, _has_override) = get_tool_card_state(
                card_id, 
                self.global_collapsed, 
                &self.individual_states
            );
            
            // Toggle the current state by setting individual override
            let new_collapsed_state = should_be_open; // If it's open, we want to collapse it
            self.individual_states.insert(card_id.to_string(), new_collapsed_state);
        }

        /// Check if a card should be open according to current state
        fn is_card_open(&self, card_id: &str) -> bool {
            let (should_be_open, _has_override) = get_tool_card_state(
                card_id, 
                self.global_collapsed, 
                &self.individual_states
            );
            should_be_open
        }

        /// Get all card states
        fn get_all_states(&self) -> HashMap<String, bool> {
            self.tool_cards.iter()
                .map(|card| (card.id.clone(), self.is_card_open(&card.id)))
                .collect()
        }
    }
    
    #[test]
    fn test_global_collapse_without_overrides() {
        let individual_states: HashMap<String, bool> = HashMap::new();
        
        // When global is collapsed (true), cards should be closed (false)
        assert_eq!(should_tool_card_be_open("card1", true, &individual_states), false);
        assert_eq!(should_tool_card_be_open("card2", true, &individual_states), false);
        
        // When global is expanded (false), cards should be open (true)
        assert_eq!(should_tool_card_be_open("card1", false, &individual_states), true);
        assert_eq!(should_tool_card_be_open("card2", false, &individual_states), true);
    }
    
    #[test]
    fn test_individual_override_when_global_collapsed() {
        let mut individual_states: HashMap<String, bool> = HashMap::new();
        
        // Global is collapsed
        let global_collapsed = true;
        
        // card1 has individual override to be expanded (false = not collapsed)
        individual_states.insert("card1".to_string(), false);
        
        // card1 should be open despite global collapse
        assert_eq!(should_tool_card_be_open("card1", global_collapsed, &individual_states), true);
        
        // card2 should follow global state (closed)
        assert_eq!(should_tool_card_be_open("card2", global_collapsed, &individual_states), false);
    }
    
    #[test]
    fn test_individual_override_when_global_expanded() {
        let mut individual_states: HashMap<String, bool> = HashMap::new();
        
        // Global is expanded
        let global_collapsed = false;
        
        // card1 has individual override to be collapsed (true = collapsed)
        individual_states.insert("card1".to_string(), true);
        
        // card1 should be closed despite global expansion
        assert_eq!(should_tool_card_be_open("card1", global_collapsed, &individual_states), false);
        
        // card2 should follow global state (open)
        assert_eq!(should_tool_card_be_open("card2", global_collapsed, &individual_states), true);
    }
    
    #[test]
    fn test_toggle_behavior_simulation() {
        let mut individual_states: HashMap<String, bool> = HashMap::new();
        let mut global_collapsed = false;
        
        // Initial state: all expanded
        assert_eq!(should_tool_card_be_open("card1", global_collapsed, &individual_states), true);
        assert_eq!(should_tool_card_be_open("card2", global_collapsed, &individual_states), true);
        
        // User clicks global toggle to collapse all
        global_collapsed = true;
        assert_eq!(should_tool_card_be_open("card1", global_collapsed, &individual_states), false);
        assert_eq!(should_tool_card_be_open("card2", global_collapsed, &individual_states), false);
        
        // User manually expands card1 (overrides global)
        individual_states.insert("card1".to_string(), false); // false = not collapsed = open
        assert_eq!(should_tool_card_be_open("card1", global_collapsed, &individual_states), true);
        assert_eq!(should_tool_card_be_open("card2", global_collapsed, &individual_states), false);
        
        // User clicks global toggle to expand all
        global_collapsed = false;
        // card1 keeps its individual state (open)
        assert_eq!(should_tool_card_be_open("card1", global_collapsed, &individual_states), true);
        // card2 follows global (open)
        assert_eq!(should_tool_card_be_open("card2", global_collapsed, &individual_states), true);
        
        // User manually collapses card1
        individual_states.insert("card1".to_string(), true); // true = collapsed
        assert_eq!(should_tool_card_be_open("card1", global_collapsed, &individual_states), false);
        assert_eq!(should_tool_card_be_open("card2", global_collapsed, &individual_states), true);
    }
    
    #[test]
    fn test_clear_individual_states_on_global_toggle() {
        // Alternative behavior: clear individual states when global toggle is used
        let mut individual_states: HashMap<String, bool> = HashMap::new();
        let mut global_collapsed = false;
        
        // Set some individual states
        individual_states.insert("card1".to_string(), true); // collapsed
        individual_states.insert("card2".to_string(), false); // expanded
        
        // When global toggle is clicked, we could clear individual states
        individual_states.clear();
        global_collapsed = true;
        
        // All cards should now follow global state
        assert_eq!(should_tool_card_be_open("card1", global_collapsed, &individual_states), false);
        assert_eq!(should_tool_card_be_open("card2", global_collapsed, &individual_states), false);
    }

    // NEW TESTS THAT CAPTURE THE REAL ISSUES

    #[test]
    fn test_global_collapse_affects_all_cards() {
        let mut tester = CollapseBehaviorTester::new();
        
        // Initially all should be open (global_collapsed = false)
        assert!(tester.is_card_open("tool1"));
        assert!(tester.is_card_open("tool2"));
        assert!(tester.is_card_open("tool3"));
        
        // Click global collapse - ALL cards should close
        tester.click_global_toggle();
        assert!(!tester.is_card_open("tool1"), "tool1 should be collapsed after global toggle");
        assert!(!tester.is_card_open("tool2"), "tool2 should be collapsed after global toggle");
        assert!(!tester.is_card_open("tool3"), "tool3 should be collapsed after global toggle");
        
        // Click global expand - ALL cards should open
        tester.click_global_toggle();
        assert!(tester.is_card_open("tool1"), "tool1 should be open after global toggle");
        assert!(tester.is_card_open("tool2"), "tool2 should be open after global toggle");
        assert!(tester.is_card_open("tool3"), "tool3 should be open after global toggle");
    }

    #[test]
    fn test_global_toggle_clears_individual_overrides() {
        let mut tester = CollapseBehaviorTester::new();
        
        // Set some individual overrides
        tester.click_individual_card("tool1"); // collapse tool1
        tester.click_individual_card("tool2"); // collapse tool2
        
        // Verify individual overrides work
        assert!(!tester.is_card_open("tool1"), "tool1 should be collapsed individually");
        assert!(!tester.is_card_open("tool2"), "tool2 should be collapsed individually");
        assert!(tester.is_card_open("tool3"), "tool3 should follow global state (open)");
        
        // Click global toggle - should clear all individual states
        tester.click_global_toggle();
        
        // Now ALL should follow the new global state (collapsed)
        assert!(!tester.is_card_open("tool1"), "tool1 should follow global collapse");
        assert!(!tester.is_card_open("tool2"), "tool2 should follow global collapse");
        assert!(!tester.is_card_open("tool3"), "tool3 should follow global collapse");
        
        // Toggle global again - ALL should be open
        tester.click_global_toggle();
        assert!(tester.is_card_open("tool1"), "tool1 should follow global expand");
        assert!(tester.is_card_open("tool2"), "tool2 should follow global expand");
        assert!(tester.is_card_open("tool3"), "tool3 should follow global expand");
    }

    #[test]
    fn test_individual_toggle_creates_override() {
        let mut tester = CollapseBehaviorTester::new();
        
        // Start with all open (global_collapsed = false)
        assert!(tester.is_card_open("tool1"));
        
        // Click individual card to collapse it
        tester.click_individual_card("tool1");
        assert!(!tester.is_card_open("tool1"), "tool1 should be collapsed after individual click");
        assert!(tester.is_card_open("tool2"), "tool2 should remain open");
        
        // Click same card again to expand it
        tester.click_individual_card("tool1");
        assert!(tester.is_card_open("tool1"), "tool1 should be open after second individual click");
    }

    #[test]
    fn test_consistent_single_click_behavior() {
        // This test captures the "double-click" issue
        let mut tester = CollapseBehaviorTester::new();
        
        // Test that one click always works - no "focus" issues
        for _ in 0..10 {
            let initial_state = tester.is_card_open("tool1");
            tester.click_individual_card("tool1");
            let new_state = tester.is_card_open("tool1");
            
            assert_ne!(initial_state, new_state, "Single click should always toggle state");
        }
    }

    #[test]
    fn test_mixed_individual_and_global_states() {
        let mut tester = CollapseBehaviorTester::new();
        
        // Create mixed state: some individual overrides
        tester.click_individual_card("tool1"); // collapse tool1 (override)
        // tool2 and tool3 follow global (open)
        
        assert!(!tester.is_card_open("tool1"), "tool1 has individual override (collapsed)");
        assert!(tester.is_card_open("tool2"), "tool2 follows global (open)");
        assert!(tester.is_card_open("tool3"), "tool3 follows global (open)");
        
        // Global collapse should affect all
        tester.click_global_toggle();
        assert!(!tester.is_card_open("tool1"), "tool1 follows new global state");
        assert!(!tester.is_card_open("tool2"), "tool2 follows new global state");
        assert!(!tester.is_card_open("tool3"), "tool3 follows new global state");
        
        // Individual overrides should be cleared
        assert!(tester.individual_states.is_empty(), "Individual states should be cleared after global toggle");
    }
}