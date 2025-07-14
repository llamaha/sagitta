// crates/sagitta-mcp/src/code_intelligence.rs

use crate::mcp::types::CodeContextInfo;
use regex::Regex;
use std::collections::HashSet;

/// Analyzes code content to extract rich context information
pub fn extract_code_context(content: &str, element_type: &str, language: &str) -> Option<CodeContextInfo> {
    let mut context = CodeContextInfo {
        signature: None,
        parent_name: None,
        description: None,
        identifiers: Vec::new(),
    };

    let mut has_any_info = false;

    // Extract function/method signatures
    if let Some(signature) = extract_signature(content, element_type, language) {
        context.signature = Some(signature);
        has_any_info = true;
    }

    // Extract parent class/module names
    if let Some(parent) = extract_parent_name(content, language) {
        context.parent_name = Some(parent);
        has_any_info = true;
    }

    // Extract description from comments
    if let Some(description) = extract_description(content, language) {
        context.description = Some(description);
        has_any_info = true;
    }

    // Extract key identifiers
    let identifiers = extract_identifiers(content, language);
    if !identifiers.is_empty() {
        context.identifiers = identifiers;
        has_any_info = true;
    }

    if has_any_info {
        Some(context)
    } else {
        None
    }
}

/// Enhanced preview generation with intelligent line selection
pub fn generate_intelligent_preview(content: &str, element_type: &str, language: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    
    if lines.is_empty() {
        return "<empty content>".to_string();
    }

    // For functions, try to show the signature
    if element_type == "function" || element_type == "method" {
        if let Some(signature_line) = find_function_signature_line(&lines, language) {
            return truncate_line(signature_line, 120);
        }
    }

    // For classes/structs, show the declaration
    if element_type == "class" || element_type == "struct" {
        if let Some(decl_line) = find_class_declaration_line(&lines, language) {
            return truncate_line(decl_line, 120);
        }
    }

    // For other types, try to find the most meaningful line
    if let Some(meaningful_line) = find_most_meaningful_line(&lines, language) {
        return truncate_line(meaningful_line, 120);
    }

    // Fallback to first non-empty line
    for line in &lines {
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with("//") && !trimmed.starts_with('#') {
            return truncate_line(line, 120);
        }
    }

    truncate_line(lines[0], 120)
}

fn extract_signature(content: &str, element_type: &str, language: &str) -> Option<String> {
    match language {
        "rust" => extract_rust_signature(content, element_type),
        "python" => extract_python_signature(content, element_type),
        "javascript" | "typescript" => extract_js_signature(content, element_type),
        "go" => extract_go_signature(content, element_type),
        "java" => extract_java_signature(content, element_type),
        _ => None,
    }
}

fn extract_rust_signature(content: &str, element_type: &str) -> Option<String> {
    if element_type == "function" {
        // Match Rust function signatures
        let re = Regex::new(r"(?m)^[\s]*(?:pub\s+)?(?:async\s+)?fn\s+\w+[^{]*").ok()?;
        if let Some(m) = re.find(content) {
            return Some(m.as_str().trim().to_string());
        }
    } else if element_type == "struct" {
        let re = Regex::new(r"(?m)^[\s]*(?:pub\s+)?struct\s+\w+[^{]*").ok()?;
        if let Some(m) = re.find(content) {
            return Some(m.as_str().trim().to_string());
        }
    }
    None
}

fn extract_python_signature(content: &str, element_type: &str) -> Option<String> {
    if element_type == "function" || element_type == "method" {
        let re = Regex::new(r"(?m)^[\s]*(?:async\s+)?def\s+\w+\([^)]*\)(?:\s*->\s*[^:]+)?:").ok()?;
        if let Some(m) = re.find(content) {
            return Some(m.as_str().trim().replace(":", "").to_string());
        }
    } else if element_type == "class" {
        let re = Regex::new(r"(?m)^[\s]*class\s+\w+[^:]*:").ok()?;
        if let Some(m) = re.find(content) {
            return Some(m.as_str().trim().replace(":", "").to_string());
        }
    }
    None
}

