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
    let instance_method_pattern = Regex::new(r"^\s*def\s+([a-zA-Z0-9_]+)").unwrap();
    let class_method_pattern = Regex::new(r"^\s*def\s+self\.([a-zA-Z0-9_]+)").unwrap();
    let module_pattern = Regex::new(r"^\s*module\s+([a-zA-Z0-9_]+)").unwrap();

    if let Some(captures) = class_method_pattern.captures(line) {
        let params = extract_params(line);
        let method_calls = extract_method_calls(context, max_calls);
        methods.push(MethodInfo {
            name: captures[1].to_string(),
            method_type: MethodType::RubyClass,
            params,
            context: context.to_string(),
            docstring: docstring.clone(),
            calls: method_calls,
            line_number: Some(line_number),
        });
    } else if let Some(captures) = instance_method_pattern.captures(line) {
        let params = extract_params(line);
        let method_calls = extract_method_calls(context, max_calls);
        methods.push(MethodInfo {
            name: captures[1].to_string(),
            method_type: MethodType::RubyInstance,
            params,
            context: context.to_string(),
            docstring: docstring.clone(),
            calls: method_calls,
            line_number: Some(line_number),
        });
    } else if let Some(captures) = module_pattern.captures(line) {
        let method_calls = extract_method_calls(context, max_calls);
        methods.push(MethodInfo {
            name: captures[1].to_string(),
            method_type: MethodType::RubyModule,
            params: String::new(),
            context: context.to_string(),
            docstring,
            calls: method_calls,
            line_number: Some(line_number),
        });
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
        Regex::new(r"\.([a-zA-Z_][a-zA-Z0-9_]*)\b").unwrap(),
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

#[cfg(test)]
#[path = "ruby_tests.rs"]
mod tests; 