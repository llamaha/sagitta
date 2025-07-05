/* New Test File */
#[cfg(test)]
mod fallback_tests {
    // Use crate::syntax::get_chunks as it's defined in the parent module's mod.rs
    use crate::get_chunks;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_fallback_parser_creates_single_chunk() {
        // Create a temporary file with an unsupported extension
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "Line 1\nLine 2\nLine 3";
        writeln!(temp_file, "{content}").unwrap();
        let temp_path = temp_file.path().to_path_buf();
        // Rename to have a .txt extension to ensure fallback
        let new_path = temp_path.with_extension("txt");
        std::fs::rename(&temp_path, &new_path).unwrap();

        // Call get_chunks, which should use the FallbackParser
        let chunks = get_chunks(&new_path).unwrap();

        // Assertions
        assert_eq!(chunks.len(), 1, "Fallback parser should return exactly one chunk for this small input.");

        let chunk = &chunks[0];
        // Adjust expected content: No trailing newline from parser logic
        let expected_content = content; // Original content without added newline
        assert_eq!(chunk.content, expected_content, "Chunk content should match file content (without trailing newline).");
        assert_eq!(chunk.start_line, 1, "Chunk should start at line 1.");
        assert_eq!(chunk.end_line, 3, "Chunk should end at the last line.");
        assert_eq!(chunk.language, "fallback", "Chunk language should be fallback.");
        // Adjust expected element type: Now includes index
        assert_eq!(chunk.element_type, "fallback_chunk_0", "Chunk element_type should be fallback_chunk_0.");

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