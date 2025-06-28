use serde_json::{Value, Map};

pub struct IncrementalJsonParser {
    buffer: String,
    in_string: bool,
    escape_next: bool,
    depth: usize,
    current_path: Vec<String>,
    in_text_field: bool,
    brace_stack: Vec<char>,
}

impl IncrementalJsonParser {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            in_string: false,
            escape_next: false,
            depth: 0,
            current_path: Vec::new(),
            in_text_field: false,
            brace_stack: Vec::new(),
        }
    }
    
    pub fn feed(&mut self, data: &str) -> Vec<StreamEvent> {
        let mut events = Vec::new();
        
        for ch in data.chars() {
            self.buffer.push(ch);
            
            // Detect text content streaming first, before JSON structure tracking
            if self.in_text_field && !self.escape_next {
                if ch == '\\' {
                    // Starting an escape sequence, don't emit the backslash
                    self.escape_next = true;
                } else if ch == '"' {
                    // Ending the text field
                    self.in_text_field = false;
                    self.in_string = false;
                } else {
                    // Regular character
                    events.push(StreamEvent::TextChar(ch));
                }
            } else if self.in_text_field && self.escape_next {
                // Process escaped character
                let escaped_char = match ch {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    '\\' => '\\',
                    '"' => '"',
                    '/' => '/',
                    'b' => '\u{0008}',  // backspace
                    'f' => '\u{000C}',  // form feed
                    _ => ch,  // For unrecognized escapes, just use the character
                };
                events.push(StreamEvent::TextChar(escaped_char));
                self.escape_next = false;
            } else {
                // Track JSON structure
                if !self.escape_next {
                    match ch {
                        '"' if !self.in_string => {
                            self.in_string = true;
                            // Check if we're entering a "text" field
                            // Need to check before adding the current quote
                            let buf_before_quote = &self.buffer[..self.buffer.len()-1];
                            if buf_before_quote.ends_with("\"text\":") {
                                self.in_text_field = true;
                            }
                        }
                        '"' if self.in_string => {
                            self.in_string = false;
                        }
                        '\\' if self.in_string => self.escape_next = true,
                        '{' if !self.in_string => {
                            self.depth += 1;
                            self.brace_stack.push('{');
                        }
                        '[' if !self.in_string => {
                            self.depth += 1;
                            self.brace_stack.push('[');
                        }
                        '}' if !self.in_string => {
                            if let Some('{') = self.brace_stack.last() {
                                self.brace_stack.pop();
                                self.depth -= 1;
                                if self.depth == 0 {
                                    // Complete JSON object
                                    if let Ok(value) = serde_json::from_str::<Value>(&self.buffer) {
                                        events.push(StreamEvent::CompleteJson(value));
                                        self.buffer.clear();
                                        self.current_path.clear();
                                    }
                                }
                            }
                        }
                        ']' if !self.in_string => {
                            if let Some('[') = self.brace_stack.last() {
                                self.brace_stack.pop();
                                self.depth -= 1;
                            }
                        }
                        _ => {}
                    }
                } else {
                    self.escape_next = false;
                }
            }
        }
        
        events
    }
    
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.in_string = false;
        self.escape_next = false;
        self.depth = 0;
        self.current_path.clear();
        self.in_text_field = false;
        self.brace_stack.clear();
    }
}

#[derive(Debug)]
pub enum StreamEvent {
    TextChar(char),
    CompleteJson(Value),
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_incremental_text_parsing() {
        let mut parser = IncrementalJsonParser::new();
        
        // Simulate streaming JSON with text content
        let chunks = vec![
            r#"{"type":"ass"#,
            r#"istant","mes"#,
            r#"sage":{"conte"#,
            r#"nt":[{"type":"#,
            r#""text","text":"#,
            r#""H"#,
            r#"e"#,
            r#"l"#,
            r#"l"#,
            r#"o"#,
            r#" "#,
            r#"w"#,
            r#"o"#,
            r#"r"#,
            r#"l"#,
            r#"d"#,
            r#""}]}}"#,
        ];
        
        let mut text_chars = Vec::new();
        let mut complete_jsons = Vec::new();
        
        for chunk in chunks {
            let events = parser.feed(chunk);
            for event in events {
                match event {
                    StreamEvent::TextChar(ch) => text_chars.push(ch),
                    StreamEvent::CompleteJson(json) => complete_jsons.push(json),
                }
            }
        }
        
        // Check that we captured the text characters
        let text: String = text_chars.into_iter().collect();
        assert_eq!(text, "Hello world");
        
        // Check that we got the complete JSON
        assert_eq!(complete_jsons.len(), 1);
    }
    
    #[test]
    fn test_escaped_characters() {
        let mut parser = IncrementalJsonParser::new();
        
        let chunk = r#"{"text":"Line 1\nLine 2\"quoted\""}"#;
        println!("Test input: {}", chunk);
        let events = parser.feed(chunk);
        
        // Debug: print all events
        for (i, event) in events.iter().enumerate() {
            match event {
                StreamEvent::TextChar(ch) => println!("Event {}: TextChar('{}')", i, ch),
                StreamEvent::CompleteJson(_) => println!("Event {}: CompleteJson", i),
            }
        }
        
        let text_chars: Vec<char> = events.iter()
            .filter_map(|e| match e {
                StreamEvent::TextChar(ch) => Some(*ch),
                _ => None,
            })
            .collect();
        
        let text: String = text_chars.into_iter().collect();
        println!("Extracted: '{}'", text);
        println!("Expected:  'Line 1\\nLine 2\"quoted\"'");
        assert_eq!(text, "Line 1\nLine 2\"quoted\"");
    }
}