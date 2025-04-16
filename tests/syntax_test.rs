use anyhow::Result;
use vectordb_lib::syntax::markdown::MarkdownParser;
use vectordb_lib::syntax::parser::SyntaxParser;

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
        
        // Validate some expected chunks
        let has_heading1 = chunks.iter().any(|chunk| 
            chunk.element_type == "h1" && 
            chunk.content.contains("Heading 1")
        );
        assert!(has_heading1, "Parser should extract h1 heading");
        
        let has_heading2 = chunks.iter().any(|chunk| 
            chunk.element_type == "h2" && 
            chunk.content.contains("Heading 2")
        );
        assert!(has_heading2, "Parser should extract h2 heading");
        
        let has_heading3 = chunks.iter().any(|chunk| 
            chunk.element_type == "h3" && 
            chunk.content.contains("Heading 3")
        );
        assert!(has_heading3, "Parser should extract h3 heading");
        
        let has_paragraph = chunks.iter().any(|chunk| 
            chunk.element_type == "paragraph" && 
            chunk.content.contains("This is a paragraph")
        );
        assert!(has_paragraph, "Parser should extract paragraph");
        
        let has_code_block = chunks.iter().any(|chunk| 
            chunk.element_type == "code_block" && 
            chunk.content.contains("fn main")
        );
        assert!(has_code_block, "Parser should extract code block");
        
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
        
        // Check if the content was recognized as a paragraph
        let has_paragraph = chunks.iter().any(|chunk| 
            chunk.element_type == "paragraph" && 
            chunk.content.contains("plain text")
        );
        
        assert!(has_paragraph, "Parser should recognize plain text as a paragraph");
        
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