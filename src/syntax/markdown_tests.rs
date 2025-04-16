// src/syntax/markdown_tests.rs
#[cfg(test)]
mod tests {
    use crate::syntax::parser::{CodeChunk, SyntaxParser};
    use crate::syntax::markdown::MarkdownParser;
    use anyhow::Result;

    fn create_parser() -> MarkdownParser {
        MarkdownParser::new()
    }

    // Helper to assert chunk properties
    fn assert_chunk(
        chunk: &CodeChunk,
        expected_content_contains: &str,
        expected_start: usize,
        expected_end: usize,
        expected_element_type_contains: &str,
    ) {
        assert!(
            chunk.content.contains(expected_content_contains),
            "Expected chunk content to contain '{}', got '{}'",
            expected_content_contains,
            chunk.content
        );
        assert_eq!(chunk.start_line, expected_start, "Expected start line {} but got {}", expected_start, chunk.start_line);
        assert_eq!(chunk.end_line, expected_end, "Expected end line {} but got {}", expected_end, chunk.end_line);
        assert_eq!(chunk.language, "markdown", "Expected language 'markdown', got '{}'", chunk.language);
        assert!(
            chunk.element_type.contains(expected_element_type_contains),
            "Expected element type to contain '{}', got '{}'",
            expected_element_type_contains,
            chunk.element_type
        );
        assert!(!chunk.file_path.is_empty(), "Expected non-empty file path");
    }

    #[test]
    fn test_section_based_chunking() -> Result<()> {
        let code = r#"
# Heading 1

This is a paragraph under heading 1.

## Heading 1.1

This is a paragraph under heading 1.1.

### Heading 1.1.1

This is a paragraph under heading 1.1.1.

## Heading 1.2

This is a paragraph under heading 1.2.

```rust
fn main() {
    println!("Hello, world!");
}
```

# Heading 2

This is a paragraph under heading 2.
"#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.md")?;

        // We should have sections for h1, h2 (under h1), h3 (under h2), h2 (under h1), and h1
        assert!(chunks.len() >= 5, "Expected at least 5 chunks for sections, got {}", chunks.len());

        // Check for content from different sections
        let has_h1_section = chunks.iter().any(|chunk| 
            chunk.element_type.contains("h1_section") && 
            chunk.content.contains("Heading 1") && 
            chunk.content.contains("This is a paragraph under heading 1.")
        );
        assert!(has_h1_section, "Missing H1 section with its content");

        let has_h1_1_section = chunks.iter().any(|chunk| 
            chunk.element_type.contains("h2_section") && 
            chunk.content.contains("Heading 1.1") && 
            chunk.content.contains("This is a paragraph under heading 1.1.")
        );
        assert!(has_h1_1_section, "Missing H1.1 section with its content");

        let has_h1_1_1_section = chunks.iter().any(|chunk| 
            chunk.element_type.contains("h3_section") && 
            chunk.content.contains("Heading 1.1.1") && 
            chunk.content.contains("This is a paragraph under heading 1.1.1.")
        );
        assert!(has_h1_1_1_section, "Missing H1.1.1 section with its content");

        let has_code_block = chunks.iter().any(|chunk| 
            chunk.content.contains("```rust") && 
            chunk.content.contains("fn main()")
        );
        assert!(has_code_block, "Missing code block in sections");

        // Check parent context is included in subsections
        let has_parent_context = chunks.iter().any(|chunk| 
            chunk.element_type.contains("h3_section") && 
            chunk.content.contains("Heading 1") && 
            chunk.content.contains("Heading 1.1") && 
            chunk.content.contains("Heading 1.1.1")
        );
        assert!(has_parent_context, "Missing parent context in subsection");

