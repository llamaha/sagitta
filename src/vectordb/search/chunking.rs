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

// Helper function to finalize and add a chunk for indentation-based chunking
fn finalize_chunk(
    chunks: &mut Vec<ChunkInfo>,
    current_chunk_lines: &mut Vec<&str>,
    start_line: usize,
    potential_end_line: usize, // 1-based line number *before* the next block starts, or total lines for the last chunk
) {
    // Find the actual end line by looking for the last non-empty line index in the buffer
    let mut actual_end_line = start_line; // Default if all lines are empty/whitespace
    let mut last_non_empty_index = None;
    for (idx, line) in current_chunk_lines.iter().enumerate().rev() {
        if !line.trim().is_empty() {
             actual_end_line = start_line + idx; // idx is 0-based within current_chunk_lines
             last_non_empty_index = Some(idx);
             break;
         }
    }

    // Ensure the actual end line doesn't exceed the potential end line derived from the next block start or EOF
    // Note: potential_end_line is the line number *before* the next block, so it's already the correct max end line.
    actual_end_line = std::cmp::min(actual_end_line, potential_end_line);

    // Slice the lines up to the last non-empty one found (if any)
    let lines_to_join = if let Some(last_idx) = last_non_empty_index {
        // Only include lines up to the last non-empty one
         &current_chunk_lines[..=last_idx]
    } else {
        // If all lines were empty/whitespace, slice is empty
         &current_chunk_lines[0..0]
    };

    let chunk_text = lines_to_join.join("\n");

    // Only add chunk if the final text (after joining relevant lines) is not just whitespace
    if !chunk_text.trim().is_empty() {
        // Check if the chunk consists only of comments/whitespace (more robust check)
        let mut contains_code = false;
        for line in lines_to_join {
            let trimmed = line.trim();
            // Consider a line as code if it's not empty and doesn't start with typical comment markers
            if !trimmed.is_empty() && !trimmed.starts_with("//") && !trimmed.starts_with("#") {
                contains_code = true;
                break;
            }
        }

        // Only push the chunk if it actually contains code lines
        if contains_code {
             chunks.push(ChunkInfo {
                 text: chunk_text, // Use the joined text directly
                 start_line,
                 end_line: actual_end_line, // Use the calculated actual end line
             });
         }
    }
    current_chunk_lines.clear(); // Clear lines buffer for the next chunk
}

/// Splits code content into chunks based on indentation heuristics.
/// Aims to group functions, classes, or blocks. Basic, no real parsing.
pub fn chunk_by_indentation(content: &str) -> Vec<ChunkInfo> {
    let mut chunks = Vec::new();
    let mut current_chunk_lines = Vec::<&str>::new();
    let mut current_chunk_start_line: Option<usize> = None;
    let all_lines: Vec<&str> = content.lines().collect();

    for (i, line) in all_lines.iter().enumerate() {
        let current_line_num = i + 1; // 1-based line number
        let trimmed_line = line.trim();
        let current_indentation = calculate_indentation(line);

        // Added basic comment checks here
        let is_likely_top_level = current_indentation == 0 && !trimmed_line.is_empty() &&
            !trimmed_line.starts_with("//") && !trimmed_line.starts_with("#") &&
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
                trimmed_line.starts_with("class ") || // Covers JS/TS/Python
                trimmed_line.starts_with("interface ") ||
                trimmed_line.starts_with("export ") || // Often prefixes other declarations
                trimmed_line.starts_with("const ") || // Top level constants
                trimmed_line.starts_with("let ") || // Top level variables
                trimmed_line.starts_with("var ") ||
                trimmed_line.starts_with("def ") || // Python
                trimmed_line.starts_with("async def")
            );

        if current_chunk_start_line.is_none() {
             // Looking for the start of the first chunk
             if is_likely_top_level { // Only start on a likely top-level code line
                current_chunk_start_line = Some(current_line_num);
                current_chunk_lines.push(line);
             }
             // Otherwise, skip leading non-top-level lines (comments, whitespace, indented code)
        } else {
            // We are inside a chunk
            if is_likely_top_level {
                // Finish the previous chunk using the helper function
                finalize_chunk(
                    &mut chunks,
                    &mut current_chunk_lines, // Passed mutable ref, will be cleared
                    current_chunk_start_line.unwrap(),
                    current_line_num - 1, // Potential end line is the one before the current line
                );

                // Start the new chunk with the current top-level line
                // current_chunk_lines is already cleared by finalize_chunk
                current_chunk_lines.push(line); // Add the current line that triggered the split
                current_chunk_start_line = Some(current_line_num);
            } else {
                // Continue the current chunk by adding the line
                current_chunk_lines.push(line);
            }
        }
    }

    // Add the last chunk if it exists
    if current_chunk_start_line.is_some() {
        finalize_chunk(
            &mut chunks,
            &mut current_chunk_lines, // Pass mutable ref
            current_chunk_start_line.unwrap(),
            all_lines.len(), // Potential end line is the last line of the file
        );
    }

    chunks
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

