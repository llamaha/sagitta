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

/// Calculates the indentation level of a line (number of leading spaces/tabs).
fn calculate_indentation(line: &str) -> usize {
    line.chars().take_while(|&c| c.is_whitespace()).count()
}

/// Splits code content into chunks based on indentation heuristics.
/// Aims to group functions, classes, or blocks. Basic, no real parsing.
pub fn chunk_by_indentation(content: &str) -> Vec<ChunkInfo> {
    let mut chunks = Vec::new();
    let mut current_chunk_lines = Vec::new();
    let mut current_chunk_start_line: Option<usize> = None;

    for (i, line) in content.lines().enumerate() {
        let current_line_num = i + 1; // 1-based line number
        let trimmed_line = line.trim();
        let current_indentation = calculate_indentation(line);

        let is_likely_top_level = current_indentation == 0 && !trimmed_line.is_empty() &&
            (
                trimmed_line.starts_with("fn ") || // Rust
                trimmed_line.starts_with("pub fn") ||
                trimmed_line.starts_with("struct ") ||
                trimmed_line.starts_with("enum ") ||
                trimmed_line.starts_with("impl ") ||
                trimmed_line.starts_with("trait ") ||
                trimmed_line.starts_with("mod ") ||
                trimmed_line.starts_with("function ") || // JS/TS
                trimmed_line.starts_with("async function") || 
                trimmed_line.starts_with("class ") ||
                trimmed_line.starts_with("interface ") ||
                trimmed_line.starts_with("export ") || // Often prefixes other declarations
                // trimmed_line.starts_with("import ") || // Let imports be part of the next block
                trimmed_line.starts_with("const ") || // Top level constants
                trimmed_line.starts_with("let ") || // Top level variables
                trimmed_line.starts_with("var ") || 
                trimmed_line.starts_with("def ") || // Python
                trimmed_line.starts_with("async def") || 
                trimmed_line.starts_with("class ") // Python class
            );

        if current_chunk_start_line.is_none() {
             // Looking for the start of the first chunk
             if !trimmed_line.is_empty() {
                current_chunk_start_line = Some(current_line_num);
                current_chunk_lines.push(line);
             }
             // Otherwise, skip leading empty/whitespace lines
        } else {
            // We are inside a chunk
            if is_likely_top_level {
                // Finish the previous chunk
                let chunk_text = current_chunk_lines.join("\n");
                if !chunk_text.trim().is_empty() {
                    chunks.push(ChunkInfo {
                        text: chunk_text.trim().to_string(),
                        start_line: current_chunk_start_line.unwrap(),
                        end_line: current_line_num - 1, // End on the line before this new top-level
                    });
                }
                // Start the new chunk
                current_chunk_lines.clear();
                current_chunk_lines.push(line);
                current_chunk_start_line = Some(current_line_num);
            } else {
                // Continue the current chunk (add the line regardless of whitespace)
                current_chunk_lines.push(line);
            }
        }
    }

    // Add the last chunk if it exists and is not empty
    if let Some(start_line) = current_chunk_start_line {
         if !current_chunk_lines.is_empty() {
             let chunk_text = current_chunk_lines.join("\n");
             if !chunk_text.trim().is_empty() {
                 chunks.push(ChunkInfo {
                     text: chunk_text.trim().to_string(),
                     start_line,
                     end_line: content.lines().count(), // Ends on the last line of the file
                 });
             }
         }
    }

    chunks // Return directly, filtering is done during creation
}

