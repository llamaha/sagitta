use serde_json::json;

fn main() {
    let delta = json\!({
        "content": "Let me <think>process this internally</think> and the answer is 42"
    });
    
    println\!("Delta: {:?}", delta);
    
    // The stream_processor should convert this to:
    // 1. Text: "Let me "
    // 2. Thought: "process this internally"  
    // 3. Text: " and the answer is 42"
}
