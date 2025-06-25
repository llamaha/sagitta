use crate::types::{MethodInfo, MethodType};
use regex::Regex;

pub fn scan_line(
    line: &str,
    context: &str,
    docstring: Option<String>,
    methods: &mut Vec<MethodInfo>,
    line_number: usize,
    max_calls: usize,
) {
    let function_pattern = Regex::new(r"^\s*def\s+([a-zA-Z0-9_]+)\s*\(").unwrap();
    let class_pattern = Regex::new(r"^\s*class\s+([a-zA-Z0-9_]+)").unwrap();
    let async_function_pattern = Regex::new(r"^\s*async\s+def\s+([a-zA-Z0-9_]+)\s*\(").unwrap();
    let _method_pattern = Regex::new(r"^\s+def\s+([a-zA-Z0-9_]+)\s*\(").unwrap();
    let _static_method_pattern = Regex::new(r"^\s+@staticmethod\s*\n\s*def\s+([a-zA-Z0-9_]+)\s*\(").unwrap();
    let _class_method_pattern = Regex::new(r"^\s+@classmethod\s*\n\s*def\s+([a-zA-Z0-9_]+)\s*\(").unwrap();

    // Check for async functions first (more specific)
    if let Some(captures) = async_function_pattern.captures(line) {
        let params = extract_params(line);
        let method_calls = extract_method_calls(context, max_calls);
        methods.push(MethodInfo {
            name: format!("async {}", &captures[1]),
            method_type: MethodType::PythonAsyncFunction,
            params,
            context: context.to_string(),
            docstring: docstring.clone(),
            calls: method_calls,
            line_number: Some(line_number),
        });
    }
    // Regular functions
    else if let Some(captures) = function_pattern.captures(line) {
        let params = extract_params(line);
        let method_calls = extract_method_calls(context, max_calls);
        
        // Determine if it's a method (indented) or top-level function
        let method_type = if line.starts_with("    ") || line.starts_with("\t") {
            // Check if it's a special method
            if context.contains("@staticmethod") {
                MethodType::PythonStaticMethod
            } else if context.contains("@classmethod") {
                MethodType::PythonClassMethod
            } else {
                MethodType::PythonMethod
            }
        } else {
            MethodType::PythonFunction
        };
        
        methods.push(MethodInfo {
            name: captures[1].to_string(),
            method_type,
            params,
            context: context.to_string(),
            docstring: docstring.clone(),
            calls: method_calls,
            line_number: Some(line_number),
        });
    }
    // Classes
    else if let Some(captures) = class_pattern.captures(line) {
        let method_calls = extract_method_calls(context, max_calls);
        methods.push(MethodInfo {
            name: captures[1].to_string(),
            method_type: MethodType::PythonClass,
            params: String::new(),
            context: context.to_string(),
            docstring: docstring.clone(),
            calls: method_calls,
            line_number: Some(line_number),
        });
    }
}

fn extract_params(line: &str) -> String {
    if let Some(params_start) = line.find('(') {
        if let Some(params_end) = line[params_start..].find(')') {
            let params = &line[params_start + 1..params_start + params_end];
            // Clean up common Python parameter patterns
            return params
                .split(',')
                .map(|p| p.trim())
                .filter(|p| !p.is_empty())
                .collect::<Vec<_>>()
                .join(", ");
        }
    }
    String::new()
}

fn extract_method_calls(context: &str, max_calls: usize) -> Vec<String> {
    let mut calls = Vec::new();
    
    let method_patterns = [
        // Function calls: function_name(
        Regex::new(r"\b([a-zA-Z_][a-zA-Z0-9_]*)\s*\(").unwrap(),
        // Method calls: object.method(
        Regex::new(r"\.([a-zA-Z_][a-zA-Z0-9_]*)\s*\(").unwrap(),
        // Self method calls: self.method(
        Regex::new(r"self\.([a-zA-Z_][a-zA-Z0-9_]*)\s*\(").unwrap(),
    ];

    for pattern in &method_patterns {
        for cap in pattern.captures_iter(context) {
            if let Some(method_name) = cap.get(1) {
                let name = method_name.as_str();
                // Filter out common Python keywords and built-ins
                if !matches!(name, "if" | "for" | "while" | "try" | "with" | "def" | "class" | "import" | "from" | "return" | "yield" | "raise" | "pass" | "break" | "continue") {
                    calls.push(name.to_string());
                }
            }
        }
    }

    calls.sort();
    calls.dedup();
    calls.truncate(max_calls);
    calls
} 