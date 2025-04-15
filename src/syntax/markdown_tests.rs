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
        expected_content_trimmed: &str,
        expected_start: usize,
        expected_end: usize,
        expected_element: &str,
    ) {
        assert_eq!(chunk.content.trim(), expected_content_trimmed.trim());
        assert_eq!(chunk.start_line, expected_start);
        assert_eq!(chunk.end_line, expected_end);
        assert_eq!(chunk.language, "markdown");
        assert_eq!(chunk.element_type, expected_element);
        assert!(!chunk.file_path.is_empty());
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

        // Expecting H1, H2, P, H3, P, H2, P = 7 chunks
        assert_eq!(chunks.len(), 7, "Expected H1, H2, P, H3, P, H2, P");

        assert_chunk(&chunks[0], "# H1 - Should be ignored by current query", 2, 3, "h1");
        assert_chunk(&chunks[1], "## H2 Section 1", 4, 5, "h2");
        assert_chunk(&chunks[2], "This is the first paragraph.", 6, 7, "paragraph");
        assert_chunk(&chunks[3], "### H3 Subsection", 8, 9, "h3");
        assert_chunk(&chunks[4], "Another paragraph here.", 10, 11, "paragraph");
        assert_chunk(&chunks[5], "## H2 Section 2", 12, 13, "h2");
        assert_chunk(&chunks[6], "Final paragraph.", 14, 15, "paragraph");

        Ok(())
    }

    #[test]
    fn test_parse_code_block() -> Result<()> {
        let code = r#"
Some text before.

```python
def hello():
    print("Hello")
```

Some text after.
"#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.md")?;

        // Expecting P, Code Block, P
        assert_eq!(chunks.len(), 3, "Expected Paragraph, Code Block, Paragraph");

        assert_chunk(&chunks[0], "Some text before.", 2, 3, "paragraph");
        assert_chunk(&chunks[1], "```python\ndef hello():\n    print(\"Hello\")\n```", 4, 8, "code_block");
        assert_chunk(&chunks[2], "Some text after.", 9, 10, "paragraph");

        Ok(())
    }

    #[test]
    fn test_parse_list() -> Result<()> {
        let code = r#"
My List:

* Item 1
  * Item 1.1
* Item 2
  - Item 2.1 (sublist with different marker)
"#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.md")?;

        // Expecting P, Top-Level Item 1, Top-Level Item 2 due to filtering logic
        assert_eq!(chunks.len(), 3, "Expected P, Top-Level Item 1, Top-Level Item 2");

        assert_chunk(&chunks[0], "My List:", 2, 3, "paragraph");
        // Chunk 1: Top-level Item 1 (node includes nested item's text)
        assert_chunk(&chunks[1], "* Item 1\n  * Item 1.1", 4, 6, "list_item");
        // Chunk 2: Top-level Item 2 (node includes nested item's text)
        assert_chunk(&chunks[2], "* Item 2\n  - Item 2.1 (sublist with different marker)", 6, 8, "list_item");

        Ok(())
    }

     #[test]
    fn test_parse_mixed_content() -> Result<()> {
        let code = r#"
## Intro

Just a paragraph.

* List A
* List B

```text
Code example
```

End paragraph.
"#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.md")?;

        // H2, P, L1, L2, Code, P = 6 chunks
        assert_eq!(chunks.len(), 6, "Expected H2, P, L1, L2, Code, P");
        assert_chunk(&chunks[0], "## Intro", 2, 3, "h2");
        assert_chunk(&chunks[1], "Just a paragraph.", 4, 5, "paragraph");
        assert_chunk(&chunks[2], "* List A", 6, 7, "list_item");
        assert_chunk(&chunks[3], "* List B", 7, 9, "list_item"); // Includes trailing newline before code block
        assert_chunk(&chunks[4], "```text\nCode example\n```", 9, 12, "code_block");
        assert_chunk(&chunks[5], "End paragraph.", 13, 14, "paragraph");
        Ok(())
    }


    #[test]
    fn test_parse_empty_input_md() -> Result<()> {
        let code = "";
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.md")?;
        assert!(chunks.is_empty());
        Ok(())
    }

    #[test]
    fn test_parse_comments_only_md() -> Result<()> {
        let code = r#"
<!-- HTML Comment -->

[//]: # (Markdown Comment)
"#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.md")?;
        
        // Expect the new line-based fallback chunking behavior
        assert_eq!(chunks.len(), 1, "Should produce one chunk via fallback.");
        let chunk = &chunks[0];
        assert_eq!(chunk.element_type, "fallback_line_chunk_0", "Element type should indicate line-based fallback.");
        assert_eq!(chunk.language, "markdown", "Language should still be markdown.");
        // Optionally, check content if necessary, but type and count are primary here.
        assert!(chunk.content.contains("<!-- HTML Comment -->")); 
        assert!(chunk.content.contains("[//]: # (Markdown Comment)"));

        Ok(())
    }
} 