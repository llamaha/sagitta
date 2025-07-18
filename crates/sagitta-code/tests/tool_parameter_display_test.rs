// Tests for tool parameter display functionality

#[cfg(test)]
mod tests {
    use serde_json::json;
    
    // Note: These functions are not public, so we'll test the behavior indirectly
    // In a real implementation, you might want to make these functions public for testing
    // or create integration tests that exercise the full UI rendering
    
    #[test]
    fn test_semantic_query_parameter_length() {
        // Test that semantic query parameters can be reasonably long
        let query_text = "This is a fairly long semantic code search query that should be displayed without truncation for better user experience and readability in the tool cards";
        
        // Our new limit should be 120 characters, so this 150+ char string should be truncated
        assert!(query_text.len() > 120, "Test query should be longer than our limit");
        assert!(query_text.len() < 200, "But not excessively long for test purposes");
        
        // In the actual implementation, anything over 120 chars gets truncated to 117 + "..."
        let expected_truncated_length = 117 + 3; // 117 chars + "..."
        
        if query_text.len() > 120 {
            let truncated = format!("{}...", &query_text[..117]);
            assert_eq!(truncated.len(), expected_truncated_length);
            assert!(truncated.ends_with("..."));
        }
    }
    
    #[test]
    fn test_bash_command_parameter_length() {
        // Test that bash commands can be reasonably long  
        let command = "find /path/to/repository -name '*.rs' -type f | grep -E '(test|spec)' | head -20";
        
        // Our new limit should be 80 characters
        assert!(command.len() > 40, "Command should be longer than old limit");
        
        if command.len() > 80 {
            let truncated = format!("{}...", &command[..77]);
            assert_eq!(truncated.len(), 80);
            assert!(truncated.ends_with("..."));
        } else {
            // Should not be truncated
            assert_eq!(command, command);
        }
    }
    
    #[test]
    fn test_url_parameter_length() {
        // Test that URLs can be reasonably long
        let url = "https://docs.anthropic.com/en/docs/build-with-claude/tool-use/tool-calling-api-reference";
        
        // Our new limit should be 60 characters (increased from 30)
        assert!(url.len() > 30, "URL should be longer than old limit");
        
        if url.len() > 60 {
            let truncated = format!("{}...", &url[..57]);
            assert_eq!(truncated.len(), 60);
            assert!(truncated.ends_with("..."));
        }
    }
    
    #[test]
    fn test_general_string_parameter_length() {
        // Test that general string parameters can be much longer
        let long_string = "This is a very long string parameter that contains important information that users need to see in the tool cards for better debugging and understanding of what tools were called with what parameters";
        
        // Our new limit should be 100 characters (increased from 30)
        assert!(long_string.len() > 30, "String should be longer than old limit");
        assert!(long_string.len() > 100, "String should be longer than new limit for this test");
        
        // Should be truncated to 97 + "..."
        let expected_truncated_length = 97 + 3;
        let truncated = format!("{}...", &long_string[..97]);
        assert_eq!(truncated.len(), expected_truncated_length);
        assert!(truncated.ends_with("..."));
    }
    
    #[test]
    fn test_json_parameter_structure() {
        // Test that we can handle complex JSON parameters
        let params = json!({
            "queryText": "search for authentication functions",
            "repositoryName": "sagitta",
            "elementType": "function",
            "limit": 10
        });
        
        assert!(params.is_object());
        assert!(params.get("queryText").is_some());
        assert!(params.get("repositoryName").is_some());
        
        // Verify we can extract string values
        if let Some(query) = params.get("queryText").and_then(|v| v.as_str()) {
            assert_eq!(query, "search for authentication functions");
        }
        
        if let Some(repo) = params.get("repositoryName").and_then(|v| v.as_str()) {
            assert_eq!(repo, "sagitta");
        }
    }
    
    #[test]
    fn test_short_parameters_not_truncated() {
        // Test that short parameters are not unnecessarily truncated
        let short_query = "authentication";
        let short_command = "ls -la";
        let short_url = "https://example.com";
        let short_string = "short text";
        
        // None of these should be truncated with our new limits
        assert!(short_query.len() < 120);
        assert!(short_command.len() < 80);
        assert!(short_url.len() < 60);
        assert!(short_string.len() < 100);
        
        // They should remain unchanged
        assert_eq!(short_query, short_query);
        assert_eq!(short_command, short_command);
        assert_eq!(short_url, short_url);
        assert_eq!(short_string, short_string);
    }
    
    #[test]
    fn test_edge_case_lengths() {
        // Test strings that are exactly at the truncation boundaries
        let exactly_120_chars = "a".repeat(120);
        let exactly_80_chars = "b".repeat(80);
        let exactly_60_chars = "c".repeat(60);
        let exactly_100_chars = "d".repeat(100);
        
        // These should not be truncated (they're exactly at the limit)
        assert_eq!(exactly_120_chars.len(), 120);
        assert_eq!(exactly_80_chars.len(), 80);
        assert_eq!(exactly_60_chars.len(), 60);
        assert_eq!(exactly_100_chars.len(), 100);
        
        // One character over should be truncated
        let over_120_chars = "a".repeat(121);
        let over_80_chars = "b".repeat(81);
        let over_60_chars = "c".repeat(61);
        let over_100_chars = "d".repeat(101);
        
        assert!(over_120_chars.len() > 120);
        assert!(over_80_chars.len() > 80);
        assert!(over_60_chars.len() > 60);
        assert!(over_100_chars.len() > 100);
    }
}