fn extract_js_signature(content: &str, element_type: &str) -> Option<String> {
    if element_type == "function" || element_type == "method" {
        // Match various JS function patterns
        let patterns = [
            r"(?m)^[\s]*(?:export\s+)?(?:async\s+)?function\s+\w+\([^)]*\)",
            r"(?m)^[\s]*(?:const|let|var)\s+\w+\s*=\s*(?:async\s+)?\([^)]*\)\s*=>",
            r"(?m)^[\s]*\w+\s*:\s*(?:async\s+)?function\s*\([^)]*\)",
            r"(?m)^[\s]*(?:async\s+)?\w+\s*\([^)]*\)\s*{",
        ];
        
        for pattern in &patterns {
            if let Ok(re) = Regex::new(pattern) {
                if let Some(m) = re.find(content) {
                    return Some(m.as_str().trim().to_string());
                }
            }
        }
    }
    None
}

fn extract_go_signature(content: &str, element_type: &str) -> Option<String> {
    if element_type == "function" {
        let re = Regex::new(r"(?m)^[\s]*func\s+(?:\([^)]*\)\s+)?\w+\([^)]*\)(?:\s*[^{]+)?").ok()?;
        if let Some(m) = re.find(content) {
            return Some(m.as_str().trim().to_string());
        }
    }
    None
}

fn extract_java_signature(content: &str, element_type: &str) -> Option<String> {
    if element_type == "method" || element_type == "function" {
        let re = Regex::new(r"(?m)^[\s]*(?:public|private|protected)?\s*(?:static)?\s*\w+\s+\w+\s*\([^)]*\)").ok()?;
        if let Some(m) = re.find(content) {
            return Some(m.as_str().trim().to_string());
        }
    }
    None
}

fn extract_parent_name(content: &str, language: &str) -> Option<String> {
    // Look for class or module context clues in the content
    match language {
        "python" => {
            if let Ok(re) = Regex::new(r"class\s+(\w+)") {
                if let Some(cap) = re.captures(content) {
                    return Some(cap[1].to_string());
                }
            }
        }
        "rust" => {
            if let Ok(re) = Regex::new(r"impl\s+(?:\w+\s+for\s+)?(\w+)") {
                if let Some(cap) = re.captures(content) {
                    return Some(cap[1].to_string());
                }
            }
        }
        "javascript" | "typescript" => {
            if let Ok(re) = Regex::new(r"class\s+(\w+)") {
                if let Some(cap) = re.captures(content) {
                    return Some(cap[1].to_string());
                }
            }
        }
        _ => {}
    }
    None
}

fn extract_description(content: &str, language: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    
    match language {
        "rust" => {
            // Look for /// doc comments
            for line in &lines {
                let trimmed = line.trim();
                if trimmed.starts_with("///") {
                    let desc = trimmed.trim_start_matches("///").trim();
                    if !desc.is_empty() && desc.len() > 10 {
                        return Some(desc.to_string());
                    }
                }
            }
        }
        "python" => {
            // Look for docstrings
            for (i, line) in lines.iter().enumerate() {
                let trimmed = line.trim();
                if trimmed.starts_with("\"\"\"") || trimmed.starts_with("'''") {
                    if let Some(next_line) = lines.get(i + 1) {
                        let desc = next_line.trim();
                        if !desc.is_empty() && desc.len() > 10 {
                            return Some(desc.to_string());
                        }
                    }
                }
            }
        }
        "javascript" | "typescript" => {
            // Look for JSDoc comments
            for line in &lines {
                let trimmed = line.trim();
                if trimmed.starts_with("* ") && !trimmed.starts_with("* @") {
                    let desc = trimmed.trim_start_matches("* ").trim();
                    if !desc.is_empty() && desc.len() > 10 {
                        return Some(desc.to_string());
                    }
                }
            }
        }
        _ => {
            // Generic comment detection
            for line in &lines {
                let trimmed = line.trim();
                if trimmed.starts_with("//") {
                    let desc = trimmed.trim_start_matches("//").trim();
                    if !desc.is_empty() && desc.len() > 10 {
                        return Some(desc.to_string());
                    }
                }
            }
        }
    }
    None
}

