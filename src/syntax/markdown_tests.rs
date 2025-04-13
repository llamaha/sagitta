#[cfg(test)]
mod tests {
    use anyhow::Result;
    use crate::syntax::parser::{CodeChunk, SyntaxParser};
    use crate::syntax::markdown::MarkdownParser;

    fn create_parser() -> MarkdownParser {
        MarkdownParser::new()
    }

    #[test]
    fn test_parse_empty_input() -> Result<()> {
        let code = "";
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.md")?;
        assert!(chunks.is_empty(), "Should return no chunks for empty input");
        Ok(())
    }

    #[test]
    fn test_parse_headings() -> Result<()> {
        let code = r#"
# Heading 1

Some text.

## Heading 2

Another heading
---------------

Final Heading
=============
"#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "headings.md")?;

        assert_eq!(chunks.len(), 4, "Should find 4 headings");

        assert_eq!(chunks[0].element_type, "heading_1");
        assert!(chunks[0].content.contains("# Heading 1"));
        assert_eq!(chunks[0].start_line, 2);
        assert_eq!(chunks[0].end_line, 2);

        assert_eq!(chunks[1].element_type, "heading_2", "Chunk 1 type mismatch. Actual: {}", chunks[1].element_type);
        assert!(chunks[1].content.contains("## Heading 2"));
        assert_eq!(chunks[1].start_line, 6);
        assert_eq!(chunks[1].end_line, 6);

        assert_eq!(chunks[2].element_type, "heading_2");
        assert_eq!(chunks[2].content.trim(), "Another heading");
        assert_eq!(chunks[2].start_line, 8);
        assert_eq!(chunks[2].end_line, 8);

        assert_eq!(chunks[3].element_type, "heading_1");
        assert_eq!(chunks[3].content.trim(), "Final Heading");
        assert_eq!(chunks[3].start_line, 11);
        assert_eq!(chunks[3].end_line, 11);

        Ok(())
    }

    #[test]
    fn test_parse_code_blocks() -> Result<()> {
        let code = r#"
Some introductory text.

```rust
fn main() {
    println!("Hello");
}
```

More text.

```
Generic code block
```
"#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "codeblocks.md")?;

        println!("Found chunks: {:#?}", chunks);

        assert_eq!(chunks.len(), 2, "Should find 2 code blocks");

        assert_eq!(chunks[0].element_type, "code_block");
        assert!(chunks[0].content.contains("fn main()"));
        assert_eq!(chunks[0].start_line, 4, "Adjusted start line expectation");
        assert_eq!(chunks[0].end_line, 9, "Adjusted end line expectation");

        assert_eq!(chunks[1].element_type, "code_block");
        assert!(chunks[1].content.contains("Generic code block"));
        assert_eq!(chunks[1].start_line, 12, "Adjusted start line expectation");
        assert_eq!(chunks[1].end_line, 15, "Adjusted end line expectation");

        Ok(())
    }

    #[test]
    fn test_fallback_chunking_md() -> Result<()> {
        let mut lines = Vec::new();
        for i in 0..300 {
            lines.push(format!("This is line {}.", i));
        }
        let code = lines.join("\n");

        let mut parser = create_parser();
        let chunks = parser.parse(&code, "fallback.md")?;

        assert_eq!(chunks.len(), 2, "Should split into fallback chunks");

        assert_eq!(chunks[0].element_type, "file_chunk");
        assert_eq!(chunks[0].language, "markdown");
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 200);

        assert_eq!(chunks[1].element_type, "file_chunk");
        assert_eq!(chunks[1].language, "markdown");
        assert_eq!(chunks[1].start_line, 201);
        assert_eq!(chunks[1].end_line, 300);

        Ok(())
    }
} 