/// Splits content into fixed-size overlapping chunks of lines.
pub fn chunk_by_lines(content: &str, chunk_size: usize, overlap: usize) -> Vec<ChunkInfo> {
    if chunk_size == 0 {
        return Vec::new();
    }
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();
    if total_lines == 0 {
        return Vec::new();
    }

    let step_size = chunk_size.saturating_sub(overlap);
    // If step_size is 0 (overlap >= chunk_size), it would loop infinitely.
    // Return empty vec in this case, as no meaningful non-repeating chunks can be made.
    if step_size == 0 {
        return Vec::new(); 
    }

    let mut chunks = Vec::new();
    let mut current_start_index = 0;

    loop { // Use loop instead of while
        let current_end_index = cmp::min(current_start_index + chunk_size, total_lines);
        
        // Ensure slice is valid before proceeding
        if current_start_index >= current_end_index {
             break; // Should not happen with valid inputs but safeguard
        }

        let chunk_lines = &lines[current_start_index..current_end_index];
        
        // Check if the chunk itself is non-empty *before* joining and trimming
        if !chunk_lines.is_empty() {
            let chunk_text = chunk_lines.join("\n");
            if !chunk_text.trim().is_empty() {
                chunks.push(ChunkInfo {
                    text: chunk_text,
                    start_line: current_start_index + 1,
                    end_line: current_end_index,
                });
            }
        }
        
        // Calculate next start index
        let next_start_index = current_start_index + step_size;

        // Break condition: Stop if the next start index is at or past the end.
        if next_start_index >= total_lines {
            break;
        }
        
        current_start_index = next_start_index;
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

// --- Tests for chunk_by_lines ---
#[cfg(test)]
mod line_chunking_tests {
    use super::*;

    #[test]
    fn test_lines_simple() {
        let content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
        let chunks = chunk_by_lines(content, 3, 1);
        // Expect 3 chunks with chunk_size=3, overlap=1 (step=2):
        // Chunk 1: Lines 1-3
        // Chunk 2: Lines 3-5
        // Chunk 3: Line 5
        assert_eq!(chunks.len(), 3, "Expected 3 chunks for this input and parameters");
        // Chunk 1
        assert_eq!(chunks[0].text, "Line 1\nLine 2\nLine 3");
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 3);
        // Chunk 2
        assert_eq!(chunks[1].text, "Line 3\nLine 4\nLine 5");
        assert_eq!(chunks[1].start_line, 3);
        assert_eq!(chunks[1].end_line, 5);
        // Chunk 3
        assert_eq!(chunks[2].text, "Line 5");
        assert_eq!(chunks[2].start_line, 5);
        assert_eq!(chunks[2].end_line, 5);
    }

    #[test]
    fn test_lines_no_overlap() {
        let content = "L1\nL2\nL3\nL4\nL5\nL6";
        let chunks = chunk_by_lines(content, 2, 0);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].text, "L1\nL2");
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 2);
        assert_eq!(chunks[1].text, "L3\nL4");
        assert_eq!(chunks[1].start_line, 3);
        assert_eq!(chunks[1].end_line, 4);
        assert_eq!(chunks[2].text, "L5\nL6");
        assert_eq!(chunks[2].start_line, 5);
        assert_eq!(chunks[2].end_line, 6);
    }

    #[test]
    fn test_lines_larger_than_content() {
        let content = "Line 1\nLine 2";
        let chunks = chunk_by_lines(content, 10, 2);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "Line 1\nLine 2");
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 2);
    }

    #[test]
    fn test_lines_with_empty_lines() {
        let content = "Line 1\n\nLine 3\nLine 4\n\nLine 6";
        let chunks = chunk_by_lines(content, 3, 1);
        // Expect 3 chunks: [1,2,3], [3,4,5], [5,6]
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].text, "Line 1\n\nLine 3");
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 3);
        assert_eq!(chunks[1].text, "Line 3\nLine 4\n");
        assert_eq!(chunks[1].start_line, 3);
        assert_eq!(chunks[1].end_line, 5);
        assert_eq!(chunks[2].text, "\nLine 6");
        assert_eq!(chunks[2].start_line, 5);
        assert_eq!(chunks[2].end_line, 6);
    }

    #[test]
    fn test_lines_empty_content() {
        let content = "";
        let chunks = chunk_by_lines(content, 10, 2);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_lines_whitespace_only_content() {
        let content = "  \n\t\n   "; // 3 lines of whitespace
        let chunks = chunk_by_lines(content, 2, 1);
        assert!(chunks.is_empty(), "Chunking whitespace-only content should result in empty vec");
    }

    #[test]
    fn test_lines_zero_chunk_size() {
         let content = "Line 1";
         let chunks = chunk_by_lines(content, 0, 0);
         assert!(chunks.is_empty());
    }
    
    #[test]
    fn test_lines_zero_step_size() {
         // overlap >= chunk_size
         let content = "Line 1\nLine 2";
         let chunks = chunk_by_lines(content, 2, 2);
         assert!(chunks.is_empty());
         let chunks = chunk_by_lines(content, 2, 3);
         assert!(chunks.is_empty());
    }
} 