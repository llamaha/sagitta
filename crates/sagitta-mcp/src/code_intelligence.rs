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
    for line in lines {
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
    let keywords = match language {
        "rust" => &["let", "mut", "fn", "pub", "use", "mod", "impl", "for", "if", "else", "match"],
        "python" => &["def", "class", "if", "else", "for", "while", "try", "except", "import", "from"],
        "javascript" | "typescript" => &["function", "var", "let", "const", "if", "else", "for", "while", "class"],
        _ => &["if", "else", "for", "while", "return", "true", "false", "null"],
    };
    keywords.contains(&word)
}

fn find_function_signature_line(lines: &[&str], language: &str) -> Option<&str> {
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

fn find_class_declaration_line(lines: &[&str], language: &str) -> Option<&str> {
    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with("class ") || trimmed.starts_with("struct ") || trimmed.starts_with("pub struct ") {
            return Some(line);
        }
    }
    None
}

fn find_most_meaningful_line(lines: &[&str], _language: &str) -> Option<&str> {
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