fn extract_identifiers(content: &str, language: &str) -> Vec<String> {
    let mut identifiers = HashSet::new();
    
    // Extract variable names, function names, etc.
    let identifier_patterns = match language {
        "rust" => vec![
            r"\blet\s+(\w+)",
            r"\bfn\s+(\w+)",
            r"\bstruct\s+(\w+)",
            r"\benum\s+(\w+)",
        ],
        "python" => vec![
            r"\bdef\s+(\w+)",
            r"\bclass\s+(\w+)",
            r"(\w+)\s*=",
        ],
        "javascript" | "typescript" => vec![
            r"\bfunction\s+(\w+)",
            r"\bclass\s+(\w+)",
            r"\bconst\s+(\w+)",
            r"\blet\s+(\w+)",
            r"\bvar\s+(\w+)",
        ],
        _ => vec![r"\b([a-zA-Z_]\w+)\b"],
    };

    for pattern in identifier_patterns {
        if let Ok(re) = Regex::new(pattern) {
            for cap in re.captures_iter(content) {
                if let Some(name) = cap.get(1) {
                    let name_str = name.as_str();
                    // Filter out common keywords and short names
                    if name_str.len() > 2 && !is_common_keyword(name_str, language) {
                        identifiers.insert(name_str.to_string());
                    }
                }
            }
        }
    }

    let mut result: Vec<String> = identifiers.into_iter().collect();
    result.sort();
    result.truncate(5); // Limit to top 5 identifiers
    result
}

fn is_common_keyword(word: &str, language: &str) -> bool {
    let keywords: &[&str] = match language {
        "rust" => &["let", "mut", "fn", "pub", "use", "mod", "impl", "for", "if", "else", "match"],
        "python" => &["def", "class", "if", "else", "for", "while", "try", "except", "import", "from"],
        "javascript" | "typescript" => &["function", "var", "let", "const", "if", "else", "for", "while", "class"],
        _ => &["if", "else", "for", "while", "return", "true", "false", "null"],
    };
    keywords.contains(&word)
}

fn find_function_signature_line<'a>(lines: &'a [&str], language: &str) -> Option<&'a str> {
    for line in lines {
        let trimmed = line.trim();
        match language {
            "rust" => {
                if trimmed.starts_with("pub fn") || trimmed.starts_with("fn") || trimmed.starts_with("async fn") {
                    return Some(line);
                }
            }
            "python" => {
                if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
                    return Some(line);
                }
            }
            "javascript" | "typescript" => {
                if trimmed.starts_with("function ") || trimmed.contains("=> {") || trimmed.contains("function(") {
                    return Some(line);
                }
            }
            _ => {
                if trimmed.contains("(") && (trimmed.contains("function") || trimmed.contains("def") || trimmed.contains("fn")) {
                    return Some(line);
                }
            }
        }
    }
    None
}

fn find_class_declaration_line<'a>(lines: &'a [&str], _language: &str) -> Option<&'a str> {
    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with("class ") || trimmed.starts_with("struct ") || trimmed.starts_with("pub struct ") {
            return Some(line);
        }
    }
    None
}

fn find_most_meaningful_line<'a>(lines: &'a [&str], _language: &str) -> Option<&'a str> {
    // Prefer lines with certain keywords or patterns
    for line in lines {
        let trimmed = line.trim();
        if !trimmed.is_empty() && 
           !trimmed.starts_with("//") && 
           !trimmed.starts_with("#") &&
           !trimmed.starts_with("*") &&
           trimmed.len() > 10 &&
           (trimmed.contains("=") || trimmed.contains("(") || trimmed.contains(":")) {
            return Some(line);
        }
    }
    None
}