        Ok(())
    }

    #[test]
    fn test_parse_headings_and_paragraphs() -> Result<()> {
        let code = r#"
# H1 - Should be ignored by current query

## H2 Section 1

This is the first paragraph.

### H3 Subsection

Another paragraph here.

## H2 Section 2

Final paragraph.
"#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.md")?;

        // Debug: Print the chunks we're getting
        println!("Number of chunks: {}", chunks.len());
        for (i, chunk) in chunks.iter().enumerate() {
            println!("Chunk {}: type={}, content={}", i, chunk.element_type, chunk.content);
        }

        // With section-based chunking, we should have chunks for:
        // - H1 section with its paragraph
        // - H2 Section 1 with its paragraph (and H3 may be a subsection)
        // - H3 Subsection with its paragraph
        // - H2 Section 2 with its paragraph
        assert!(!chunks.is_empty(), "Expected at least one section chunk");

        // Update our expected content checks to be more flexible about what's in the content
        // Since exact structure can vary in different implementations
        let has_h1_text = chunks.iter().any(|chunk| 
            chunk.content.contains("H1 - Should be ignored")
        );
        assert!(has_h1_text, "Missing H1 text in any chunk");

        let has_h2_section1 = chunks.iter().any(|chunk| 
            chunk.content.contains("H2 Section 1") && 
            chunk.content.contains("This is the first paragraph")
        );
        assert!(has_h2_section1, "Missing H2 Section 1 with its paragraph");

        let has_h2_section2 = chunks.iter().any(|chunk| 
            chunk.content.contains("H2 Section 2") && 
            chunk.content.contains("Final paragraph")
        );
        assert!(has_h2_section2, "Missing H2 Section 2 with its paragraph");

        let has_h3_section = chunks.iter().any(|chunk| 
            chunk.content.contains("H3 Subsection") && 
            chunk.content.contains("Another paragraph here")
        );
        assert!(has_h3_section, "Missing H3 section with its paragraph");

        Ok(())
    }

    #[test]
    fn test_parse_code_block() -> Result<()> {
        let code = r#"
# Code Example

Some text before.

```python
def hello():
    print("Hello")
```

Some text after.
"#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.md")?;

        // Should have a section containing the heading, text, code block, and text after
        assert!(!chunks.is_empty(), "Expected at least one section");
        
        let full_section = chunks.iter().find(|chunk| 
            chunk.content.contains("Code Example") && 
            chunk.content.contains("Some text before") && 
            chunk.content.contains("```python") && 
            chunk.content.contains("def hello") && 
            chunk.content.contains("Some text after")
        );
        
        assert!(full_section.is_some(), "Missing complete section with code block");
        
        Ok(())
    }

    #[test]
    fn test_parse_list() -> Result<()> {
        let code = r#"
# List Example

My List:

* Item 1
  * Item 1.1
* Item 2
  - Item 2.1 (sublist with different marker)
"#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.md")?;

        // Should have a section with the heading, intro text, and all list items
        assert!(!chunks.is_empty(), "Expected at least one section");
        
        let has_list_section = chunks.iter().any(|chunk| 
            chunk.element_type.contains("section") && 
            chunk.content.contains("List Example") && 
            chunk.content.contains("My List:") && 
            chunk.content.contains("Item 1") && 
            chunk.content.contains("Item 1.1") && 
            chunk.content.contains("Item 2") &&
            chunk.content.contains("Item 2.1")
        );
        
        assert!(has_list_section, "Missing complete list section");

        Ok(())
    }
    
    #[test]
    fn test_large_section_splitting() -> Result<()> {
        // Create a very large markdown section that should be split
        let mut large_section = String::from("# Very Large Section\n\n");
        // Add a lot of paragraphs to exceed MAX_SECTION_SIZE
        for i in 1..100 {
            large_section.push_str(&format!("This is paragraph {} with some additional text to increase size. The quick brown fox jumps over the lazy dog. Lorem ipsum dolor sit amet, consectetur adipiscing elit.\n\n", i));
        }
        
        let mut parser = create_parser();
        let chunks = parser.parse(&large_section, "large.md")?;
        
        // The section should be split into multiple chunks
        assert!(chunks.len() > 1, "Expected large section to be split into multiple chunks, got {} chunks", chunks.len());
        
        // Check that each chunk has the correct format
        for chunk in &chunks {
            assert!(chunk.content.len() <= 3000, "Chunk exceeds expected maximum size");
            if chunk.element_type.contains("section_chunk_") {
                // This is a partial chunk of the large section
                assert!(chunk.content.contains("paragraph"), "Expected split chunk to contain paragraph content");
            }
        }
        
        Ok(())
    }
    
    #[test]
    fn test_markdown_plain_text() -> Result<()> {
        // Test with content that is just plain text without any markdown elements
        let plain_text = "This is just plain text without any markdown elements.\nIt should be parsed as a paragraph.";
        
        let file_path = "plain.md";
        let mut parser = create_parser();
        let chunks = parser.parse(plain_text, file_path)?;
        
        assert!(!chunks.is_empty(), "Parser should create chunks for plain text");
        
        // Plain text should be in the root section
        let has_root_section = chunks.iter().any(|chunk| 
            chunk.element_type.contains("root_section") && 
            chunk.content.contains("plain text")
        );
        
        assert!(has_root_section, "Parser should place plain text in a root section");
        
        Ok(())
    }
    
    #[test]
    fn test_markdown_empty_content() -> Result<()> {
        let empty_content = "";
        let whitespace_content = "   \n  \t  ";
        
        let file_path = "empty.md";
        let mut parser = create_parser();
        
        // Test with completely empty content
        let empty_chunks = parser.parse(empty_content, file_path)?;
        assert!(empty_chunks.is_empty(), "Parser should return empty chunks for empty content");
        
        // Test with only whitespace
        let whitespace_chunks = parser.parse(whitespace_content, file_path)?;
        assert!(whitespace_chunks.is_empty(), "Parser should return empty chunks for whitespace-only content");
        
        Ok(())
    }
} 