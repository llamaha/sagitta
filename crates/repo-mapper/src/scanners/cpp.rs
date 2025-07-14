use crate::types::{CodeMethod, MethodType};
use regex::Regex;
use std::collections::HashMap;

pub fn scan_cpp_methods(content: &str) -> Vec<CodeMethod> {
    let mut methods = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    
    // Regexes for different C++ constructs
    let function_regex = Regex::new(r"^(?:(?:inline|static|virtual|explicit|constexpr|template\s*<[^>]*>)\s+)*(?:[a-zA-Z_][a-zA-Z0-9_]*(?:\s*<[^>]*>)?\s+)?([a-zA-Z_][a-zA-Z0-9_]*)\s*\([^)]*\)\s*(?:const\s*)?(?:override\s*)?(?:final\s*)?(?:\s*->\s*[^{;]+)?[{;]").unwrap();
    let class_regex = Regex::new(r"^(?:template\s*<[^>]*>\s+)?class\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*[:{]").unwrap();
    let struct_regex = Regex::new(r"^(?:template\s*<[^>]*>\s+)?struct\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*[:{]").unwrap();
    let namespace_regex = Regex::new(r"^namespace\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\{").unwrap();
    let constructor_regex = Regex::new(r"^(?:(?:inline|explicit|constexpr)\s+)*([a-zA-Z_][a-zA-Z0-9_]*)\s*\([^)]*\)\s*[:{]").unwrap();
    let destructor_regex = Regex::new(r"^(?:(?:inline|virtual)\s+)*~([a-zA-Z_][a-zA-Z0-9_]*)\s*\([^)]*\)\s*[{;]").unwrap();
    let method_regex = Regex::new(r"^(?:(?:inline|static|virtual|constexpr)\s+)*(?:[a-zA-Z_][a-zA-Z0-9_]*(?:\s*<[^>]*>)?\s+)?([a-zA-Z_][a-zA-Z0-9_]*)\s*\([^)]*\)\s*(?:const\s*)?(?:override\s*)?(?:final\s*)?[{;]").unwrap();
    
    let mut current_class: Option<String> = None;
    let mut current_namespace: Option<String> = None;
    let mut brace_level = 0;
    let mut in_class = false;
    let mut in_namespace = false;
    
    for (line_no, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("/*") {
            continue;
        }
        
        // Track brace levels
        brace_level += line.matches('{').count() as i32;
        brace_level -= line.matches('}').count() as i32;
        
        // Check for namespace
        if let Some(captures) = namespace_regex.captures(trimmed) {
            let namespace_name = captures.get(1).unwrap().as_str().to_string();
            current_namespace = Some(namespace_name.clone());
            in_namespace = true;
            
            methods.push(CodeMethod {
                name: namespace_name,
                signature: extract_cpp_signature(line, line_no),
                description: extract_cpp_description(&lines, line_no),
                line_number: line_no + 1,
                method_type: MethodType::CppNamespace,
                parent_name: current_namespace.clone(),
                method_calls: extract_method_calls(line),
            });
            continue;
        }
        
        // Check for class
        if let Some(captures) = class_regex.captures(trimmed) {
            let class_name = captures.get(1).unwrap().as_str().to_string();
            current_class = Some(class_name.clone());
            in_class = true;
            
            methods.push(CodeMethod {
                name: class_name,
                signature: extract_cpp_signature(line, line_no),
                description: extract_cpp_description(&lines, line_no),
                line_number: line_no + 1,
                method_type: MethodType::CppClass,
                parent_name: current_namespace.clone(),
                method_calls: extract_method_calls(line),
            });
            continue;
        }
        
        // Check for struct
        if let Some(captures) = struct_regex.captures(trimmed) {
            let struct_name = captures.get(1).unwrap().as_str().to_string();
            current_class = Some(struct_name.clone());
            in_class = true;
            
            methods.push(CodeMethod {
                name: struct_name,
                signature: extract_cpp_signature(line, line_no),
                description: extract_cpp_description(&lines, line_no),
                line_number: line_no + 1,
                method_type: MethodType::CppStruct,
                parent_name: current_namespace.clone(),
                method_calls: extract_method_calls(line),
            });
            continue;
        }
        
        // Check for destructor
        if let Some(captures) = destructor_regex.captures(trimmed) {
            let destructor_name = format!("~{}", captures.get(1).unwrap().as_str());
            
            methods.push(CodeMethod {
                name: destructor_name,
                signature: extract_cpp_signature(line, line_no),
                description: extract_cpp_description(&lines, line_no),
                line_number: line_no + 1,
                method_type: MethodType::CppDestructor,
                parent_name: if in_class { current_class.clone() } else { current_namespace.clone() },
                method_calls: extract_method_calls(line),
            });
            continue;
        }
        
        // Check for constructor (inside class context)
        if in_class && current_class.is_some() {
            if let Some(captures) = constructor_regex.captures(trimmed) {
                let potential_constructor = captures.get(1).unwrap().as_str();
                if Some(potential_constructor) == current_class.as_deref() {
                    methods.push(CodeMethod {
                        name: potential_constructor.to_string(),
                        signature: extract_cpp_signature(line, line_no),
                        description: extract_cpp_description(&lines, line_no),
                        line_number: line_no + 1,
                        method_type: MethodType::CppConstructor,
                        parent_name: current_class.clone(),
                        method_calls: extract_method_calls(line),
                    });
                    continue;
                }
            }
        }
        
        // Check for methods (inside class)
        if in_class && method_regex.is_match(trimmed) {
            if let Some(captures) = method_regex.captures(trimmed) {
                let method_name = captures.get(1).unwrap().as_str().to_string();
                let method_type = if trimmed.contains("static") {
                    MethodType::CppStaticMethod
                } else if trimmed.contains("virtual") {
                    MethodType::CppVirtualMethod
                } else {
                    MethodType::CppMethod
                };
                
                methods.push(CodeMethod {
                    name: method_name,
                    signature: extract_cpp_signature(line, line_no),
                    description: extract_cpp_description(&lines, line_no),
                    line_number: line_no + 1,
                    method_type,
                    parent_name: current_class.clone(),
                    method_calls: extract_method_calls(line),
                });
            }
        }
        // Check for top-level functions
        else if !in_class && function_regex.is_match(trimmed) {
            if let Some(captures) = function_regex.captures(trimmed) {
                let function_name = captures.get(1).unwrap().as_str().to_string();
                
                // Skip common non-function patterns
                if !["if", "while", "for", "switch", "catch"].contains(&function_name.as_str()) {
                    methods.push(CodeMethod {
                        name: function_name,
                        signature: extract_cpp_signature(line, line_no),
                        description: extract_cpp_description(&lines, line_no),
                        line_number: line_no + 1,
                        method_type: MethodType::CppFunction,
                        parent_name: current_namespace.clone(),
                        method_calls: extract_method_calls(line),
                    });
                }
            }
        }
        
        // Reset class/namespace context when leaving scope
        if brace_level == 0 {
            current_class = None;
            current_namespace = None;
            in_class = false;
            in_namespace = false;
        } else if in_class && brace_level == 1 {
            in_class = false;
            current_class = None;
        } else if in_namespace && brace_level == 1 {
            in_namespace = false;
            current_namespace = None;
        }
    }
    
    methods
}

