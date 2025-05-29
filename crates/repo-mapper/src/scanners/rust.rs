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
    let function_pattern = Regex::new(r"^\s*(?:pub\s+)?fn\s+([a-zA-Z0-9_]+)").unwrap();
    let impl_pattern = Regex::new(r"^\s*impl\s+(?:<[^>]+>\s+)?([a-zA-Z0-9_]+)").unwrap();
    let trait_pattern = Regex::new(r"^\s*(?:pub\s+)?trait\s+([a-zA-Z0-9_]+)").unwrap();
    let trait_method_pattern = Regex::new(r"^\s+fn\s+([a-zA-Z0-9_]+)\s*(?:<[^>]+>)?\s*\([^)]*\)").unwrap();

    if let Some(captures) = function_pattern.captures(line) {
        let params = extract_params(line);
        let method_calls = extract_method_calls(context, max_calls);
        methods.push(MethodInfo {
            name: captures[1].to_string(),
            method_type: MethodType::RustFn,
            params,
            context: context.to_string(),
            docstring: docstring.clone(),
            calls: method_calls,
            line_number: Some(line_number),
        });
    }
    if let Some(captures) = impl_pattern.captures(line) {
        let params = extract_params(line);
        let method_calls = Vec::new();
        methods.push(MethodInfo {
            name: captures[1].to_string(),
            method_type: MethodType::RustImpl,
            params,
            context: context.to_string(),
            docstring: docstring.clone(),
            calls: method_calls,
            line_number: Some(line_number),
        });
    }
    if let Some(captures) = trait_pattern.captures(line) {
        let params = extract_params(line);
        let method_calls = Vec::new();
        methods.push(MethodInfo {
            name: captures[1].to_string(),
            method_type: MethodType::RustTrait,
            params,
            context: context.to_string(),
            docstring: docstring.clone(),
            calls: method_calls,
            line_number: Some(line_number),
        });
    }
    if let Some(captures) = trait_method_pattern.captures(line) {
        let params = extract_params(line);
        let method_calls = Vec::new();
        methods.push(MethodInfo {
            name: captures[1].to_string(),
            method_type: MethodType::RustTraitMethod,
            params,
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