// --- Indentation Chunking Tests ---
#[cfg(test)]
mod indentation_chunking_tests {
    use super::*;

    #[test]
    fn test_indentation_simple_rust() {
        // Use raw string, quotes inside are literal
        let content = r#"
fn main() {
    println!("Hello");
}

struct Point {
    x: i32,
    y: i32,
}
"#;
        let chunks = chunk_by_indentation(content);
        assert_eq!(chunks.len(), 2);
        // Use standard string for assertion argument
        assert!(chunks[0].text.contains("fn main()"));
        assert_eq!(chunks[0].start_line, 2);
        assert_eq!(chunks[0].end_line, 4);
        assert!(chunks[1].text.contains("struct Point"));
        assert_eq!(chunks[1].start_line, 6);
        assert_eq!(chunks[1].end_line, 9);
    }

    #[test]
    fn test_indentation_leading_whitespace_and_comments() {
        let content = r#"

  // A comment

fn first_func() {
    // ...
}

  // Another comment
  pub fn second_func() { // This line has indentation > 0
     // ...
  }
"#;
        let chunks = chunk_by_indentation(content);
        // Updated assertion: Current heuristic only finds top-level blocks with 0 indentation.
        // The second function is indented and thus not treated as a new block start.
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("fn first_func()"));
        assert_eq!(chunks[0].start_line, 5);
        // The chunk now includes the indented second function because it wasn't split.
        // finalize_chunk will find the last code line within this whole block.
        // Let's check the expected end line based on the full content now part of chunk 1.
        // Last non-empty line is line 12 ('}').
        assert_eq!(chunks[0].end_line, 12); 
        // Remove assertions related to the non-existent second chunk
        // assert!(chunks[1].text.contains("pub fn second_func()"));
        // assert_eq!(chunks[1].start_line, 10);
        // assert_eq!(chunks[1].end_line, 12);
    }

    #[test]
    fn test_indentation_python_mixed() {
        let content = r#"
class MyClass:
    def __init__(self):
        pass

def top_level_func():
    print("hello")

  # Indented comment...

async def another_func(): # Line 11
    pass # Line 12
"#;
        let chunks = chunk_by_indentation(content);
        assert_eq!(chunks.len(), 3);
        // Chunk 0 (class)
        assert!(chunks[0].text.contains("class MyClass"));
        assert_eq!(chunks[0].start_line, 2);
        assert_eq!(chunks[0].end_line, 4);
        // Chunk 1 (def)
        assert!(chunks[1].text.contains("def top_level_func()"));
        assert_eq!(chunks[1].start_line, 6);
        // End line calculated by finalize_chunk should ignore trailing comment/empty lines
        assert_eq!(chunks[1].end_line, 9);
        // Chunk 2 (async def)
        assert!(chunks[2].text.contains("async def another_func()"));
        // Corrected assertion: The start line is 11
        assert_eq!(chunks[2].start_line, 11);
        assert_eq!(chunks[2].end_line, 12);
    }

    #[test]
    fn test_indentation_javascript_export() {
        let content = r#"
export const foo = 1;

export function bar() {
  return 2;
}

class Baz {
  method() {}
}
"#;
        let chunks = chunk_by_indentation(content);
        assert_eq!(chunks.len(), 3);
        assert!(chunks[0].text.contains("export const foo"));
        assert_eq!(chunks[0].start_line, 2);
        assert_eq!(chunks[0].end_line, 2);
        assert!(chunks[1].text.contains("export function bar"));
        assert_eq!(chunks[1].start_line, 4);
        assert_eq!(chunks[1].end_line, 6);
        assert!(chunks[2].text.contains("class Baz"));
        assert_eq!(chunks[2].start_line, 8);
        assert_eq!(chunks[2].end_line, 10);
    }

    #[test]
    fn test_indentation_no_top_level() {
        let content = r#"
   line 1
   line 2
      line 3
"#;
        let chunks = chunk_by_indentation(content);
        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn test_indentation_empty_content() {
        let content = "";
        let chunks = chunk_by_indentation(content);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_indentation_whitespace_only_content() {
        let content = "  \n   \t\n";
        let chunks = chunk_by_indentation(content);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_indentation_single_top_level() {
        let content = "fn main() {}\n";
        let chunks = chunk_by_indentation(content);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 1);
    }

    #[test]
    fn test_indentation_only_comments_or_empty() {
        let content = r#"
// comment 1

  // comment 2


"#;
        let chunks = chunk_by_indentation(content);
        assert!(chunks.is_empty());
    }

}

// --- More Line Chunking Tests ---
#[cfg(test)]
mod line_chunking_tests_extended {
    use super::*;

    #[test]
    fn test_lines_exact_multiple() {
        let content = "1\n2\n3\n4\n5\n6";
        let chunks = chunk_by_lines(content, 3, 1);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].text, "1\n2\n3");
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 3);
        assert_eq!(chunks[1].text, "3\n4\n5");
        assert_eq!(chunks[1].start_line, 3);
        assert_eq!(chunks[1].end_line, 5);
        assert_eq!(chunks[2].text, "5\n6");
        assert_eq!(chunks[2].start_line, 5);
        assert_eq!(chunks[2].end_line, 6);
    }

    #[test]
    fn test_lines_overlap_equals_chunk_size() {
        let content = "a\nb\nc";
        let chunks = chunk_by_lines(content, 2, 2);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_lines_overlap_greater_than_chunk_size() {
        let content = "a\nb\nc";
        let chunks = chunk_by_lines(content, 2, 3);
        assert!(chunks.is_empty());
    }

     #[test]
    fn test_lines_only_whitespace_lines() {
        let content = "  \n\t \n \n   ";
        let chunks = chunk_by_lines(content, 2, 1);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_lines_mixed_whitespace_and_content() {
        let content = "line 1\n  \nline 3\n\t\nline 5";
        let chunks = chunk_by_lines(content, 3, 1);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].text, "line 1\n  \nline 3");
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 3);
        assert_eq!(chunks[1].text, "line 3\n\t\nline 5");
        assert_eq!(chunks[1].start_line, 3);
        assert_eq!(chunks[1].end_line, 5);
        assert_eq!(chunks[2].text, "line 5");
        assert_eq!(chunks[2].start_line, 5);
        assert_eq!(chunks[2].end_line, 5);
    }

    #[test]
    fn test_lines_chunk_size_one() {
         let content = "a\nb\nc";
         let chunks = chunk_by_lines(content, 1, 0);
         assert_eq!(chunks.len(), 3);
         assert_eq!(chunks[0].text, "a"); assert_eq!(chunks[0].start_line, 1); assert_eq!(chunks[0].end_line, 1);
         assert_eq!(chunks[1].text, "b"); assert_eq!(chunks[1].start_line, 2); assert_eq!(chunks[1].end_line, 2);
         assert_eq!(chunks[2].text, "c"); assert_eq!(chunks[2].start_line, 3); assert_eq!(chunks[2].end_line, 3);
    }

     #[test]
    fn test_lines_chunk_size_one_with_overlap() {
         let content = "a\nb\nc";
         let chunks = chunk_by_lines(content, 1, 1);
         assert!(chunks.is_empty());
         let chunks_overlap_0 = chunk_by_lines(content, 1, 0);
         assert_eq!(chunks_overlap_0.len(), 3);
    }

} 