fn extract_cpp_signature(line: &str, _line_no: usize) -> String {
    // Clean up the signature
    let mut signature = line.trim().to_string();
    
    // Remove template declarations for cleaner signatures
    if signature.starts_with("template") {
        if let Some(pos) = signature.find('>') {
            signature = signature[pos+1..].trim().to_string();
        }
    }
    
    // Remove implementation details (everything after {)
    if let Some(pos) = signature.find('{') {
        signature = signature[..pos].trim().to_string();
    }
    
    // Clean up trailing semicolons
    signature = signature.trim_end_matches(';').trim().to_string();
    
    signature
}

fn extract_cpp_description(lines: &[&str], line_no: usize) -> Option<String> {
    let mut description_lines = Vec::new();
    
    // Look for Doxygen-style comments above the line
    let mut i = line_no;
    while i > 0 {
        i -= 1;
        let line = lines[i].trim();
        
        if line.is_empty() {
            continue;
        }
        
        // Check for various C++ comment styles
        if line.starts_with("///") {
            description_lines.insert(0, line.trim_start_matches("///").trim().to_string());
        } else if line.starts_with("/**") && line.ends_with("*/") {
            // Single line /** comment */
            let content = line.trim_start_matches("/**").trim_end_matches("*/").trim();
            description_lines.insert(0, content.to_string());
            break;
        } else if line.starts_with("/**") {
            // Multi-line /** comment start
            description_lines.insert(0, line.trim_start_matches("/**").trim().to_string());
            continue;
        } else if line.starts_with("*/") {
            // End of multi-line comment, stop here
            break;
        } else if line.starts_with("*") {
            // Continuation of multi-line comment
            description_lines.insert(0, line.trim_start_matches("*").trim().to_string());
        } else if line.starts_with("//") {
            description_lines.insert(0, line.trim_start_matches("//").trim().to_string());
        } else {
            // Not a comment, stop looking
            break;
        }
    }
    
    if description_lines.is_empty() {
        None
    } else {
        Some(description_lines.join(" ").trim().to_string())
    }
}

