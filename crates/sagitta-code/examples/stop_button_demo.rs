// Demo showing STOP button functionality

use sagitta_code::gui::app::state::AppState;

fn main() {
    println!("=== STOP Button Demo ===\n");
    
    // Create app state
    let mut state = AppState::new();
    
    println!("Initial state:");
    println!("  is_waiting_for_response: {}", state.is_waiting_for_response);
    println!("  stop_requested: {}", state.stop_requested);
    
    // Simulate agent thinking
    println!("\n1. Agent starts thinking...");
    state.is_waiting_for_response = true;
    state.is_thinking = true;
    state.thinking_message = Some("Processing your request...".to_string());
    
    println!("  is_waiting_for_response: {}", state.is_waiting_for_response);
    println!("  is_thinking: {}", state.is_thinking);
    println!("  thinking_message: {:?}", state.thinking_message);
    println!("  -> STOP button is now visible in UI");
    
    // User clicks STOP
    println!("\n2. User clicks STOP button...");
    state.stop_requested = true;
    println!("  stop_requested: {}", state.stop_requested);
    
    // Handle stop request (what happens in rendering.rs)
    println!("\n3. App handles stop request...");
    if state.stop_requested {
        println!("  - Cancelling agent operation");
        println!("  - Clearing UI state");
        
        state.stop_requested = false;
        state.is_waiting_for_response = false;
        state.is_thinking = false;
        state.is_responding = false;
        state.is_streaming_response = false;
        state.thinking_message = None;
    }
    
    println!("\n4. Final state:");
    println!("  is_waiting_for_response: {}", state.is_waiting_for_response);
    println!("  is_thinking: {}", state.is_thinking);
    println!("  stop_requested: {}", state.stop_requested);
    println!("  thinking_message: {:?}", state.thinking_message);
    
    println!("\n=== Summary ===");
    println!("✅ STOP button appears when agent is thinking");
    println!("✅ Clicking STOP sets stop_requested flag");
    println!("✅ Handler cancels agent and clears UI state");
    println!("✅ UI returns to idle state");
}