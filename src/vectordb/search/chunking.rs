use std::cmp;

#[derive(Debug, Clone)]
pub struct ChunkInfo {
    pub text: String,
    pub start_line: usize, // 1-indexed
    pub end_line: usize,   // 1-indexed
}

/// Splits content into chunks based on double newlines (paragraphs).
/// Tracks the 1-based start and end lines for each chunk.
pub fn chunk_by_paragraphs(content: &str) -> Vec<ChunkInfo> {
    let mut chunks = Vec::new();
    let mut current_line_num = 1;
    let mut chunk_start_line = 1;
    let mut current_chunk = String::new();

    for line in content.lines() {
        if line.trim().is_empty() {
            // Potential paragraph break
            if !current_chunk.is_empty() {
                // End of a paragraph chunk
                chunks.push(ChunkInfo {
                    text: current_chunk.trim().to_string(),
                    start_line: chunk_start_line,
                    end_line: current_line_num -1, // Previous line was the end
                });
                current_chunk.clear();
                // Next non-empty line will start a new chunk
                chunk_start_line = current_line_num + 1;
            } else {
                 // Multiple empty lines, just advance chunk_start_line
                 chunk_start_line = current_line_num + 1;
            }
        } else {
            // Non-empty line, part of the current chunk
            if !current_chunk.is_empty() {
                current_chunk.push('\n');
            }
            current_chunk.push_str(line);
        }
        current_line_num += 1;
    }

    // Add the last chunk if it wasn't terminated by an empty line
    if !current_chunk.is_empty() {
        chunks.push(ChunkInfo {
            text: current_chunk.trim().to_string(),
            start_line: chunk_start_line,
            // End line is the last line number we processed
            end_line: cmp::max(chunk_start_line, current_line_num.saturating_sub(1)),
        });
    }

    chunks
}

// --- Basic Tests ---
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_paragraphs() {
        let content = "First paragraph.\nLine two.\n\nSecond paragraph.\n\nThird.";
        let chunks = chunk_by_paragraphs(content);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].text, "First paragraph.\nLine two.");
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 2);
        assert_eq!(chunks[1].text, "Second paragraph.");
        assert_eq!(chunks[1].start_line, 4);
        assert_eq!(chunks[1].end_line, 4);
        assert_eq!(chunks[2].text, "Third.");
        assert_eq!(chunks[2].start_line, 6);
        assert_eq!(chunks[2].end_line, 6);
    }

     #[test]
     fn test_leading_trailing_empty_lines() {
        let content = "\n\nFirst paragraph.\n\n\nSecond paragraph.\n\n";
        let chunks = chunk_by_paragraphs(content);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].text, "First paragraph.");
        assert_eq!(chunks[0].start_line, 3);
        assert_eq!(chunks[0].end_line, 3);
        assert_eq!(chunks[1].text, "Second paragraph.");
        assert_eq!(chunks[1].start_line, 6);
        assert_eq!(chunks[1].end_line, 6);
    }

     #[test]
     fn test_no_empty_lines() {
        let content = "Single line one.\nSingle line two.";
        let chunks = chunk_by_paragraphs(content);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "Single line one.\nSingle line two.");
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 2);
    }

     #[test]
     fn test_empty_content() {
        let content = "";
        let chunks = chunk_by_paragraphs(content);
        assert!(chunks.is_empty());
     }

     #[test]
     fn test_only_empty_lines() {
         let content = "\n\n\n";
         let chunks = chunk_by_paragraphs(content);
         assert!(chunks.is_empty());
     }

      #[test]
    fn test_single_line_content() {
        let content = "Just one line.";
        let chunks = chunk_by_paragraphs(content);       assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "Just one line.");
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 1);
    }
} 