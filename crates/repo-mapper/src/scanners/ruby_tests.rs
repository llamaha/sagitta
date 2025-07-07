#[cfg(test)]
mod tests {
    use super::super::scan_line;
    use crate::types::MethodType;

    #[test]
    fn test_scan_instance_method() {
        let mut methods = Vec::new();
        scan_line(
            "  def calculate_total",
            "  def calculate_total\n    items.sum(&:price)\n  end",
            None,
            &mut methods,
            5,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "calculate_total");
        assert_eq!(methods[0].method_type, MethodType::RubyInstance);
        assert_eq!(methods[0].params, "");
        assert_eq!(methods[0].line_number, Some(5));
        assert!(methods[0].calls.contains(&"sum".to_string()));
    }

    #[test]
    fn test_scan_instance_method_with_params() {
        let mut methods = Vec::new();
        scan_line(
            "def process_data(input, options = {})",
            "def process_data(input, options = {})\n  validate(input)\n  transform(input, options)\nend",
            Some("Processes input data with options".to_string()),
            &mut methods,
            10,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "process_data");
        assert_eq!(methods[0].method_type, MethodType::RubyInstance);
        assert_eq!(methods[0].params, "input, options = {}");
        assert_eq!(methods[0].docstring, Some("Processes input data with options".to_string()));
        assert!(methods[0].calls.contains(&"validate".to_string()));
        assert!(methods[0].calls.contains(&"transform".to_string()));
    }

    #[test]
    fn test_scan_class_method() {
        let mut methods = Vec::new();
        scan_line(
            "  def self.create_from_json(json_data)",
            "  def self.create_from_json(json_data)\n    data = JSON.parse(json_data)\n    new(data)\n  end",
            None,
            &mut methods,
            15,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "create_from_json");
        assert_eq!(methods[0].method_type, MethodType::RubyClass);
        assert_eq!(methods[0].params, "json_data");
        assert!(methods[0].calls.contains(&"parse".to_string()));
        assert!(methods[0].calls.contains(&"new".to_string()));
    }

    #[test]
    fn test_scan_module() {
        let mut methods = Vec::new();
        scan_line(
            "module Helpers",
            "module Helpers\n  def format_name(name)\n    name.strip.capitalize\n  end\nend",
            Some("Helper module for formatting".to_string()),
            &mut methods,
            1,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "Helpers");
        assert_eq!(methods[0].method_type, MethodType::RubyModule);
        assert_eq!(methods[0].params, "");
        assert_eq!(methods[0].docstring, Some("Helper module for formatting".to_string()));
    }

    #[test]
    fn test_scan_method_with_keyword_params() {
        let mut methods = Vec::new();
        scan_line(
            "def initialize(name:, age:, email: nil)",
            "def initialize(name:, age:, email: nil)\n  @name = name\n  @age = age\n  @email = email\nend",
            None,
            &mut methods,
            20,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "initialize");
        assert_eq!(methods[0].params, "name:, age:, email: nil");
    }

    #[test]
    fn test_scan_method_with_splat_params() {
        let mut methods = Vec::new();
        scan_line(
            "  def log_messages(*messages, **options)",
            "  def log_messages(*messages, **options)\n    messages.each { |msg| logger.info(msg) }\n  end",
            None,
            &mut methods,
            25,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "log_messages");
        assert_eq!(methods[0].params, "*messages, **options");
        assert!(methods[0].calls.contains(&"each".to_string()));
        assert!(methods[0].calls.contains(&"info".to_string()));
    }

    #[test]
    fn test_scan_method_with_block_param() {
        let mut methods = Vec::new();
        scan_line(
            "def process_items(items, &block)",
            "def process_items(items, &block)\n  items.map(&block)\nend",
            None,
            &mut methods,
            30,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "process_items");
        assert_eq!(methods[0].params, "items, &block");
        assert!(methods[0].calls.contains(&"map".to_string()));
    }

    #[test]
    fn test_scan_private_method() {
        let mut methods = Vec::new();
        scan_line(
            "    def validate_input",
            "private\n    def validate_input\n      raise 'Invalid' unless valid?\n    end",
            None,
            &mut methods,
            35,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "validate_input");
        assert_eq!(methods[0].method_type, MethodType::RubyInstance);
    }