fn truncate_line(line: &str, max_length: usize) -> String {
    if line.len() > max_length {
        format!("{}...", &line[..max_length - 3])
    } else {
        line.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_function_signature_extraction() {
        let content = r#"
/// This is a test function for authentication
pub async fn authenticate_user(username: &str, password: &str) -> Result<User, AuthError> {
    // Implementation here
    Ok(User::new(username))
}
"#;
        
        let context = extract_code_context(content, "function", "rust").unwrap();
        assert!(context.signature.is_some());
        let signature = context.signature.unwrap();
        assert!(signature.contains("pub async fn authenticate_user"));
        assert!(context.description.is_some());
        assert_eq!(context.description.unwrap(), "This is a test function for authentication");
    }

    #[test]
    fn test_python_class_extraction() {
        let content = r#"
class DatabaseManager:
    """
    A comprehensive database management class for handling connections and queries.
    """
    def __init__(self, connection_string):
        self.connection = None
    
    def connect(self):
        # Implementation here
        pass
"#;
        
        let context = extract_code_context(content, "class", "python").unwrap();
        assert!(context.signature.is_some());
        let signature = context.signature.unwrap();
        assert!(signature.contains("class DatabaseManager"));
        assert!(context.description.is_some());
        assert!(context.description.unwrap().contains("comprehensive database management"));
    }

    #[test]
    fn test_javascript_function_extraction() {
        let content = r#"
/**
 * Handles user authentication with advanced security features
 */
export async function authenticateUser(username, password) {
    const user = await findUser(username);
    const validation = validateCredentials(user, password);
    return validation;
}
"#;
        
        let context = extract_code_context(content, "function", "javascript").unwrap();
        assert!(context.signature.is_some());
        let signature = context.signature.unwrap();
        assert!(signature.contains("export async function authenticateUser"));
        assert!(context.description.is_some());
        assert!(context.description.unwrap().contains("advanced security features"));
    }

    #[test]
    fn test_intelligent_preview_function() {
        let content = r#"
// Some comment
pub fn calculate_user_score(user_id: u64, metrics: &Vec<Metric>) -> f64 {
    let base_score = 100.0;
    let mut adjusted_score = base_score;
    // Complex calculation logic...
    adjusted_score
}
"#;
        
        let preview = generate_intelligent_preview(content, "function", "rust");
        assert!(preview.contains("pub fn calculate_user_score"));
        assert!(!preview.contains("Some comment"));
    }

    #[test]
    fn test_intelligent_preview_struct() {
        let content = r#"
/// User configuration structure
pub struct UserConfig {
    pub name: String,
    pub email: String,
    pub preferences: UserPreferences,
}
"#;
        
        let preview = generate_intelligent_preview(content, "struct", "rust");
        assert!(preview.contains("pub struct UserConfig"));
    }

    #[test]
    fn test_identifier_extraction() {
        let content = r#"
fn process_user_data(user_data: UserData) -> ProcessedResult {
    let validation_engine = ValidationEngine::new();
    let processed_metrics = user_data.process();
    validation_engine.validate(processed_metrics)
}
"#;
        
        let context = extract_code_context(content, "function", "rust").unwrap();
        assert!(!context.identifiers.is_empty());
        assert!(context.identifiers.contains(&"validation_engine".to_string()));
        assert!(context.identifiers.contains(&"processed_metrics".to_string()));
    }

    #[test]
    fn test_parent_name_extraction_rust_impl() {
        let content = r#"
impl UserManager {
    pub fn create_user(&self, name: &str) -> User {
        User::new(name)
    }
}
"#;
        
        let context = extract_code_context(content, "method", "rust").unwrap();
        assert!(context.parent_name.is_some());
        assert_eq!(context.parent_name.unwrap(), "UserManager");
    }

    #[test]
    fn test_no_context_extraction() {
        let content = "let x = 5;";
        let context = extract_code_context(content, "unknown", "rust");
        assert!(context.is_none());
    }

    #[test]
    fn test_preview_fallback_to_meaningful_line() {
        let content = r#"
// Just a comment
/* Another comment */
const important_config = {
    database: 'production',
    timeout: 5000
};
"#;
        
        let preview = generate_intelligent_preview(content, "unknown", "javascript");
        assert!(preview.contains("const important_config"));
        assert!(!preview.contains("Just a comment"));
    }

    #[test]
    fn test_truncate_line_functionality() {
        let long_line = "a".repeat(150);
        let truncated = truncate_line(&long_line, 120);
        assert_eq!(truncated.len(), 120);
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_go_function_signature() {
        let content = r#"
func (s *UserService) AuthenticateUser(username, password string) (*User, error) {
    // Implementation
    return nil, nil
}
"#;
        
        let context = extract_code_context(content, "function", "go").unwrap();
        assert!(context.signature.is_some());
        let signature = context.signature.unwrap();
        assert!(signature.contains("func (s *UserService) AuthenticateUser"));
    }
}