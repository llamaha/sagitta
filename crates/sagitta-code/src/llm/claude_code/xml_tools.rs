use serde_json::{Value, json};
use regex::Regex;
use crate::llm::client::MessagePart;
use std::collections::HashMap;
use uuid::Uuid;

/// Parse XML-formatted tool calls from text content
pub fn parse_xml_tool_calls(text: &str) -> (String, Vec<MessagePart>) {
    log::debug!("XML_TOOLS: Parsing text for tool calls: {}", text);
    let mut remaining_text = text.to_string();
    let mut tool_calls = Vec::new();
    
    // Process each known tool type specifically to avoid matching nested parameter tags
    for tool_name in SAGITTA_TOOLS {
        // Create a regex specifically for this tool
        let tool_pattern = format!(r"<{0}>([\s\S]*?)</{0}>", regex::escape(tool_name));
        let tool_regex = Regex::new(&tool_pattern).unwrap();
        
        // Find all occurrences of this specific tool
        let mut replacements = Vec::new();
        
        for capture in tool_regex.captures_iter(&remaining_text) {
            let full_match = capture.get(0).unwrap();
            let tool_content = capture.get(1).unwrap().as_str();
            
            log::debug!("XML_TOOLS: Found tool '{}' with content: {}", tool_name, tool_content);
            
            // Parse parameters from the tool content
            let parameters = parse_tool_parameters(tool_content);
            
            log::debug!("XML_TOOLS: Parsed parameters for {}: {:?}", tool_name, parameters);
            
            // Create a tool call
            let tool_call = MessagePart::ToolCall {
                tool_call_id: uuid::Uuid::new_v4().to_string(),
                name: tool_name.to_string(),
                parameters,
            };
            
            tool_calls.push(tool_call);
            
            // Store the match for later removal
            replacements.push((full_match.start(), full_match.end()));
        }
        
        // Remove matches in reverse order to maintain correct positions
        for (start, end) in replacements.into_iter().rev() {
            remaining_text.replace_range(start..end, "");
        }
    }
    
    // Clean up any extra whitespace left behind
    remaining_text = clean_whitespace(&remaining_text);
    
    log::debug!("XML_TOOLS: Finished parsing. Found {} tool calls", tool_calls.len());
    
    (remaining_text, tool_calls)
}

const SAGITTA_TOOLS: &[&str] = &[
    // Repository tools
    "add_existing_repository",
    "sync_repository", 
    "remove_repository",
    "list_repositories",
    "search_file_in_repository",
    "view_file_in_repository",
    "repository_map",
    "targeted_view",
    
    // Code tools
    "search_code",
    "edit_file",
    "validate_edit",
    "semantic_edit",
    
    // Web search
    "web_search",
    
    // File system tools (if enabled)
    "shell_execution",
    "direct_file_read",
    "direct_file_edit",
    "get_current_directory",
    "change_directory",
    
    // Git tools (if enabled)
    "git_create_branch",
    "git_list_branches",
];

/// Parse parameters from XML content
fn parse_tool_parameters(content: &str) -> Value {
    let mut params = HashMap::new();
    
    log::debug!("XML_TOOLS: Parsing tool parameters from content: {}", content);
    
    // Match parameter tags like <param_name>value</param_name>
    let param_regex = Regex::new(r"<(\w+)>([\s\S]*?)</(\w+)>").unwrap();
    
    for capture in param_regex.captures_iter(content) {
        let opening_tag = capture.get(1).unwrap().as_str();
        let param_value = capture.get(2).unwrap().as_str().trim();
        let closing_tag = capture.get(3).unwrap().as_str();
        
        log::debug!("XML_TOOLS: Found parameter tag '{}' with value '{}'", opening_tag, param_value);
        
        // Verify opening and closing tags match
        if opening_tag != closing_tag {
            log::debug!("XML_TOOLS: Skipping mismatched tags: {} != {}", opening_tag, closing_tag);
            continue;
        }
        
        let param_name = opening_tag;
        
        // Try to parse the value as JSON first (for arrays and objects)
        let value = if let Ok(json_value) = serde_json::from_str::<Value>(param_value) {
            json_value
        } else {
            // Try to parse as number
            if let Ok(num) = param_value.parse::<f64>() {
                json!(num)
            } else if param_value == "true" || param_value == "false" {
                // Parse boolean
                json!(param_value == "true")
            } else {
                // Default to string
                json!(param_value)
            }
        };
        
        params.insert(param_name.to_string(), value);
    }
    
    // Check if there's any non-parameter content (for tools with single unnamed parameter)
    let remaining = param_regex.replace_all(content, "").trim().to_string();
    if !remaining.is_empty() && params.is_empty() {
        // Single parameter tool - use the content directly
        params.insert("content".to_string(), json!(remaining));
    }
    
    json!(params)
}


