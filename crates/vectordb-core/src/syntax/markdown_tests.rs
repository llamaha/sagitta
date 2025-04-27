// src/syntax/markdown_tests.rs
#[cfg(test)]
mod tests {
    // Adjust import paths
    use crate::syntax::parser::{CodeChunk, SyntaxParser};
    use crate::syntax::markdown::{MarkdownParser, MAX_SECTION_SIZE}; // Import constant
    use anyhow::Result;

    fn create_parser() -> MarkdownParser {
        MarkdownParser::new()
    }

    // Helper to check if a chunk's content contains all expected substrings
    fn assert_content_contains_all(content: &str, expected: &[&str]) {
        for item in expected {
            assert!(content.contains(item), "Expected content to contain '{}', got: \n{}", item, content);
        }
    }

    // Helper to check if a chunk's content does NOT contain any unexpected substrings
    fn assert_content_not_contains_any(content: &str, not_expected: &[&str]) {
        for item in not_expected {
            assert!(!content.contains(item), "Expected content NOT to contain '{}', got: \n{}", item, content);
        }
    }

    #[test]
    fn test_basic_section_structure_and_context() -> Result<()> {
        let code = r#"
Root content line 1.
Root content line 2.

# Heading 1

Paragraph under H1.

## Heading 1.1

Paragraph under H1.1.
List:
* Item A
* Item B

### Heading 1.1.1

Paragraph under H1.1.1.

## Heading 1.2

Paragraph under H1.2.
```python
print("hello")
```

# Heading 2

Paragraph under H2.
"#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.md")?;

        assert_eq!(chunks.len(), 6, "Expected 6 chunks (root, H1, H1.1, H1.1.1, H1.2, H2)");

        // 1. Root Content Chunk
        let chunk0 = &chunks[0];
        assert_eq!(chunk0.element_type, "root_content");
        assert_content_contains_all(&chunk0.content, &["Root content line 1.", "Root content line 2."]);
        assert_content_not_contains_any(&chunk0.content, &["# Heading", "Paragraph under"]);
        assert_eq!(chunk0.start_line, 1);
        assert_eq!(chunk0.end_line, 2); // Ends before H1 starts

        // 2. H1 Section Chunk
        let chunk1 = &chunks[1];
        assert_eq!(chunk1.element_type, "h1_section");
        assert_content_contains_all(&chunk1.content, &["# Heading 1", "Paragraph under H1."]);
        assert_content_not_contains_any(&chunk1.content, &["Root content", "## Heading 1.1", "Paragraph under H1.1"]);
        assert_eq!(chunk1.start_line, 4); // Starts at the heading
        assert_eq!(chunk1.end_line, 6); // Ends before H1.1 starts
        
        // 3. H1.1 Section Chunk
        let chunk2 = &chunks[2];
        assert_eq!(chunk2.element_type, "h2_section");
        assert_content_contains_all(&chunk2.content, &["# Heading 1", "## Heading 1.1", "Paragraph under H1.1.", "* Item A", "* Item B"]);
        assert_content_not_contains_any(&chunk2.content, &["Root content", "Paragraph under H1.", "### Heading 1.1.1", "Paragraph under H1.1.1"]);
        assert_eq!(chunk2.start_line, 8); // Starts at the H1.1 heading
        assert_eq!(chunk2.end_line, 12); // Ends before H1.1.1 starts
        
        // 4. H1.1.1 Section Chunk
        let chunk3 = &chunks[3];
        assert_eq!(chunk3.element_type, "h3_section");
        assert_content_contains_all(&chunk3.content, &["# Heading 1", "## Heading 1.1", "### Heading 1.1.1", "Paragraph under H1.1.1."]);
        assert_content_not_contains_any(&chunk3.content, &["Root content", "Paragraph under H1.", "Paragraph under H1.1.", "## Heading 1.2", "Paragraph under H1.2"]);
        assert_eq!(chunk3.start_line, 14); // Starts at the H1.1.1 heading
        assert_eq!(chunk3.end_line, 16); // Ends before H1.2 starts
        
        // 5. H1.2 Section Chunk
        let chunk4 = &chunks[4];
        assert_eq!(chunk4.element_type, "h2_section");
        assert_content_contains_all(&chunk4.content, &["# Heading 1", "## Heading 1.2", "Paragraph under H1.2.", "```python", "print(\"hello\")"]);
        assert_content_not_contains_any(&chunk4.content, &["Root content", "Paragraph under H1.", "## Heading 1.1", "### Heading 1.1.1", "# Heading 2"]);
        assert_eq!(chunk4.start_line, 18); // Starts at the H1.2 heading
        assert_eq!(chunk4.end_line, 22); // Ends before H2 starts
        
