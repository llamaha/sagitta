use crate::llm::client::MessagePart;
use uuid::Uuid;

/// Parse tool calls from text containing XML tags using the new format
/// where the tool name is the XML tag itself
pub fn parse_tool_calls_from_text(text: &str) -> (String, Vec<MessagePart>) {
    let mut remaining_text = String::new();
    let mut tool_calls = Vec::new();
    let mut current_pos = 0;
    
    // Get list of known tool names to match against
    let known_tools = vec![
        "websearch", "web_search", "WebSearch",
        "repository_add", "repository_sync", "repository_query", "repository_list",
        "read_file", "write_file", "edit_file",
        "list_files", "search_files",
        "run_command", "shell", "bash",
        // Add more tool names as needed
    ];
    
    let text_chars: Vec<char> = text.chars().collect();
    
    while current_pos < text_chars.len() {
        // Look for opening tag
        if text_chars[current_pos] == '<' && current_pos + 1 < text_chars.len() && text_chars[current_pos + 1] != '/' {
            let tag_start = current_pos;
            
            // Find the end of the opening tag
            let mut tag_end = current_pos + 1;
            while tag_end < text_chars.len() && text_chars[tag_end] != '>' && !text_chars[tag_end].is_whitespace() {
                tag_end += 1;
            }
            
            if tag_end < text_chars.len() && text_chars[tag_end] == '>' {
                // Extract tag name
                let tag_name: String = text_chars[(tag_start + 1)..tag_end].iter().collect();
                
                // Check if this looks like a tool name (alphanumeric + underscore)
                // and optionally matches known tools
                if tag_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    // Look for the closing tag
                    let closing_tag = format!("</{}>", tag_name);
                    let remaining: String = text_chars[tag_end + 1..].iter().collect();
                    
                    if let Some(close_pos) = remaining.find(&closing_tag) {
                        // Extract content between tags
                        let content: String = text_chars[(tag_end + 1)..(tag_end + 1 + close_pos)].iter().collect();
                        
                        // Add text before the tool call
                        if tag_start > current_pos {
                            let before_text: String = text_chars[current_pos..tag_start].iter().collect();
                            remaining_text.push_str(&before_text);
                        }
                        
                        // Parse parameters from content
                        let params = parse_tool_parameters(&content);
                        
                        // Create tool call
                        let tool_call_id = Uuid::new_v4().to_string();
                        tool_calls.push(MessagePart::ToolCall {
                            tool_call_id,
                            name: tag_name,
                            parameters: serde_json::Value::Object(params),
                        });
                        
                        // Move position after the closing tag
                        current_pos = tag_end + 1 + close_pos + closing_tag.len();
                        continue;
                    }
                }
            }
        }
        
        // Not a tool call, add as regular text
        remaining_text.push(text_chars[current_pos]);
        current_pos += 1;
    }
    
    (remaining_text.trim().to_string(), tool_calls)
}

/// Parse parameters from tool content
fn parse_tool_parameters(content: &str) -> serde_json::Map<String, serde_json::Value> {
    let mut params = serde_json::Map::new();
    let mut pos = 0;
    let chars: Vec<char> = content.chars().collect();
    
    while pos < chars.len() {
        // Skip whitespace
        while pos < chars.len() && chars[pos].is_whitespace() {
            pos += 1;
        }
        
        if pos >= chars.len() {
            break;
        }
        
        // Look for parameter opening tag
        if chars[pos] == '<' && pos + 1 < chars.len() && chars[pos + 1] != '/' {
            let tag_start = pos;
            let mut tag_end = pos + 1;
            
            // Find tag name end
            while tag_end < chars.len() && chars[tag_end] != '>' && !chars[tag_end].is_whitespace() {
                tag_end += 1;
            }
            
            if tag_end < chars.len() && chars[tag_end] == '>' {
                let param_name: String = chars[(tag_start + 1)..tag_end].iter().collect();
                
                if param_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    let closing_tag = format!("</{}>", param_name);
                    let remaining: String = chars[(tag_end + 1)..].iter().collect();
                    
                    if let Some(close_pos) = remaining.find(&closing_tag) {
                        let param_value: String = chars[(tag_end + 1)..(tag_end + 1 + close_pos)].iter().collect();
                        
                        // Clean the value (remove comments, trim whitespace)
                        let param_value = if let Some(comment_pos) = param_value.find("<!--") {
                            param_value[..comment_pos].trim()
                        } else {
                            param_value.trim()
                        };
                        
                        // Parse the value into appropriate JSON type
                        let json_value = parse_json_value(param_value);
                        params.insert(param_name, json_value);
                        
                        pos = tag_end + 1 + close_pos + closing_tag.len();
                        continue;
                    }
                }
            }
        }
        
        pos += 1;
    }
    
    params
}

/// Parse a string value into appropriate JSON type
fn parse_json_value(value: &str) -> serde_json::Value {
    // Boolean
    if value == "true" || value == "false" {
        return serde_json::Value::Bool(value == "true");
    }
    
    // Number
    if let Ok(num) = value.parse::<i64>() {
        return serde_json::Value::Number(num.into());
    }
    if let Ok(num) = value.parse::<f64>() {
        if let Some(json_num) = serde_json::Number::from_f64(num) {
            return serde_json::Value::Number(json_num);
        }
    }
    
    // JSON object or array
    if (value.starts_with('{') && value.ends_with('}')) || 
       (value.starts_with('[') && value.ends_with(']')) {
        if let Ok(parsed) = serde_json::from_str(value) {
            return parsed;
        }
    }
    
    // Default to string
    serde_json::Value::String(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_simple_tool_call() {
        let text = "Let me search for that. <web_search><query>latest Bevy version</query></web_search> I'll check the results.";
        let (remaining, tools) = parse_tool_calls_from_text(text);
        
        assert_eq!(remaining, "Let me search for that.  I'll check the results.");
        assert_eq!(tools.len(), 1);
        
        if let MessagePart::ToolCall { name, parameters, .. } = &tools[0] {
            assert_eq!(name, "web_search");
            assert_eq!(parameters["query"], "latest Bevy version");
        } else {
            panic!("Expected tool call");
        }
    }
    
    #[test]
    fn test_parse_multiple_tools() {
        let text = "I'll help you. <read_file><path>/tmp/test.txt</path></read_file> And then <write_file><path>/tmp/out.txt</path><content>Hello</content></write_file>";
        let (remaining, tools) = parse_tool_calls_from_text(text);
        
        assert_eq!(remaining, "I'll help you.  And then");
        assert_eq!(tools.len(), 2);
        
        if let MessagePart::ToolCall { name, parameters, .. } = &tools[0] {
            assert_eq!(name, "read_file");
            assert_eq!(parameters["path"], "/tmp/test.txt");
        }
        
        if let MessagePart::ToolCall { name, parameters, .. } = &tools[1] {
            assert_eq!(name, "write_file");
            assert_eq!(parameters["path"], "/tmp/out.txt");
            assert_eq!(parameters["content"], "Hello");
        }
    }
    
    #[test]
    fn test_parse_with_types() {
        let text = "<test_tool><count>42</count><enabled>true</enabled><ratio>3.14</ratio></test_tool>";
        let (_, tools) = parse_tool_calls_from_text(text);
        
        assert_eq!(tools.len(), 1);
        if let MessagePart::ToolCall { parameters, .. } = &tools[0] {
            assert_eq!(parameters["count"], 42);
            assert_eq!(parameters["enabled"], true);
            assert_eq!(parameters["ratio"], 3.14);
        }
    }
}