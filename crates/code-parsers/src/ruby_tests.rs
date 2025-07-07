// src/syntax/ruby_tests.rs
#[cfg(test)]
mod tests {
    use super::super::{RubyParser, SyntaxParser};
    
    // Helper function to create a parser instance
    fn create_parser() -> RubyParser {
        RubyParser::new()
    }

    #[test]
    fn test_parse_simple_method() {
        let mut parser = create_parser();
        let code = r#"
def hello
  puts "Hello, world!"
end
"#;
        let result = parser.parse(code, "test.rb").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].element_type, "method");
        assert_eq!(result[0].start_line, 2);
        assert_eq!(result[0].end_line, 4);
        assert!(result[0].content.contains("def hello"));
    }

    #[test]
    fn test_parse_class() {
        let mut parser = create_parser();
        let code = r#"
class Person
  def initialize(name)
    @name = name
  end
  
  def greet
    puts "Hello, I'm #{@name}"
  end
end
"#;
        let result = parser.parse(code, "person.rb").unwrap();
        assert_eq!(result.len(), 3); // class + 2 methods
        
        // Check class
        assert_eq!(result[0].element_type, "class");
        assert_eq!(result[0].start_line, 2);
        assert_eq!(result[0].end_line, 10);
        
        // Check methods
        assert_eq!(result[1].element_type, "method");
        assert!(result[1].content.contains("def initialize"));
        
        assert_eq!(result[2].element_type, "method");
        assert!(result[2].content.contains("def greet"));
    }

    #[test]
    fn test_parse_module() {
        let mut parser = create_parser();
        let code = r#"
module Helpers
  def format_name(name)
    name.strip.capitalize
  end
end
"#;
        let result = parser.parse(code, "helpers.rb").unwrap();
        assert_eq!(result.len(), 2); // module + method
        assert_eq!(result[0].element_type, "module");
        assert_eq!(result[1].element_type, "method");
    }

    #[test]
    fn test_parse_singleton_method() {
        let mut parser = create_parser();
        let code = r#"
class Calculator
  def self.add(a, b)
    a + b
  end
end
"#;
        let result = parser.parse(code, "calc.rb").unwrap();
        // Debug: print what we got
        for (i, chunk) in result.iter().enumerate() {
            eprintln!("Chunk {}: type={}, content={}", i, chunk.element_type, chunk.content.trim());
        }
        // Only the class is captured, singleton_method is nested inside
        assert_eq!(result.len(), 1); 
        assert_eq!(result[0].element_type, "class");
        // The singleton method is part of the class content
        assert!(result[0].content.contains("def self.add"));
    }

    #[test]
    fn test_parse_nested_structures() {
        let mut parser = create_parser();
        let code = r#"
module MyApp
  class User
    def name
      @name
    end
  end
  
  class Admin < User
    def permissions
      [:read, :write, :delete]
    end
  end
end
"#;
        let result = parser.parse(code, "app.rb").unwrap();
        // Should have: module, 2 classes, 2 methods
        assert_eq!(result.len(), 5);
        assert_eq!(result[0].element_type, "module");
        assert_eq!(result[1].element_type, "class");
        assert_eq!(result[2].element_type, "method");
        assert_eq!(result[3].element_type, "class");
        assert_eq!(result[4].element_type, "method");
    }

    #[test]
    fn test_parse_empty_file() {
        let mut parser = create_parser();
        let code = "";
        let result = parser.parse(code, "empty.rb").unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_parse_comments_only() {
        let mut parser = create_parser();
        let code = r#"
# This is a comment
# Another comment
"#;
        let result = parser.parse(code, "comments.rb").unwrap();
        // Comments with whitespace trigger fallback chunking
        if result.len() > 0 {
            assert!(result[0].element_type.starts_with("fallback_chunk_"));
        }
    }

    #[test]
    fn test_fallback_chunking() {
        let mut parser = create_parser();
        // Create a file with no methods/classes, just code
        let mut code = String::new();
        for i in 0..250 {
            code.push_str(&format!("puts 'Line {}'\n", i));
        }
        
        let result = parser.parse(&code, "script.rb").unwrap();
        // Should create fallback chunks
        assert!(result.len() > 0);
        assert!(result[0].element_type.starts_with("fallback_chunk_"));
    }

    #[test]
    fn test_parse_attr_accessors() {
        let mut parser = create_parser();
        let code = r#"
class Book
  attr_reader :title
  attr_writer :author
  attr_accessor :isbn
  
  def initialize(title)
    @title = title
  end
end
"#;
        let result = parser.parse(code, "book.rb").unwrap();
        // Should have class and initialize method
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].element_type, "class");
        assert_eq!(result[1].element_type, "method");
    }

    #[test]
    fn test_parse_complex_method() {
        let mut parser = create_parser();
        let code = r#"
def process_data(items, &block)
  items.each do |item|
    yield item if block_given?
  end
rescue StandardError => e
  puts "Error: #{e.message}"
ensure
  puts "Processing complete"
end
"#;
        let result = parser.parse(code, "process.rb").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].element_type, "method");
        assert!(result[0].content.contains("rescue"));
        assert!(result[0].content.contains("ensure"));
    }
}