    #[test]
    fn test_scan_method_with_question_mark() {
        let mut methods = Vec::new();
        scan_line(
            "def valid?",
            "def valid?\n  !@errors.any?\nend",
            None,
            &mut methods,
            40,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "valid");
        assert_eq!(methods[0].method_type, MethodType::RubyInstance);
        assert!(methods[0].calls.contains(&"any".to_string()));
    }

    #[test]
    fn test_scan_method_with_exclamation() {
        let mut methods = Vec::new();
        scan_line(
            "  def save!",
            "  def save!\n    validate!\n    persist!\n  end",
            None,
            &mut methods,
            45,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "save");
        assert_eq!(methods[0].method_type, MethodType::RubyInstance);
    }

    #[test]
    fn test_extract_method_calls() {
        let mut methods = Vec::new();
        let context = r#"
        def complex_method
          data = fetch_data()
          processed = data.map { |item| item.transform }
          results = process(processed)
          logger.info("Done")
          send_notification(results)
        end
        "#;
        
        scan_line(
            "def complex_method",
            context,
            None,
            &mut methods,
            50,
            10,
        );
        
        // The regex patterns only match:
        // 1. Methods with parentheses: method_name(
        // 2. Methods after a dot: .method_name
        assert!(methods[0].calls.contains(&"fetch_data".to_string()));
        assert!(methods[0].calls.contains(&"map".to_string()));
        assert!(methods[0].calls.contains(&"transform".to_string()));
        assert!(methods[0].calls.contains(&"process".to_string()));
        assert!(methods[0].calls.contains(&"info".to_string()));
        assert!(methods[0].calls.contains(&"send_notification".to_string()));
    }

    #[test]
    fn test_extract_method_calls_deduplication() {
        let mut methods = Vec::new();
        let context = r#"
        def process
          validate()
          validate()
          save()
          validate()
        end
        "#;
        
        scan_line(
            "def process",
            context,
            None,
            &mut methods,
            60,
            10,
        );
        
        // validate should only appear once due to deduplication
        assert_eq!(methods[0].calls.iter().filter(|&c| c == "validate").count(), 1);
        assert!(methods[0].calls.contains(&"save".to_string()));
    }

    #[test]
    fn test_max_calls_limit() {
        let mut methods = Vec::new();
        let context = r#"
        def many_calls
          a(); b(); c(); d(); e()
          f(); g(); h(); i(); j()
        end
        "#;
        
        scan_line(
            "def many_calls",
            context,
            None,
            &mut methods,
            70,
            5, // max_calls = 5
        );
        
        assert_eq!(methods[0].calls.len(), 5);
    }

    #[test]
    fn test_no_match() {
        let mut methods = Vec::new();
        
        // Class definition
        scan_line(
            "class User",
            "class User\nend",
            None,
            &mut methods,
            80,
            10,
        );
        assert_eq!(methods.len(), 0);
        
        // Variable assignment
        scan_line(
            "  name = 'John'",
            "  name = 'John'",
            None,
            &mut methods,
            81,
            10,
        );
        assert_eq!(methods.len(), 0);
        
        // Comment
        scan_line(
            "# This is a comment",
            "# This is a comment",
            None,
            &mut methods,
            82,
            10,
        );
        assert_eq!(methods.len(), 0);
        
        // Require statement
        scan_line(
            "require 'json'",
            "require 'json'",
            None,
            &mut methods,
            83,
            10,
        );
        assert_eq!(methods.len(), 0);
    }

    #[test]
    fn test_nested_module() {
        let mut methods = Vec::new();
        scan_line(
            "  module InnerModule",
            "module OuterModule\n  module InnerModule\n  end\nend",
            None,
            &mut methods,
            90,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "InnerModule");
        assert_eq!(methods[0].method_type, MethodType::RubyModule);
    }

    #[test]
    fn test_method_without_parentheses() {
        let mut methods = Vec::new();
        scan_line(
            "def greet name",
            "def greet name\n  puts(\"Hello, #{name}\")\nend",
            None,
            &mut methods,
            95,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "greet");
        assert_eq!(methods[0].params, "");
        // puts is called with parentheses so it will be detected
        assert!(methods[0].calls.contains(&"puts".to_string()));
    }
}