        // 6. H2 Section Chunk (This index needs checking - likely index 5 if root counted)
        // Re-check expected chunk count. If root + 5 sections = 6 chunks.
        // Let's adjust assertion and indices assuming 6 chunks.
        assert_eq!(chunks.len(), 6, "Expected 6 chunks (root, H1, H1.1, H1.1.1, H1.2, H2)");
        let chunk5 = &chunks[5];
        assert_eq!(chunk5.element_type, "h1_section"); // H2 is under root
        assert_content_contains_all(&chunk5.content, &["# Heading 2", "Paragraph under H2."]);
        assert_content_not_contains_any(&chunk5.content, &["Root content", "# Heading 1", "## Heading", "### Heading"]);
        assert_eq!(chunk5.start_line, 24); // Starts at the H2 heading
        assert_eq!(chunk5.end_line, 26); // Ends at EOF

        Ok(())
    }

    #[test]
    fn test_setext_headings() -> Result<()> {
        let code = r#"
Root content.

Heading 1 (Setext)
=========

Paragraph under H1.

Heading 2 (Setext)
---------

Paragraph under H2.
"#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.md")?;

        assert_eq!(chunks.len(), 3, "Expected 3 chunks (root, H1, H2)");

        // Root
        assert_eq!(chunks[0].element_type, "root_content");
        assert_content_contains_all(&chunks[0].content, &["Root content."]);
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 1);

        // H1
        assert_eq!(chunks[1].element_type, "h1_section");
        assert_content_contains_all(&chunks[1].content, &["# Heading 1 (Setext)", "Paragraph under H1."]);
        assert_content_not_contains_any(&chunks[1].content, &["Root content", "Heading 2", "Paragraph under H2"]);
        assert_eq!(chunks[1].start_line, 3); // Heading starts on line 3
        assert_eq!(chunks[1].end_line, 6); // Content ends on line 6

        // H2 (under H1)
        assert_eq!(chunks[2].element_type, "h2_section");
        assert_content_contains_all(&chunks[2].content, &["# Heading 1 (Setext)", "## Heading 2 (Setext)", "Paragraph under H2."]);
        assert_content_not_contains_any(&chunks[2].content, &["Root content", "Paragraph under H1."]);
        assert_eq!(chunks[2].start_line, 8); // Heading starts on line 8
        assert_eq!(chunks[2].end_line, 11); // Content ends on line 11

        Ok(())
    }

    #[test]
    fn test_adjacent_headings_no_content() -> Result<()> {
        let code = r#"
# H1
## H1.1
### H1.1.1
Actual content here under H3.
## H1.2
More content.
"#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.md")?;

        // Expecting chunks for H1.1.1 and H1.2, as H1 and H1.1 have no direct content before the next heading.
        // The logic *should* produce chunks for H1.1.1 and H1.2 which have content.
        // If it skips H1/H1.1, that's acceptable for now, but the contentful ones should be present.
        assert_eq!(chunks.len(), 2, "Expected 2 chunks (H1.1.1, H1.2) with content");

        // Chunk 1: H1.1.1
        if chunks.len() == 2 { // Only check contents if the count is right
            assert_eq!(chunks[0].element_type, "h3_section", "First chunk should be H1.1.1");
            assert_content_contains_all(&chunks[0].content, &["# H1", "## H1.1", "### H1.1.1", "Actual content here under H3."]);
            assert_eq!(chunks[0].start_line, 3); // H1.1.1 starts line 3
            assert_eq!(chunks[0].end_line, 4); // Content ends line 4

            // Chunk 2: H1.2
            assert_eq!(chunks[1].element_type, "h2_section", "Second chunk should be H1.2");
            assert_content_contains_all(&chunks[1].content, &["# H1", "## H1.2", "More content."]);
            assert_content_not_contains_any(&chunks[1].content, &["H1.1", "H1.1.1", "Actual content"]);
            assert_eq!(chunks[1].start_line, 5); // H1.2 starts line 5
            assert_eq!(chunks[1].end_line, 6); // Content ends line 6
        }

        Ok(())
    }
    
    #[test]
    fn test_large_section_splitting() -> Result<()> {
        // Generate content designed to exceed MAX_SECTION_SIZE slightly when combined with headers
        let header1 = "# Main Section\n\n";
        let header2 = "## Subsection\n\n";
        let base_content = "Paragraph with text. ".repeat(MAX_SECTION_SIZE / 20); // Make base large
        
        // Estimate header length (approximate)
        let header_len = header1.len() + header2.len();
        // Create content *guaranteed* to cause a split by making the total length >> MAX_SECTION_SIZE
        let required_len = MAX_SECTION_SIZE * 2; // Aim for roughly double the max size
        let repetitions = required_len / base_content.len() + 1;
        let large_content = base_content.repeat(repetitions);
        
        // Split the large content notionally for assertion checking later
        let approx_split_point = (MAX_SECTION_SIZE - header_len).min(large_content.len());
        // Try to find a newline near the split point
        let split_point = large_content[..approx_split_point].rfind('\n').map_or(approx_split_point, |p| p + 1);

        let content1 = &large_content[..split_point];
        let content2 = &large_content[split_point..];
        
        let code = format!("{}{}{}", header1, header2, large_content);
        
        let mut parser = create_parser();
        let chunks = parser.parse(&code, "test_large.md")?;
        
        println!("MAX_SECTION_SIZE: {}", MAX_SECTION_SIZE);
        println!("Code length: {}", code.len());
        for (i, chunk) in chunks.iter().enumerate() {
            println!("Chunk {}: type={}, len={}, start={}, end={}, content_preview='{}'", 
                     i, chunk.element_type, chunk.content.len(), chunk.start_line, chunk.end_line, chunk.content.chars().take(50).collect::<String>());
        }

        assert!(chunks.len() > 1, "Expected section to be split into multiple chunks ({})", chunks.len());
        assert_eq!(chunks[0].element_type, "h2_section_split_1", "First chunk should be marked as split_1");
        assert!(chunks[0].content.len() <= MAX_SECTION_SIZE, "First chunk size should not exceed MAX_SECTION_SIZE");
        assert_content_contains_all(&chunks[0].content, &["# Main Section", "## Subsection"]);
        assert_eq!(chunks[0].start_line, 3); // Subsection starts line 3

        assert_eq!(chunks[1].element_type, "h2_section_split_2", "Second chunk should be marked as split_2");
        assert_content_contains_all(&chunks[1].content, &["# Main Section", "## Subsection"]);
        assert!(chunks[1].start_line > chunks[0].end_line, "Second chunk start line should be after first chunk end line");
        
        Ok(())
    }

    #[test]
    fn test_plain_text_no_headings() -> Result<()> {
        let code = r#"
This is line one.
This is line two.
  Indented line three.

Line five after blank.
"#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test_plain.md")?;

        assert_eq!(chunks.len(), 1, "Expected 1 chunk for plain text");
        println!("Plain text input:\n'{}'", code);
        println!("Plain text lines detected: {}", code.lines().count());
        println!("Chunk[0] details: start={}, end={}, type='{}'", chunks[0].start_line, chunks[0].end_line, chunks[0].element_type);
        assert_eq!(chunks[0].element_type, "root_plain_text");
        assert_content_contains_all(&chunks[0].content, &["line one", "line two", "line three", "Line five"]);
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 6, "Expected end line 6 for raw string input with 6 lines");
        Ok(())
    }
    
    #[test]
    fn test_plain_text_splitting_by_lines() -> Result<()> {
        // Create text longer than MAX_CHUNK_LINES (assuming 500)
        let mut lines = Vec::new();
        for i in 1..=600 {
            lines.push(format!("Line {}", i));
        }
        let code = lines.join("\n");
        
        let mut parser = create_parser();
        let chunks = parser.parse(&code, "test_plain_split.md")?;
        
        assert_eq!(chunks.len(), 2, "Expected 2 chunks due to line limit");
        
        assert_eq!(chunks[0].element_type, "root_plain_text_split_1");
        assert_content_contains_all(&chunks[0].content, &["Line 1", "Line 500"]);
        assert_content_not_contains_any(&chunks[0].content, &["Line 501"]);
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 500);
        
        assert_eq!(chunks[1].element_type, "root_plain_text_split_2");
        assert_content_contains_all(&chunks[1].content, &["Line 501", "Line 600"]);
        assert_content_not_contains_any(&chunks[1].content, &["Line 500"]);
        assert_eq!(chunks[1].start_line, 501);
        assert_eq!(chunks[1].end_line, 600);
        
        Ok(())
    }

    #[test]
    fn test_empty_content() -> Result<()> {
        let code = "";
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test_empty.md")?;
        assert!(chunks.is_empty(), "Expected no chunks for empty input");
        Ok(())
    }

    #[test]
    fn test_whitespace_only_content() -> Result<()> {
        let code = "  \n\t\n  ";
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test_whitespace.md")?;
        assert!(chunks.is_empty(), "Expected no chunks for whitespace-only input");
        Ok(())
    }

    // TODO: Add test for root content splitting if it exceeds MAX_SECTION_SIZE
    // TODO: Add test for case where last section goes to EOF
    // TODO: Add test with mixed ATX and Setext headings
} 