fn extract_method_calls(line: &str) -> Vec<String> {
    let mut calls = Vec::new();
    
    // Simple regex to find function calls (name followed by parentheses)
    let call_regex = Regex::new(r"([a-zA-Z_][a-zA-Z0-9_]*)\s*\(").unwrap();
    
    for captures in call_regex.captures_iter(line) {
        if let Some(name) = captures.get(1) {
            let call_name = name.as_str().to_string();
            // Filter out keywords and common patterns
            if !["if", "while", "for", "switch", "catch", "return", "sizeof", "static_cast", "dynamic_cast", "const_cast", "reinterpret_cast"].contains(&call_name.as_str()) {
                calls.push(call_name);
            }
        }
    }
    
    calls
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpp_function_scanning() {
        let content = r#"
// Simple function
int add(int a, int b) {
    return a + b;
}

/// Calculate factorial
/// Returns the factorial of n
int factorial(int n) {
    if (n <= 1) return 1;
    return n * factorial(n - 1);
}
"#;

        let methods = scan_cpp_methods(content);
        assert_eq!(methods.len(), 2);
        
        assert_eq!(methods[0].name, "add");
        assert_eq!(methods[0].method_type, MethodType::CppFunction);
        assert_eq!(methods[0].line_number, 3);
        
        assert_eq!(methods[1].name, "factorial");
        assert_eq!(methods[1].method_type, MethodType::CppFunction);
        assert_eq!(methods[1].description, Some("Calculate factorial Returns the factorial of n".to_string()));
    }

    #[test]
    fn test_cpp_class_scanning() {
        let content = r#"
/**
 * A simple calculator class
 * Provides basic arithmetic operations
 */
class Calculator {
public:
    /// Default constructor
    Calculator();
    
    /// Destructor
    ~Calculator();
    
    /// Add two numbers
    int add(int a, int b);
    
    /// Static utility method
    static bool isValid(int value);
    
    /// Virtual method for overriding
    virtual void reset();
};
"#;

        let methods = scan_cpp_methods(content);
        assert!(methods.len() >= 6);
        
        // Find the class
        let class_method = methods.iter().find(|m| m.name == "Calculator" && m.method_type == MethodType::CppClass).unwrap();
        assert_eq!(class_method.description, Some("A simple calculator class Provides basic arithmetic operations".to_string()));
        
        // Find constructor
        let constructor = methods.iter().find(|m| m.method_type == MethodType::CppConstructor).unwrap();
        assert_eq!(constructor.name, "Calculator");
        
        // Find destructor
        let destructor = methods.iter().find(|m| m.method_type == MethodType::CppDestructor).unwrap();
        assert_eq!(destructor.name, "~Calculator");
        
        // Find static method
        let static_method = methods.iter().find(|m| m.method_type == MethodType::CppStaticMethod).unwrap();
        assert_eq!(static_method.name, "isValid");
        
        // Find virtual method
        let virtual_method = methods.iter().find(|m| m.method_type == MethodType::CppVirtualMethod).unwrap();
        assert_eq!(virtual_method.name, "reset");
    }

    #[test]
    fn test_cpp_namespace_scanning() {
        let content = r#"
/// Math utilities namespace
namespace math {
    /// Calculate square
    int square(int x) {
        return x * x;
    }
    
    /// Nested namespace
    namespace advanced {
        double sqrt(double x);
    }
}
"#;

        let methods = scan_cpp_methods(content);
        
        // Find namespace
        let namespace = methods.iter().find(|m| m.method_type == MethodType::CppNamespace).unwrap();
        assert_eq!(namespace.name, "math");
        assert_eq!(namespace.description, Some("Math utilities namespace".to_string()));
        
        // Find function in namespace
        let square_func = methods.iter().find(|m| m.name == "square").unwrap();
        assert_eq!(square_func.parent_name, Some("math".to_string()));
    }
}