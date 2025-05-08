#[cfg(test)]
mod tests {
    // Use super::... to access items from the parent syntax module
    use crate::syntax::parser::{CodeChunk, SyntaxParser};
    use crate::syntax::typescript::TypeScriptParser;
    use anyhow::Result;

    // Helper function to create a parser instance
    fn create_parser() -> TypeScriptParser {
        TypeScriptParser::new()
    }

    // Re-use the assert_chunk helper from JS tests
    fn assert_chunk(
        chunk: &CodeChunk,
        expected_content_trimmed: &str,
        expected_start: usize,
        expected_end: usize,
        expected_element: &str,
        expected_language: &str, // Add language parameter
    ) {
        assert_eq!(chunk.content.trim(), expected_content_trimmed.trim());
        assert_eq!(chunk.start_line, expected_start);
        assert_eq!(chunk.end_line, expected_end);
        assert_eq!(chunk.language, expected_language);
        assert_eq!(chunk.element_type, expected_element);
        assert!(!chunk.file_path.is_empty());
    }

    #[test]
    fn test_parse_simple_function_ts() -> Result<()> {
        let code = r#"
function add(x: number, y: number): number {
  return x + y;
}
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.ts")?;

        assert_eq!(chunks.len(), 1);
        assert_chunk(&chunks[0], code, 2, 4, "function", "typescript");
        Ok(())
    }

    #[test]
    fn test_parse_class_with_methods_ts() -> Result<()> {
        let code = r#"
class Point {
  x: number;
  y: number;

  constructor(x: number, y: number) {
    this.x = x;
    this.y = y;
  }

  distance(other: Point): number {
    const dx = this.x - other.x;
    const dy = this.y - other.y;
    return Math.sqrt(dx * dx + dy * dy);
  }
}
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.ts")?;

        assert_eq!(chunks.len(), 3, "Expected Class, Constructor, Method");

        let class_chunk = chunks.iter().find(|c| c.element_type == "class").unwrap();
        assert_chunk(class_chunk, code, 2, 16, "class", "typescript");

        let constructor_chunk = chunks.iter().find(|c| c.element_type == "method" && c.content.contains("constructor")).unwrap();
        assert_chunk(constructor_chunk, "constructor(x: number, y: number) {\n    this.x = x;\n    this.y = y;\n  }", 6, 9, "method", "typescript");

        let distance_chunk = chunks.iter().find(|c| c.element_type == "method" && c.content.contains("distance")).unwrap();
        assert_chunk(distance_chunk, "distance(other: Point): number {\n    const dx = this.x - other.x;\n    const dy = this.y - other.y;\n    return Math.sqrt(dx * dx + dy * dy);\n  }", 11, 15, "method", "typescript");

        Ok(())
    }

    #[test]
    fn test_parse_interface() -> Result<()> {
        let code = r#"
interface Person {
  name: string;
  age?: number; // Optional property
}
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.ts")?;

        assert_eq!(chunks.len(), 1);
        assert_chunk(&chunks[0], code, 2, 5, "interface", "typescript");
        Ok(())
    }

    #[test]
    fn test_parse_enum() -> Result<()> {
        let code = r#"
enum Color {
  Red,
  Green,
  Blue,
}
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.ts")?;

        assert_eq!(chunks.len(), 1);
        assert_chunk(&chunks[0], code, 2, 6, "enum", "typescript");
        Ok(())
    }

    #[test]
    fn test_parse_type_alias() -> Result<()> {
        let code = r#"
type StringOrNumber = string | number;
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.ts")?;

        assert_eq!(chunks.len(), 1);
        assert_chunk(&chunks[0], code, 2, 2, "type", "typescript");
        Ok(())
    }

    #[test]
    fn test_parse_arrow_function_variable_ts() -> Result<()> {
        let code = r#"
const multiply = (a: number, b: number): number => {
  return a * b;
};
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.ts")?;

        assert_eq!(chunks.len(), 1);
        // Captures the arrow function itself
        assert_chunk(&chunks[0], "(a: number, b: number): number => {\n  return a * b;\n}", 2, 4, "function", "typescript");
        Ok(())
    }

    #[test]
    fn test_parse_empty_input_ts() -> Result<()> {
        let code = "";
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.ts")?;
        assert!(chunks.is_empty());
        Ok(())
    }

    #[test]
    fn test_parse_comments_only_ts() -> Result<()> {
        let code = r#"
// This is a TypeScript comment
/*
Another comment
*/
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.ts")?;
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].element_type, "file");
        assert_eq!(chunks[0].language, "typescript");
        Ok(())
    }
} 