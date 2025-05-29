use crate::types::{MethodInfo, MethodType};
use regex::Regex;

pub fn scan_line(
    line: &str,
    context: &str,
    docstring: Option<String>,
    methods: &mut Vec<MethodInfo>,
    line_number: usize,
    _max_calls: usize,
) {
    let header_pattern = Regex::new(r"^(#{1,6})\s+(.+)$").unwrap();

    if let Some(captures) = header_pattern.captures(line) {
        let level = captures[1].len();
        let title = captures[2].trim().to_string();
        
        methods.push(MethodInfo {
            name: title,
            method_type: MethodType::MarkdownHeader,
            params: format!("level {}", level),
            context: context.to_string(),
            docstring,
            calls: Vec::new(),
            line_number: Some(line_number),
        });
    }
} 