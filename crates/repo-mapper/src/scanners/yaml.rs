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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_yaml_definition() {
        let mut methods = Vec::new();
        let line = "service:";
        let context = "service:\n  name: web-app\n  port: 8080";
        
        scan_line(line, context, None, &mut methods, 10, 10);
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "service");
        assert_eq!(methods[0].method_type, MethodType::YamlDef);
        assert_eq!(methods[0].params, "");
        assert_eq!(methods[0].line_number, Some(10));
        assert!(methods[0].calls.is_empty());
    }

    #[test]
    fn test_scan_yaml_value() {
        let mut methods = Vec::new();
        let line = "name: web-application";
        let context = "service:\n  name: web-application\n  version: 1.0.0";
        
        scan_line(line, context, None, &mut methods, 20, 10);
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "name");
        assert_eq!(methods[0].method_type, MethodType::YamlValue);
        assert_eq!(methods[0].params, "web-application");
        assert_eq!(methods[0].line_number, Some(20));
    }

    #[test]
    fn test_scan_yaml_template() {
        let mut methods = Vec::new();
        let line = "  template: deployment-template";
        let context = "deployment:\n  template: deployment-template\n  replicas: 3";
        
        scan_line(line, context, None, &mut methods, 30, 10);
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "deployment-template");
        assert_eq!(methods[0].method_type, MethodType::YamlTemplate);
        assert_eq!(methods[0].params, "");
        assert_eq!(methods[0].line_number, Some(30));
    }

    #[test]
    fn test_scan_with_docstring() {
        let mut methods = Vec::new();
        let line = "config:";
        let context = "# Main configuration\nconfig:\n  environment: production";
        let docstring = Some("Main configuration".to_string());
        
        scan_line(line, context, docstring, &mut methods, 40, 10);
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "config");
        assert_eq!(methods[0].docstring, Some("Main configuration".to_string()));
    }

    #[test]
    fn test_scan_yaml_value_with_spaces() {
        let mut methods = Vec::new();
        let line = "description: This is a multi-word description";
        let context = "metadata:\n  description: This is a multi-word description\n  author: John Doe";
        
        scan_line(line, context, None, &mut methods, 50, 10);
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "description");
        assert_eq!(methods[0].method_type, MethodType::YamlValue);
        assert_eq!(methods[0].params, "This is a multi-word description");
    }

    #[test]
    fn test_scan_yaml_value_with_special_chars() {
        let mut methods = Vec::new();
        let line = "url: https://example.com:8080/api";
        let context = "endpoints:\n  url: https://example.com:8080/api\n  timeout: 30";
        
        scan_line(line, context, None, &mut methods, 60, 10);
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "url");
        assert_eq!(methods[0].method_type, MethodType::YamlValue);
        assert_eq!(methods[0].params, "https://example.com:8080/api");
    }

    #[test]
    fn test_scan_yaml_numeric_value() {
        let mut methods = Vec::new();
        let line = "port: 8080";
        let context = "server:\n  port: 8080\n  host: localhost";
        
        scan_line(line, context, None, &mut methods, 70, 10);
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "port");
        assert_eq!(methods[0].method_type, MethodType::YamlValue);
        assert_eq!(methods[0].params, "8080");
    }

    #[test]
    fn test_scan_yaml_boolean_value() {
        let mut methods = Vec::new();
        let line = "enabled: true";
        let context = "feature:\n  enabled: true\n  debug: false";
        
        scan_line(line, context, None, &mut methods, 80, 10);
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "enabled");
        assert_eq!(methods[0].method_type, MethodType::YamlValue);
        assert_eq!(methods[0].params, "true");
    }

    #[test]
    fn test_non_matching_lines() {
        let mut methods = Vec::new();
        
        // Test various non-matching lines
        scan_line("# This is a comment", "", None, &mut methods, 1, 10);
        scan_line("- item in list", "", None, &mut methods, 2, 10);
        scan_line("  - nested item", "", None, &mut methods, 3, 10);
        scan_line("", "", None, &mut methods, 4, 10);
        scan_line("   ", "", None, &mut methods, 5, 10);
        
        assert!(methods.is_empty());
    }

    #[test]
    fn test_scan_with_hyphens_in_name() {
        let mut methods = Vec::new();
        let line = "my-service-name:";
        let context = "my-service-name:\n  type: microservice";
        
        scan_line(line, context, None, &mut methods, 90, 10);
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "my-service-name");
        assert_eq!(methods[0].method_type, MethodType::YamlDef);
    }

    #[test]
    fn test_scan_with_underscores_in_name() {
        let mut methods = Vec::new();
        let line = "env_variable: MY_ENV_VAR";
        let context = "environment:\n  env_variable: MY_ENV_VAR";
        
        scan_line(line, context, None, &mut methods, 100, 10);
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "env_variable");
        assert_eq!(methods[0].method_type, MethodType::YamlValue);
        assert_eq!(methods[0].params, "MY_ENV_VAR");
    }

    #[test]
    fn test_max_calls_parameter_ignored() {
        // The max_calls parameter is unused in YAML scanner
        let mut methods1 = Vec::new();
        let mut methods2 = Vec::new();
        let line = "test:";
        let context = "test:\n  value: 123";
        
        scan_line(line, context, None, &mut methods1, 110, 0);
        scan_line(line, context, None, &mut methods2, 110, 100);
        
        assert_eq!(methods1.len(), methods2.len());
        assert_eq!(methods1[0].name, methods2[0].name);
    }
} 