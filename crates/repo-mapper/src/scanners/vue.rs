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
    let method_pattern = Regex::new(r"([a-zA-Z0-9_]+)\s*\([^)]*\)\s*\{").unwrap();
    let computed_pattern = Regex::new(r"([a-zA-Z0-9_]+)\s*\(\)\s*\{").unwrap();
    let component_pattern = Regex::new(r#"name:\s*['"]([a-zA-Z0-9_]+)['"]"#).unwrap();
    let prop_pattern = Regex::new(r"([a-zA-Z0-9_]+):\s*\{").unwrap();

    if let Some(captures) = component_pattern.captures(line) {
        methods.push(MethodInfo {
            name: captures[1].to_string(),
            method_type: MethodType::VueComponent,
            params: String::new(),
            context: context.to_string(),
            docstring: docstring.clone(),
            calls: Vec::new(),
            line_number: Some(line_number),
        });
    } else if context.contains("computed") {
        if let Some(captures) = computed_pattern.captures(line) {
        let method_calls = extract_method_calls(context, max_calls);
            methods.push(MethodInfo {
                name: captures[1].to_string(),
                method_type: MethodType::VueComputed,
                params: String::new(),
                context: context.to_string(),
                docstring: docstring.clone(),
                calls: method_calls,
                line_number: Some(line_number),
            });
        }
    } else if context.contains("methods") {
        if let Some(captures) = method_pattern.captures(line) {
        let params = extract_params(line);
        let method_calls = extract_method_calls(context, max_calls);
            methods.push(MethodInfo {
                name: captures[1].to_string(),
                method_type: MethodType::VueMethod,
                params,
                context: context.to_string(),
                docstring: docstring.clone(),
                calls: method_calls,
                line_number: Some(line_number),
            });
        }
    } else if context.contains("props") {
        if let Some(captures) = prop_pattern.captures(line) {
            methods.push(MethodInfo {
                name: captures[1].to_string(),
                method_type: MethodType::VueProp,
                params: String::new(),
                context: context.to_string(),
                docstring,
                calls: Vec::new(),
                line_number: Some(line_number),
            });
        }
    }
}

fn extract_params(line: &str) -> String {
    if let Some(params) = line.find('(') {
        if let Some(end) = line[params..].find(')') {
            return line[params + 1..params + end].trim().to_string();
        }
    }
    String::new()
}

fn extract_method_calls(context: &str, max_calls: usize) -> Vec<String> {
    let mut calls = Vec::new();
    
    let method_patterns = [
        Regex::new(r"\b([a-zA-Z_][a-zA-Z0-9_]*)\(").unwrap(),
        Regex::new(r"\.([a-zA-Z_][a-zA-Z0-9_]*)\(").unwrap(),
    ];

    for pattern in &method_patterns {
        for cap in pattern.captures_iter(context) {
            if let Some(method_name) = cap.get(1) {
                calls.push(method_name.as_str().to_string());
            }
        }
    }

    calls.sort();
    calls.dedup();
    calls.truncate(max_calls);
    calls
} 