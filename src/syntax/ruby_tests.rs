// src/syntax/ruby_tests.rs
#[cfg(test)]
mod tests {
    use crate::syntax::parser::{CodeChunk, SyntaxParser};
    use crate::syntax::ruby::RubyParser;
    use anyhow::Result;

    // Helper function to create a parser instance
    fn create_parser() -> RubyParser {
        RubyParser::new()
    }

    // Helper to assert chunk properties
    fn assert_chunk(
        chunk: &CodeChunk,
        expected_content: &str,
        expected_start: usize,
        expected_end: usize,
        expected_element: &str,
    ) {
        // Trim whitespace for comparison as tree-sitter might include surrounding whitespace
        assert_eq!(chunk.content.trim(), expected_content.trim());
        assert_eq!(chunk.start_line, expected_start);
        assert_eq!(chunk.end_line, expected_end);
        assert_eq!(chunk.language, "ruby");
        assert_eq!(chunk.element_type, expected_element);
        assert!(!chunk.file_path.is_empty());
    }

    #[test]
    fn test_parse_simple_method() -> Result<()> {
        let code = r#"
def greet(name)
  puts "Hello, #{name}!"
end
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.rb")?;

        // Expect 1 chunk (the method)
        if chunks.len() != 1 {
            eprintln!("Unexpected number of chunks in simple_method: {}. Chunks: {:?}", chunks.len(), chunks);
        }
        assert_eq!(chunks.len(), 1, "Expected 1 chunk (method)");
        // Find and assert the actual method chunk
        let method_chunk = chunks.iter().find(|c| c.element_type == "method").unwrap();
        assert_chunk(method_chunk, code, 2, 4, "method");
        Ok(())
    }

    #[test]
    fn test_parse_class_with_methods() -> Result<()> {
        let code = r#"
class Greeter
  def initialize(name)
    @name = name
  end

  def say_hi
    puts "Hi, #{@name}"
  end

  def self.info
    puts "This is a Greeter class"
  end
end
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.rb")?;

        // Expecting Class, initialize method, say_hi method, self.info singleton_method
        assert_eq!(chunks.len(), 4);

        let class_chunk = chunks.iter().find(|c| c.element_type == "class").unwrap();
        assert_chunk(class_chunk, code, 2, 14, "class");

        let init_chunk = chunks.iter().find(|c| c.element_type == "method" && c.content.contains("initialize")).unwrap();
        assert_chunk(init_chunk, "def initialize(name)\n    @name = name\n  end", 3, 5, "method");

        let say_hi_chunk = chunks.iter().find(|c| c.element_type == "method" && c.content.contains("say_hi")).unwrap();
        assert_chunk(say_hi_chunk, "def say_hi\n    puts \"Hi, #{@name}\"\n  end", 7, 9, "method");

        let info_chunk = chunks.iter().find(|c| c.element_type == "singleton_method").unwrap();
        assert_chunk(info_chunk, "def self.info\n    puts \"This is a Greeter class\"\n  end", 11, 13, "singleton_method");

        Ok(())
    }

    #[test]
    fn test_parse_module() -> Result<()> {
        let code = r#"
module MyUtils
  def self.utility_method
    # ...
  end
end
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.rb")?;

        // Expecting Module and singleton_method
        assert_eq!(chunks.len(), 2);

        let module_chunk = chunks.iter().find(|c| c.element_type == "module").unwrap();
        assert_chunk(module_chunk, code, 2, 6, "module");

        let method_chunk = chunks.iter().find(|c| c.element_type == "singleton_method").unwrap();
        assert_chunk(method_chunk, "def self.utility_method\n    # ...\n  end", 3, 5, "singleton_method");

        Ok(())
    }

    #[test]
    fn test_parse_block() -> Result<()> {
        let code = r#"
[1, 2, 3].each do |num|
  puts num
end
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.rb")?;

        // Expecting 1 chunk (the whole file) due to fallback logic
        if chunks.len() != 1 || chunks[0].element_type != "file" {
             eprintln!("Unexpected chunks/type found in block test: {:?}", chunks);
        }
        assert_eq!(chunks.len(), 1, "Expected 1 chunk from fallback");
        assert_eq!(chunks[0].element_type, "file", "Expected fallback chunk type to be 'file'");
        // We don't need to assert content precisely for fallback file chunk
        Ok(())
    }

    #[test]
    fn test_parse_empty_input() -> Result<()> {
        let code = "";
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.rb")?;
        assert!(chunks.is_empty());
        Ok(())
    }

    #[test]
    fn test_parse_comments_only() -> Result<()> {
        let code = r#"
# This is a comment
=begin
Another comment
=end
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.rb")?;
        assert_eq!(chunks.len(), 1);
        Ok(())
    }
} 