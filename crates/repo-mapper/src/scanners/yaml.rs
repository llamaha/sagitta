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
    let definition_pattern = Regex::new(r"^([a-zA-Z0-9_-]+):\s*$").unwrap();
    let value_pattern = Regex::new(r"^([a-zA-Z0-9_-]+):\s*(.+)$").unwrap();
    let template_pattern = Regex::new(r"template:\s*([a-zA-Z0-9_-]+)").unwrap();

    if let Some(captures) = template_pattern.captures(line) {
        methods.push(MethodInfo {
            name: captures[1].to_string(),
            method_type: MethodType::YamlTemplate,
            params: String::new(),
            context: context.to_string(),
            docstring: docstring.clone(),
            calls: Vec::new(),
            line_number: Some(line_number),
        });
    } else if let Some(captures) = definition_pattern.captures(line) {
        methods.push(MethodInfo {
            name: captures[1].to_string(),
            method_type: MethodType::YamlDef,
            params: String::new(),
            context: context.to_string(),
            docstring: docstring.clone(),
            calls: Vec::new(),
            line_number: Some(line_number),
        });
    } else if let Some(captures) = value_pattern.captures(line) {
        methods.push(MethodInfo {
            name: captures[1].to_string(),
            method_type: MethodType::YamlValue,
            params: captures[2].trim().to_string(),
            context: context.to_string(),
            docstring,
            calls: Vec::new(),
            line_number: Some(line_number),
        });
    }
} 