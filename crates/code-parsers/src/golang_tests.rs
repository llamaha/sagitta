// src/syntax/golang_tests.rs
#[cfg(test)]
mod tests {
    // Use super::... to access items from the parent syntax module
    use crate::parser::{CodeChunk, SyntaxParser};
    use crate::golang::GolangParser;
    use anyhow::Result;

    // Helper function to create a parser instance
    fn create_parser() -> GolangParser {
        GolangParser::new()
    }

    // Helper to assert chunk properties
    fn assert_chunk(
        chunk: &CodeChunk,
        expected_content: &str,
        expected_start: usize,
        expected_end: usize,
        expected_element: &str,
    ) {
        assert_eq!(chunk.content.trim(), expected_content.trim());
        assert_eq!(chunk.start_line, expected_start);
        assert_eq!(chunk.end_line, expected_end);
        assert_eq!(chunk.language, "go");
        assert_eq!(chunk.element_type, expected_element);
        assert!(!chunk.file_path.is_empty());
    }

    #[test]
    fn test_parse_simple_function() -> Result<()> {
        let code = r#"
package main

func greet(name string) {
	println("Hello,", name)
}
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.go")?;

        assert_eq!(chunks.len(), 1);
        assert_chunk(&chunks[0], "func greet(name string) {\n\tprintln(\"Hello,\", name)\n}", 4, 6, "function");
        Ok(())
    }

    #[test]
    fn test_parse_struct_and_method() -> Result<()> {
        let code = r#"
package main

type Point struct {
	X, Y int
}

func (p *Point) Dist() int {
	return p.X*p.X + p.Y*p.Y // Simplified
}
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.go")?;

        // Expecting Type (struct) and Method
        assert_eq!(chunks.len(), 2);

        let type_chunk = chunks.iter().find(|c| c.element_type == "type").unwrap();
        assert_chunk(type_chunk, "type Point struct {\n\tX, Y int\n}", 4, 6, "type");

        let method_chunk = chunks.iter().find(|c| c.element_type == "method").unwrap();
        assert_chunk(method_chunk, "func (p *Point) Dist() int {\n\treturn p.X*p.X + p.Y*p.Y // Simplified\n}", 8, 10, "method");

        Ok(())
    }

    #[test]
    fn test_parse_interface() -> Result<()> {
        let code = r#"
package main

type Shape interface {
    Area() float64
}
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.go")?;

        assert_eq!(chunks.len(), 1);
        assert_chunk(&chunks[0], "type Shape interface {\n    Area() float64\n}", 4, 6, "type");
        Ok(())
    }

    #[test]
    fn test_parse_const_and_var() -> Result<()> {
        let code = r#"
package main

const Pi = 3.14

var GlobalCounter int = 0
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.go")?;

        // Const is a core element type, but var is not, so we expect only the const
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].element_type, "const");

        Ok(())
    }

    #[test]
    fn test_parse_empty_input() -> Result<()> {
        let code = "";
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.go")?;
        assert!(chunks.is_empty());
        Ok(())
    }

    #[test]
    fn test_parse_comments_only() -> Result<()> {
        let code = r#"
// This is a comment
/* Another comment */
package main // Need package for it to be valid Go
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.go")?;
        // Should fallback to file chunk as no declarations are found
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].element_type, "fallback_chunk_0");
        Ok(())
    }

    #[test]
    fn test_fallback_chunking_large_file() -> Result<()> {
        // Generate a large string with lines but no top-level declarations
        // that the main query would catch.
        let mut large_code_lines: Vec<String> = vec![
            String::from("package main"),
            String::from("// Start comments")
        ];
        for i in 0..500 { // Generate 500 lines of comments
            large_code_lines.push(format!("// Line {}", i));
        }
        let code = large_code_lines.join("\n");

        let mut parser = create_parser();
        let chunks = parser.parse(&code, "large_file.go")?;

        // Expected number of chunks based on FALLBACK_CHUNK_SIZE = 200
        // 502 lines total (package + 500 comments + initial comment)
        // ceil(502 / 200) = 3 chunks
        let expected_chunks = 3;
        assert_eq!(chunks.len(), expected_chunks, "Should split into multiple fallback chunks");

        // Check properties of the first chunk
        assert_eq!(chunks[0].element_type, "fallback_chunk_0");
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 200);
        assert!(chunks[0].content.starts_with("package main"));
        assert!(chunks[0].content.ends_with("// Line 197")); // 200 lines total, 0-indexed loop means last line is 197

        // Check properties of the last chunk
        let last_chunk_idx = expected_chunks - 1;
        assert_eq!(chunks[last_chunk_idx].element_type, "fallback_chunk_2");
        assert_eq!(chunks[last_chunk_idx].start_line, 401); // Chunk 1: 1-200, Chunk 2: 201-400, Chunk 3: 401-502
        assert_eq!(chunks[last_chunk_idx].end_line, 502); // Total 502 lines
        assert!(chunks[last_chunk_idx].content.starts_with("// Line 398")); // First line of third chunk
        assert!(chunks[last_chunk_idx].content.ends_with("// Line 499")); // Last line


        Ok(())
    }
} 