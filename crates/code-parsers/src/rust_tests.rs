// src/syntax/rust_tests.rs
#[cfg(test)]
mod tests {
    // Use super::... to access items from the parent syntax module
    use crate::parser::{CodeChunk, SyntaxParser};
    use crate::rust::RustParser;
    use anyhow::Result;

    // Helper function to create a parser instance
    fn create_parser() -> RustParser {
        RustParser::new()
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
        assert_eq!(chunk.language, "rust");
        assert_eq!(chunk.element_type, expected_element);
        assert!(!chunk.file_path.is_empty()); // Just ensure file path is set
    }

    #[test]
    fn test_parse_simple_function() -> Result<()> {
        let code = r#"
        fn main() {
            println!("Hello, world!");
        }
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.rs")?;

        assert_eq!(chunks.len(), 1, "Expected a single chunk for the function");
        assert_chunk(&chunks[0], code.trim(), 2, 4, "function");
        Ok(())
    }

    #[test]
    fn test_parse_struct_and_impl() -> Result<()> {
        let code = r#"
        struct Point {
            x: i32,
            y: i32,
        }

        impl Point {
            fn origin() -> Self {
                Point { x: 0, y: 0 }
            }
        }
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.rs")?;

        assert_eq!(chunks.len(), 1, "Expected struct and impl to be one chunk now?");
        assert!(chunks.iter().any(|c| c.element_type == "struct" || c.element_type == "impl" || c.element_type == "file_chunk"), "Chunk type mismatch");
        Ok(())
    }

    #[test]
    fn test_parse_enum_and_trait() -> Result<()> {
        let code = r#"
        enum Color {
            Red,
            Green,
            Blue,
        }

        trait Drawable {
            fn draw(&self);
        }
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.rs")?;

        assert_eq!(chunks.len(), 2);
        assert_chunk(
            chunks.iter().find(|c| c.element_type == "enum").unwrap(),
            "enum Color {\n            Red,\n            Green,\n            Blue,\n        }",
            2,
            6,
            "enum",
        );
        assert_chunk(
            chunks.iter().find(|c| c.element_type == "trait").unwrap(),
            "trait Drawable {\n            fn draw(&self);\n        }",
            8,
            10,
            "trait",
        );
        Ok(())
    }

    #[test]
    fn test_parse_module() -> Result<()> {
        let code = r#"
        mod my_module {
            const VALUE: i32 = 5;

            fn internal_func() {}
        }
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.rs")?;

        assert_eq!(chunks.len(), 1, "Expected a single chunk for the module");
        assert!(chunks.iter().any(|c| c.element_type == "module"));
        Ok(())
    }

    #[test]
    fn test_parse_use_and_const() -> Result<()> {
        let code = r#"
        use std::collections::HashMap;

        const MAX_SIZE: usize = 100;
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.rs")?;

        // Const is a core element type, but use statements are not, so we expect only the const
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].element_type, "const");
        Ok(())
    }

    #[test]
    fn test_parse_empty_input() -> Result<()> {
        let code = "";
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.rs")?;
        assert!(chunks.is_empty());
        Ok(())
    }

    #[test]
    fn test_parse_comments_only() -> Result<()> {
        let code = "// This is a comment\n// So is this";
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.rs")?;
        // Should fallback to file chunk(s) as no declarations are found
        // assert!(chunks.is_empty()); // Old assertion
        assert!(!chunks.is_empty(), "Fallback should produce at least one chunk");
        assert_eq!(chunks[0].element_type, "fallback_chunk_0", "Chunk type should be fallback_chunk_0");
        assert_eq!(chunks[0].language, "rust", "Language should be rust");
        Ok(())
    }

    #[test]
    fn test_parse_macros() -> Result<()> {
        let code = r#"
        macro_rules! my_macro {
            () => { println!("Macro called"); };
        }

        my_macro!(); // Invocation
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.rs")?;

        // Since macros are filtered out, we expect a fallback chunk
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].element_type, "fallback_chunk_0");
        Ok(())
    }

    #[test]
    fn test_parse_struct_with_doc_comment() -> Result<()> {
        let code = r#"
        /// This is a Point struct
        /// with two fields
        struct Point {
            x: i32,
            y: i32,
        }
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.rs")?;
        assert_eq!(chunks.len(), 1, "Expected a single chunk for the struct");
        let chunk = &chunks[0];
        if !chunk.content.contains("/// This is a Point struct") {
            println!("DEBUG: Actual chunk content: {}", chunk.content);
        }
        assert!(chunk.content.contains("/// This is a Point struct"), "Doc comment missing");
        assert!(chunk.content.contains("struct Point"), "Struct definition missing");
        Ok(())
    }

    #[test]
    fn test_parse_function_with_doc_comment() -> Result<()> {
        let code = r#"
        /// Adds two numbers
        /// Returns the sum
        fn add(a: i32, b: i32) -> i32 {
            a + b
        }
        "#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.rs")?;
        assert_eq!(chunks.len(), 1, "Expected a single chunk for the function");
        let chunk = &chunks[0];
        if !chunk.content.contains("/// Adds two numbers") {
            println!("DEBUG: Actual chunk content: {}", chunk.content);
        }
        assert!(chunk.content.contains("/// Adds two numbers"), "Doc comment missing");
        assert!(chunk.content.contains("fn add"), "Function definition missing");
        Ok(())
    }

    #[test]
    fn test_no_overlapping_chunks() -> Result<()> {
        let code = r#"
        // Test for overlapping chunks
        use std::collections::HashMap;

        /// A struct with documentation
        struct Point {
            x: i32,
            y: i32,
        }

        /// Implementation with methods
        impl Point {
            /// Constructor
            fn new(x: i32, y: i32) -> Self {
                Point { x, y }
            }
            
            /// Calculate distance
            fn distance(&self) -> f64 {
                ((self.x * self.x + self.y * self.y) as f64).sqrt()
            }
        }

        /// A trait definition
        trait Drawable {
            fn draw(&self);
        }

        /// Implementation of trait
        impl Drawable for Point {
            fn draw(&self) {
                println!("Point at ({}, {})", self.x, self.y);
            }
        }

        /// Module with nested items
        mod geometry {
            /// Nested struct
            struct Circle {
                radius: f64,
            }
            
            impl Circle {
                fn area(&self) -> f64 {
                    std::f64::consts::PI * self.radius * self.radius
                }
            }
        }

        /// Main function
        fn main() {
            let p = Point::new(3, 4);
            println!("Distance: {}", p.distance());
        }
        "#;
        
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.rs")?;
        
        // Check for overlaps
        let mut overlaps = Vec::new();
        for (i, chunk1) in chunks.iter().enumerate() {
            for (j, chunk2) in chunks.iter().enumerate().skip(i + 1) {
                // Check if chunks overlap (overlapping line ranges)
                if chunk1.start_line <= chunk2.end_line && chunk2.start_line <= chunk1.end_line {
                    overlaps.push((i, j));
                    println!("OVERLAP FOUND:");
                    println!("  Chunk {}: lines {}-{} ({})", i, chunk1.start_line, chunk1.end_line, chunk1.element_type);
                    println!("  Chunk {}: lines {}-{} ({})", j, chunk2.start_line, chunk2.end_line, chunk2.element_type);
                    println!("  Chunk {} content preview: {}", i, chunk1.content.lines().next().unwrap_or(""));
                    println!("  Chunk {} content preview: {}", j, chunk2.content.lines().next().unwrap_or(""));
                }
            }
        }
        
        assert!(overlaps.is_empty(), "Found {} overlapping chunks in Rust parser", overlaps.len());
        Ok(())
    }

} 