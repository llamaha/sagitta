// src/syntax/javascript_tests.rs
#[cfg(test)]
mod tests {
    // Use super::... to access items from the parent syntax module
    use crate::parser::{CodeChunk, SyntaxParser};
    use crate::javascript::JavaScriptParser;
    use anyhow::Result;

    // Helper function to create a parser instance
    fn create_parser() -> JavaScriptParser {
        JavaScriptParser::new()
    }

    // Helper to assert chunk properties (allowing for slight whitespace variations)
    fn assert_chunk(
        chunk: &CodeChunk,
        expected_content_trimmed: &str,
        expected_start: usize,
        expected_end: usize,
        expected_element: &str,
    ) {
        assert_eq!(chunk.content.trim(), expected_content_trimmed.trim());
        assert_eq!(chunk.start_line, expected_start);
        assert_eq!(chunk.end_line, expected_end);
        assert_eq!(chunk.language, "javascript");
        assert_eq!(chunk.element_type, expected_element);
        assert!(!chunk.file_path.is_empty());
    }

    #[test]
    fn test_parse_simple_function() -> Result<()> {
        let code = r#"
function greet(name) {
  console.log(`Hello, ${name}!`);
}
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.js")?;

        assert_eq!(chunks.len(), 1, "Expected 1 chunk (function)");
        assert_chunk(&chunks[0], code, 2, 4, "function");
        Ok(())
    }

    #[test]
    fn test_parse_class_with_methods() -> Result<()> {
        let code = r#"
class Greeter {
  constructor(name) {
    this.name = name;
  }

  sayHi() {
    console.log(`Hi, ${this.name}`);
  }

  static info() {
    console.log("This is a Greeter class");
  }
}
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.js")?;

        // Expecting Class, constructor method, sayHi method, static info method
        assert_eq!(chunks.len(), 4);

        let class_chunk = chunks.iter().find(|c| c.element_type == "class").unwrap();
        assert_chunk(class_chunk, code, 2, 14, "class");

        let constructor_chunk = chunks.iter().find(|c| c.element_type == "method" && c.content.contains("constructor")).unwrap();
        assert_chunk(constructor_chunk, "constructor(name) {\n    this.name = name;\n  }", 3, 5, "method");

        let say_hi_chunk = chunks.iter().find(|c| c.element_type == "method" && c.content.contains("sayHi")).unwrap();
        assert_chunk(say_hi_chunk, "sayHi() {\n    console.log(`Hi, ${this.name}`);\n  }", 7, 9, "method");

        let info_chunk = chunks.iter().find(|c| c.element_type == "method" && c.content.contains("static info")).unwrap();
        assert_chunk(info_chunk, "static info() {\n    console.log(\"This is a Greeter class\");\n  }", 11, 13, "method");

        Ok(())
    }

    #[test]
    fn test_parse_arrow_function_variable() -> Result<()> {
        let code = r#"
const add = (a, b) => {
  return a + b;
};
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.js")?;

        assert_eq!(chunks.len(), 1, "Expected 1 chunk (arrow function assigned to const)");
        assert_chunk(&chunks[0], "(a, b) => {\n  return a + b;\n}", 2, 4, "function");
        Ok(())
    }

    #[test]
    fn test_parse_function_expression_variable() -> Result<()> {
        let code = r#"
let multiply = function(a, b) {
  return a * b;
};
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.js")?;

        assert_eq!(chunks.len(), 1, "Expected 1 chunk (function expression assigned to let)");
        assert_chunk(&chunks[0], "function(a, b) {\n  return a * b;\n}", 2, 4, "function");
        Ok(())
    }

     #[test]
    fn test_parse_arrow_function_assignment() -> Result<()> {
        let code = r#"
let divide;
divide = (a, b) => {
  if (b === 0) throw new Error("Division by zero");
  return a / b;
};
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.js")?;
        // Expecting 1 chunk (the arrow function itself)
        if chunks.len() != 1 {
             eprintln!("Arrow assignment test chunks: {:?}", chunks);
        }
        assert_eq!(chunks.len(), 1, "Expected 1 chunk (arrow function assignment)");
        assert_chunk(&chunks[0], "(a, b) => {\n  if (b === 0) throw new Error(\"Division by zero\");\n  return a / b;\n}", 3, 6, "function");
        Ok(())
    }

    #[test]
    fn test_parse_empty_input() -> Result<()> {
        let code = "";
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.js")?;
        assert!(chunks.is_empty());
        Ok(())
    }

    #[test]
    fn test_parse_comments_only() -> Result<()> {
        let code = r#"
// This is a comment
/*
Another comment
*/
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.js")?;
        // Expect fallback to whole file
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].element_type, "fallback_chunk_0");
        Ok(())
    }

     #[test]
    fn test_parse_iife() -> Result<()> {
        let code = r#"
(function() {
  console.log("IIFE executed!");
})();
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.js")?;
        // Expect fallback to whole file as IIFE isn't captured by the current query
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].element_type, "fallback_chunk_0");
        Ok(())
    }

} 