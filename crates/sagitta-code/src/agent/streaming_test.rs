use serde_json::{Deserializer, Value};

fn main() {
    // Test case: Multiple JSON objects, some with newlines, some without
    let test_data = r#"{"type":"system","subtype":"init"}
{"type":"assistant","message":{"content":[{"type":"tool_use","id":"tool1"}]}}
{"type":"result","result":{"content":"Result 1"}}{"type":"assistant","message":{"content":[{"type":"tool_use","id":"tool2"}]}}{"type":"result","result":{"content":"Result 2"}}"#;
    
    println!("Test data:\n{}\n", test_data);
    println!("Testing JSON parsing with current approach...\n");
    
    let mut json_buffer = test_data.as_bytes().to_vec();
    let mut processed_count = 0;
    
    while !json_buffer.is_empty() {
        println!("Buffer size: {} bytes", json_buffer.len());
        
        // Current approach from streaming.rs
        let deserializer = Deserializer::from_slice(&json_buffer).into_iter::<Value>();
        let mut bytes_consumed = 0;
        
        for (idx, result) in deserializer.enumerate() {
            match result {
                Ok(value) => {
                    processed_count += 1;
                    println!("Parsed object #{}: {:?}", processed_count, value.get("type"));
                    
                    // Current buggy approach: only looks for first newline
                    bytes_consumed = json_buffer.iter().position(|&b| b == b'\n')
                        .map(|p| p + 1)
                        .unwrap_or(json_buffer.len());
                    
                    println!("  Current approach would consume: {} bytes", bytes_consumed);
                }
                Err(e) => {
                    println!("Parse error: {:?}", e);
                    if e.is_eof() {
                        println!("  EOF - need more data");
                    }
                    break;
                }
            }
        }
        
        if bytes_consumed > 0 {
            println!("Draining {} bytes from buffer", bytes_consumed);
            json_buffer.drain(..bytes_consumed);
        } else {
            println!("No bytes consumed, breaking");
            break;
        }
        
        println!("---");
    }
    
    println!("\nProcessed {} objects total", processed_count);
    if !json_buffer.is_empty() {
        println!("WARNING: {} bytes left unprocessed!", json_buffer.len());
        if let Ok(s) = std::str::from_utf8(&json_buffer) {
            println!("Unprocessed data: {}", s);
        }
    }
    
    // Now test the proper approach
    println!("\n\nTesting with proper byte tracking...\n");
    
    let mut json_buffer = test_data.as_bytes().to_vec();
    let mut processed_count = 0;
    
    while !json_buffer.is_empty() {
        println!("Buffer size: {} bytes", json_buffer.len());
        
        let mut stream = Deserializer::from_slice(&json_buffer).into_iter::<Value>();
        let mut last_valid_offset = 0;
        
        loop {
            let current_offset = stream.byte_offset();
            match stream.next() {
                Some(Ok(value)) => {
                    processed_count += 1;
                    println!("Parsed object #{}: {:?}", processed_count, value.get("type"));
                    last_valid_offset = stream.byte_offset();
                    println!("  Consumed up to byte: {}", last_valid_offset);
                }
                Some(Err(e)) if e.is_eof() => {
                    println!("EOF at byte {}", current_offset);
                    break;
                }
                Some(Err(e)) => {
                    println!("Parse error at byte {}: {:?}", current_offset, e);
                    // Skip to next likely JSON boundary
                    last_valid_offset = current_offset + 1;
                    break;
                }
                None => {
                    println!("Iterator exhausted");
                    break;
                }
            }
        }
        
        if last_valid_offset > 0 {
            println!("Draining {} bytes from buffer", last_valid_offset);
            json_buffer.drain(..last_valid_offset);
        } else {
            println!("No valid offset, breaking");
            break;
        }
        
        println!("---");
    }
    
    println!("\nProcessed {} objects total", processed_count);
    if !json_buffer.is_empty() {
        println!("Bytes left: {}", json_buffer.len());
    }
}