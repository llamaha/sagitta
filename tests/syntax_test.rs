use anyhow::Result;
use vectordb_core::syntax::markdown::MarkdownParser;
use vectordb_core::syntax::parser::SyntaxParser;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_parser() -> Result<()> {
        let markdown_content = r#"# Heading 1

This is a paragraph with some text.

## Heading 2

Some more content here.

```rust
fn main() {
    println!("Hello, world!");
}
```

### Heading 3

- List item 1
- List item 2
  - Nested list item
- List item 3
"#;

        let file_path = "test_document.md";
        let mut parser = MarkdownParser::new();
        let chunks = parser.parse(markdown_content, file_path)?;

        // General validations
        assert!(!chunks.is_empty(), "Parser should extract chunks from markdown");
        
        // Validate some expected chunks using the section-based element types
        let has_heading1_section = chunks.iter().any(|chunk|
            chunk.element_type == "h1_section" &&
            chunk.content.contains("Heading 1")
        );
        assert!(has_heading1_section, "Parser should extract h1 section chunk");

        let has_heading2_section = chunks.iter().any(|chunk|
            chunk.element_type == "h2_section" &&
            chunk.content.contains("Heading 2")
        );
        assert!(has_heading2_section, "Parser should extract h2 section chunk");

        let has_heading3_section = chunks.iter().any(|chunk|
            chunk.element_type == "h3_section" &&
            chunk.content.contains("Heading 3")
        );
        assert!(has_heading3_section, "Parser should extract h3 section chunk");

        // Paragraphs and code blocks might be within sections, check for their content
        let has_paragraph_content = chunks.iter().any(|chunk|
            chunk.content.contains("This is a paragraph")
        );
        assert!(has_paragraph_content, "Parser should include paragraph content in some chunk");

        let has_code_block_content = chunks.iter().any(|chunk|
            chunk.content.contains("fn main")
        );
        assert!(has_code_block_content, "Parser should include code block content in some chunk");
        
        // Validate metadata
        for chunk in &chunks {
            assert_eq!(chunk.file_path, file_path, "Chunk should have correct file path");
            assert_eq!(chunk.language, "markdown", "Chunk should have correct language");
            assert!(chunk.start_line > 0, "Chunk should have positive start line");
            assert!(chunk.end_line >= chunk.start_line, "Chunk end line should be >= start line");
        }
        
        Ok(())
    }
    
    #[test]
    fn test_markdown_plain_text() -> Result<()> {
        // Test with content that is just plain text, which should be parsed as a paragraph
        let plain_text = "This is just plain text without any markdown elements.\nIt should be parsed as a paragraph.";
        
        let file_path = "plain.md";
        let mut parser = MarkdownParser::new();
        let chunks = parser.parse(plain_text, file_path)?;
        
        assert!(!chunks.is_empty(), "Parser should create chunks for plain text");
        
        // Check if the plain text content exists in any chunk, as it might be part of a root_section
        let has_plain_text_content = chunks.iter().any(|chunk|
            chunk.content.contains("plain text") &&
            // Optionally, check if it's in a root section if that's the expected behavior
            (chunk.element_type == "root_section" || chunk.element_type == "paragraph")
        );

        assert!(has_plain_text_content, "Parser should include plain text content in a chunk");
        
        Ok(())
    }
    
    // Test for fallback using truly special case
    #[test]
    fn test_markdown_fallback_with_special_chars() -> Result<()> {
        // Create content that might confuse the parser but shouldn't be empty
        // Unusual characters that might not parse cleanly as markdown elements
        let special_chars = ">=<\n\t\r\u{FEFF}";
        
        let file_path = "special.md";
        let mut parser = MarkdownParser::new();
        let chunks = parser.parse(special_chars, file_path)?;
        
        // We only care that something was produced and no panic occurred
        assert!(!chunks.is_empty(), "Parser should handle unusual characters without failing");
        
        Ok(())
    }
    
    #[test]
    fn test_markdown_empty_content() -> Result<()> {
        let empty_content = "";
        let whitespace_content = "   \n  \t  ";
        
        let file_path = "empty.md";
        let mut parser = MarkdownParser::new();
        
        // Test with completely empty content
        let empty_chunks = parser.parse(empty_content, file_path)?;
        assert!(empty_chunks.is_empty(), "Parser should return empty chunks for empty content");
        
        // Test with only whitespace
        let whitespace_chunks = parser.parse(whitespace_content, file_path)?;
        assert!(whitespace_chunks.is_empty(), "Parser should return empty chunks for whitespace-only content");
        
        Ok(())
    }
} 