/// Check if a tag name is likely an HTML tag rather than a tool
fn is_html_tag(tag: &str) -> bool {
    const HTML_TAGS: &[&str] = &[
        "a", "abbr", "address", "area", "article", "aside", "audio", "b", "base", 
        "bdi", "bdo", "blockquote", "body", "br", "button", "canvas", "caption", 
        "cite", "code", "col", "colgroup", "data", "datalist", "dd", "del", "details", 
        "dfn", "dialog", "div", "dl", "dt", "em", "embed", "fieldset", "figcaption", 
        "figure", "footer", "form", "h1", "h2", "h3", "h4", "h5", "h6", "head", 
        "header", "hr", "html", "i", "iframe", "img", "input", "ins", "kbd", "label", 
        "legend", "li", "link", "main", "map", "mark", "meta", "meter", "nav", 
        "noscript", "object", "ol", "optgroup", "option", "output", "p", "param", 
        "picture", "pre", "progress", "q", "rp", "rt", "ruby", "s", "samp", "script", 
        "section", "select", "small", "source", "span", "strong", "style", "sub", 
        "summary", "sup", "svg", "table", "tbody", "td", "template", "textarea", 
        "tfoot", "th", "thead", "time", "title", "tr", "track", "u", "ul", "var", 
        "video", "wbr",
        // Common non-tool tags that Claude might output
        "explanation", "thinking", "planning", "note", "comment", "reasoning"
    ];
    
    HTML_TAGS.contains(&tag.to_lowercase().as_str())
}

/// Clean up whitespace left after removing tool calls
fn clean_whitespace(text: &str) -> String {
    // Replace multiple newlines with double newline
    let multi_newline = Regex::new(r"\n{3,}").unwrap();
    let text = multi_newline.replace_all(text, "\n\n");
    
    // Replace multiple spaces with single space
    let multi_space = Regex::new(r"  +").unwrap();
    let text = multi_space.replace_all(&text, " ");
    
    // Trim leading and trailing whitespace
    text.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_simple_tool_call() {
        let text = "Let me search for that. <search_code><query>async function</query><repository>my-repo</repository></search_code> I'll analyze the results.";
        
        let (remaining, tools) = parse_xml_tool_calls(text);
        
        assert_eq!(remaining, "Let me search for that. I'll analyze the results.");
        assert_eq!(tools.len(), 1);
        
        if let MessagePart::ToolCall { name, parameters, .. } = &tools[0] {
            assert_eq!(name, "search_code");
            assert_eq!(parameters["query"], json!("async function"));
            assert_eq!(parameters["repository"], json!("my-repo"));
        } else {
            panic!("Expected ToolCall");
        }
    }
    
    #[test]
    fn test_parse_multiple_tools() {
        let text = "First <tool1><param>value1</param></tool1> then <tool2><param>value2</param></tool2> done.";
        
        let (remaining, tools) = parse_xml_tool_calls(text);
        
        assert_eq!(remaining, "First then done.");
        assert_eq!(tools.len(), 2);
    }
    
    #[test]
    fn test_parse_web_search() {
        let text = r#"I'll help you search for the Tokio Rust repository.

<web_search>
<search_term>tokio rust async runtime github official repository</search_term>
<explanation>Finding the official Tokio Rust repository URL for cloning</explanation>
</web_search>

Let me analyze the results."#;
        
        let (remaining, tools) = parse_xml_tool_calls(text);
        
        println!("Remaining text: {}", remaining);
        println!("Tools found: {}", tools.len());
        
        assert_eq!(tools.len(), 1);
        
        if let MessagePart::ToolCall { name, parameters, .. } = &tools[0] {
            assert_eq!(name, "web_search");
            println!("Parameters: {:?}", parameters);
            assert_eq!(parameters["search_term"], json!("tokio rust async runtime github official repository"));
            assert_eq!(parameters["explanation"], json!("Finding the official Tokio Rust repository URL for cloning"));
        } else {
            panic!("Expected ToolCall");
        }
    }
    
    #[test]
    fn test_parse_nested_json() {
        let text = r#"<execute_command><command>ls -la</command><options>{"recursive": true, "hidden": true}</options></execute_command>"#;
        
        let (_, tools) = parse_xml_tool_calls(text);
        
        assert_eq!(tools.len(), 1);
        if let MessagePart::ToolCall { parameters, .. } = &tools[0] {
            assert_eq!(parameters["command"], json!("ls -la"));
            assert_eq!(parameters["options"], json!({"recursive": true, "hidden": true}));
        }
    }
    
    #[test]
    fn test_ignore_html_tags() {
        let text = "This is <b>bold</b> and <code>inline code</code> but <my_tool><param>value</param></my_tool> is a tool.";
        
        let (remaining, tools) = parse_xml_tool_calls(text);
        
        assert_eq!(tools.len(), 1);
        assert!(remaining.contains("<b>bold</b>"));
        assert!(remaining.contains("<code>inline code</code>"));
    }
}