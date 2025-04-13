/* New Test File */
#[cfg(test)]
mod fallback_tests {
    use crate::syntax::get_chunks;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_fallback_parser_creates_single_chunk() {
        // Create a temporary file with an unsupported extension
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "Line 1\nLine 2\nLine 3";
        writeln!(temp_file, "{}", content).unwrap();
        let temp_path = temp_file.path().to_path_buf();
        // Rename to have a .txt extension to ensure fallback
        let new_path = temp_path.with_extension("txt");
        std::fs::rename(&temp_path, &new_path).unwrap();

        // Call get_chunks, which should use the FallbackParser
        let chunks = get_chunks(&new_path).unwrap();

        // Assertions
        assert_eq!(chunks.len(), 1, "Fallback parser should return exactly one chunk.");

        let chunk = &chunks[0];
        // Need to read the content again as writeln adds a newline
        let expected_content = format!("{}\n", content);
        assert_eq!(chunk.content, expected_content, "Chunk content should match file content.");
        assert_eq!(chunk.start_line, 1, "Chunk should start at line 1.");
        assert_eq!(chunk.end_line, 3, "Chunk should end at the last line.");
        assert_eq!(chunk.language, "fallback", "Chunk language should be fallback.");
        assert_eq!(chunk.element_type, "fallback_chunk", "Chunk element_type should be fallback_chunk.");

        // Clean up the renamed file
        std::fs::remove_file(&new_path).unwrap();
    }

    #[test]
    fn test_fallback_parser_empty_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path().to_path_buf();
        let new_path = temp_path.with_extension("empty");
        std::fs::rename(&temp_path, &new_path).unwrap();

        let chunks = get_chunks(&new_path).unwrap();

        assert_eq!(chunks.len(), 1, "Should return one chunk for an empty file.");
        let chunk = &chunks[0];
        assert_eq!(chunk.content, "", "Content should be empty.");
        assert_eq!(chunk.start_line, 1, "Start line should be 1.");
        assert_eq!(chunk.end_line, 1, "End line should be 1 for an empty file.");
        assert_eq!(chunk.language, "fallback", "Language should be fallback.");
        assert_eq!(chunk.element_type, "fallback_chunk", "Element type should be fallback_chunk.");

        std::fs::remove_file(&new_path).unwrap();
    }

} 