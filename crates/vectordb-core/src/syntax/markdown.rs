use anyhow::{Context, Result};
use std::error::Error as StdError;
use tree_sitter::{Node, Parser, Query, QueryCursor};

// Use super::parser instead of crate::syntax::parser
use super::parser::{CodeChunk, SyntaxParser};

// Reuse the constant from the fallback parser or define locally if preferred
const MAX_CHUNK_LINES: usize = 500; // Max lines for plain text fallback chunks
const MAX_SECTION_SIZE: usize = 3000; // Character limit for content within a single section chunk before splitting

/// Represents a heading identified in the markdown document.
#[derive(Debug, Clone)]
struct MarkdownHeading {
    level: usize,      // Heading level (1-6)
    content: String,   // Heading content (cleaned)
    start_line: usize, // Start line in the document (1-based)
    end_line: usize,   // End line in the document (1-based)
}

pub struct MarkdownParser {
    parser: Parser,
    // Query might not be needed directly if extract_headings uses its own
}

impl MarkdownParser {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        // Use tree-sitter-md
        let language = tree_sitter_md::language();
        parser
            .set_language(&language)
            .expect("Error loading Markdown (md) grammar");

        // No need for the general query here anymore
        MarkdownParser { parser }
    }

    /// Identifies the heading level of a node
    fn get_heading_level(&self, node: &Node, code: &str) -> usize {
        let node_kind = node.kind();
        let content = code[node.start_byte()..node.end_byte()].to_string();

        match node_kind {
            "atx_heading" => {
                // Count the number of # characters at the start
                let hash_count = content.chars().take_while(|&c| c == '#').count();
                hash_count.clamp(1, 6)
            }
            "setext_heading" => {
                // Extract the underline node to determine level
                let underline_node = node.children(&mut node.walk())
                                         .find(|n| n.kind() == "setext_h1_underline" || n.kind() == "setext_h2_underline");
                match underline_node {
                    Some(n) if n.kind() == "setext_h1_underline" => 1,
                    Some(n) if n.kind() == "setext_h2_underline" => 2,
                    _ => {
                        // Fallback/guess if underline node isn't found as expected
                        if content.contains("
===") { 1 } else { 2 }
                    }
                }
            }
            _ => 0, // Not a heading
        }
    }

    /// Converts a heading node to a MarkdownHeading
    fn node_to_heading(&self, node: Node, code: &str) -> Option<MarkdownHeading> {
        let level = self.get_heading_level(&node, code);
        if level == 0 {
            log::trace!("node_to_heading: Node at line {} is not a heading (level 0). Kind: {}", node.start_position().row + 1, node.kind());
            return None; // Not a valid heading
        }

        // Extract the heading text content correctly, excluding markers/underlines
        let heading_content_node = match node.kind() {
            "atx_heading" => node.child_by_field_name("content").unwrap_or(node),
            "setext_heading" => node.child_by_field_name("content").unwrap_or(node), // Assuming 'content' field exists or fallback needed
            _ => node, // Should not happen if level > 0
        };
        
        // Determine start/end lines based on the *content* node primarily
        let start_line = heading_content_node.start_position().row + 1;
        // For ATX, the end line is the same as the content start line.
        // For Setext, the actual node end line includes the underline.
        let end_line = if node.kind() == "atx_heading" {
            start_line 
        } else { // setext_heading
            node.end_position().row + 1 // Use the full node end for Setext
        };

        // Use the content node's byte range if available, otherwise approximate
        let raw_text = code[heading_content_node.start_byte()..heading_content_node.end_byte()].trim();

        // Remove leading # and whitespace for ATX headings
        let final_text = if node.kind() == "atx_heading" {
            raw_text
                .trim_start_matches(|c: char| c == '#' || c.is_whitespace())
                .to_string()
        } else {
            // Setext headings might have the underline included if 'content' field isn't precise, clean it
            raw_text.lines().next().unwrap_or("").trim().to_string()
        };

        Some(MarkdownHeading {
            level,
            content: final_text,
            start_line,
            end_line,
        })
    }
    
    /// Extracts headings from the tree using a specific query.
    fn extract_headings(&mut self, tree: &tree_sitter::Tree, code: &str) -> Result<Vec<MarkdownHeading>> {
        let language = tree_sitter_md::language(); // Get language

        // Combined query
        let headings_query = Query::new(
            &language,
             r#"
             [
               (atx_heading) @heading
               (setext_heading) @heading
             ]
             "#,
        ).context("Failed to create heading query")?;
        log::debug!("extract_headings: Using combined ATX/Setext query.");
        
        let mut cursor = QueryCursor::new();

        let matches = cursor.matches(&headings_query, tree.root_node(), code.as_bytes());
        
        let mut headings = Vec::new();
        for mat in matches {
             // We only expect one capture per match in this query
             if let Some(capture) = mat.captures.first() {
                 let node = capture.node;
                 // Log the kind of node found by the query
                 log::debug!("extract_headings: Matched node kind: '{}' at line {}", node.kind(), node.start_position().row + 1);
                 if node.kind() == "setext_heading" {
                     log::debug!("extract_headings: Found Setext node kind: {}, line: {}", node.kind(), node.start_position().row + 1);
                 }
                 if let Some(heading) = self.node_to_heading(capture.node, code) {
                     if heading.level == 1 || heading.level == 2 { // Extra check for setext typical levels
                         let kind = if code[node.start_byte()..node.end_byte()].contains("===") { "Setext H1" } else { "Setext H2" };
                         log::debug!("extract_headings: Converted node to {} heading: {:?}", kind, heading);
                     }
                     headings.push(heading);
                 }
             }
        }
        log::debug!("extract_headings: Finished extraction. Total headings found: {}", headings.len());
        // Ensure headings are sorted by start line, though tree-sitter usually gives them in order
        headings.sort_by_key(|h| h.start_line);
        Ok(headings)
    }


    /// Handles plain text splitting when no markdown structure is found or parsing fails.
    /// Splits purely by MAX_CHUNK_LINES.
    fn handle_plain_text(&self, code: &str, file_path: &str) -> Vec<CodeChunk> {
        log::debug!("Handling content as plain text for file: {}", file_path);
        let lines: Vec<&str> = code.lines().collect();
        let mut fallback_chunks = Vec::new();
        let mut current_line_start = 1;

        if lines.is_empty() {
            return fallback_chunks;
        }

        let total_lines = lines.len();
        let mut line_idx = 0;

        while line_idx < total_lines {
            let chunk_end_line_idx = (line_idx + MAX_CHUNK_LINES).min(total_lines);
            let line_chunk = &lines[line_idx..chunk_end_line_idx];
            let content = line_chunk.join("
");
            let start_line = current_line_start;
            let end_line = start_line + line_chunk.len() - 1;

            let element_type = if total_lines <= MAX_CHUNK_LINES {
                log::debug!(
                    "handle_plain_text: Creating single chunk. Start: {}, End: {}, Lines: {}, Content Preview: '{}'",
                    start_line, end_line, line_chunk.len(), content.chars().take(50).collect::<String>()
                );
                // Use a simpler type if the whole file fits in one chunk
                 "root_plain_text".to_string()
            } else {
                log::debug!(
                    "handle_plain_text: Creating split chunk {}. Start: {}, End: {}, Lines: {}, Content Preview: '{}'",
                    fallback_chunks.len() + 1, start_line, end_line, line_chunk.len(), content.chars().take(50).collect::<String>()
                );
                // Indicate splitting for multiple chunks
                format!("root_plain_text_split_{}", fallback_chunks.len() + 1)
            };

            fallback_chunks.push(CodeChunk {
                content,
                file_path: file_path.to_string(),
                start_line,
                end_line,
                language: "markdown".to_string(), // Still markdown, just unstructured
                element_type,
            });

            current_line_start = end_line + 1;
            line_idx = chunk_end_line_idx;
        }
        
        log::debug!("Plain text handling yielded {} chunks for {}", fallback_chunks.len(), file_path);
        fallback_chunks
    }

    /// Builds CodeChunks based on heading sections, including parent context.
    fn build_section_chunks(
        &self,
        code_lines: &[&str], // Pass code lines to avoid repeated splitting
        total_lines: usize,
        headings: &[MarkdownHeading],
        file_path: &str,
    ) -> Vec<CodeChunk> {
        let mut chunks = Vec::new();
        let mut heading_stack: Vec<&MarkdownHeading> = Vec::new();

        let mut current_pos = 0; // Current line index (0-based)

        // 1. Handle content before the first heading
        let first_heading_start_line = headings.first().map_or(usize::MAX, |h| h.start_line);
        if first_heading_start_line > 1 {
             // Calculate the end line (0-based index) for the root content
             let root_end_line_idx = (first_heading_start_line - 1).min(total_lines);
             if root_end_line_idx > current_pos {
                 let root_content_lines = &code_lines[current_pos..root_end_line_idx];
                 let root_content = root_content_lines.join("
").trim().to_string();
                 if !root_content.is_empty() {
                     // Split root content if necessary
                     self.split_and_add_chunk(
                         &mut chunks,
                         None, // No current heading for root content
                         &root_content,
                         file_path,
                         1, // Root starts at line 1
                         root_end_line_idx, // End line (1-based)
                         "root_content".to_string(),
                         &[], // No parent headings for root
                     );
                     current_pos = root_end_line_idx;
                 }
             }
         } else if first_heading_start_line == usize::MAX && !code_lines.iter().all(|&s| s.is_empty()) {
             // No headings at all, but there is content. The calling 'parse' should handle this via handle_plain_text,
             // but as a safeguard, we could process it here too. Let's rely on 'parse'.
             log::debug!("No headings found, build_section_chunks returning empty, expecting handle_plain_text fallback.");
             return chunks; // Or call handle_plain_text directly? Let's stick to the plan.
         }


        // 2. Process each heading and the content following it
        for (i, heading) in headings.iter().enumerate() {
            // Update heading stack (pop higher/equal levels)
            log::debug!("build_section_chunks: Processing heading: {:?}", heading);
            while let Some(last_heading) = heading_stack.last() {
                if last_heading.level >= heading.level {
                    heading_stack.pop();
                } else {
                    break;
                }
            }

            // Determine the end line of this section's content
            let next_heading_start_line = if i + 1 < headings.len() {
                headings[i + 1].start_line
            } else {
                total_lines + 1 // Go to end of file
            };

            // Content starts after the current heading's end line
            // Section content ends just before the next heading starts
            let content_start_line = heading.end_line + 1;
            let content_end_line = next_heading_start_line - 1; // Inclusive end line (1-based)

            log::debug!(
                "build_section_chunks: Heading '{}' (L{}) - H.Start={}, H.End={}, NextH.Start={}, ContentStart={}, ContentEnd={}", 
                heading.content,
                heading.level, 
                heading.start_line, 
                heading.end_line, 
                next_heading_start_line, 
                content_start_line, 
                content_end_line
            );

            // Ensure start is not after end
            if content_start_line <= content_end_line {
                 // Get the actual content lines (0-based index)
                 let content_start_idx = (content_start_line - 1).min(total_lines);
                 let content_end_idx = (content_end_line).min(total_lines); // Exclusive end for slicing
                 
                 if content_start_idx < content_end_idx { // Check if there are lines to slice
                     let section_content_lines = &code_lines[content_start_idx..content_end_idx];
                     let section_content = section_content_lines.join("
").trim().to_string();
                     
                     // Only create a chunk if there's actual content in the section
                     if !section_content.is_empty() {
                          self.split_and_add_chunk(
                             &mut chunks,
                             Some(heading), // Pass the current heading
                             &section_content,
                             file_path,
                             heading.start_line, // Chunk logically starts at the heading
                             content_end_line,   // Ends where the content ends
                             format!("h{}_section", heading.level),
                             &heading_stack,
                         );
                     } else {
                         // Handle heading-only chunk if needed? Decide against for now.
                         // We could add a chunk containing only the heading + parents here if desired.
                         // For now, sections require content following the heading.
                         log::trace!("Skipping chunk for heading at line {} as it has no subsequent content before next heading/EOF.", heading.start_line);
                     }
                 } else {
                     log::trace!("No content lines between heading at {} and next heading/EOF at {}", heading.start_line, next_heading_start_line);
                 }
            } else {
                 log::trace!("Heading at line {} has no content lines immediately following it before next heading/EOF at {}", heading.start_line, next_heading_start_line);
            }

            // Push current heading onto the stack for subsequent sections
            heading_stack.push(heading);
            // Update current_pos: section content ends at content_end_line (1-based)
            current_pos = content_end_line.min(total_lines);
        }

        log::debug!("build_section_chunks created {} chunks for {}", chunks.len(), file_path);
        chunks
    }

    /// Helper to build the context string from parent headings.
    fn build_parent_context(parents: &[&MarkdownHeading]) -> String {
        let mut context = String::new();
        for parent in parents {
            context.push_str(&"#".repeat(parent.level));
            context.push(' ');
            context.push_str(&parent.content);
            context.push_str("

"); // Double newline for separation
        }
        context
    }

    /// Helper to split large content and add chunk(s).
    fn split_and_add_chunk(
        &self,
        chunks: &mut Vec<CodeChunk>,
        current_heading: Option<&MarkdownHeading>, // Added: Pass the actual current heading (or None for root)
        section_content: &str, // Just the content *within* this section
        file_path: &str,
        section_start_line: usize, // Line number of the heading (or 1 for root)
        section_end_line: usize,   // Line number of the last line of content for this section
        base_element_type: String,
        parent_headings: &[&MarkdownHeading],
    ) {
        let parent_context = Self::build_parent_context(parent_headings);
        // Use the passed-in current_heading (if Some) to format the heading line
        let current_heading_line = match current_heading {
            Some(h) => format!("{} {}\n\n", "#".repeat(h.level), h.content), // Use h.level and h.content
            None => "".to_string(), // No heading line for root content (handled by base_element_type == "root_content")
        };
        
        let full_header = format!("{}{}", parent_context, current_heading_line);

        // Calculate the actual content slice *excluding* the headers
        let section_content_trimmed = section_content.trim();

        // Combine header with the actual content slice for this section
        let full_content_combined = format!("{}{}", full_header, section_content_trimmed);

        // Now, check size and split `full_content` if needed
        if full_content_combined.len() <= MAX_SECTION_SIZE {
            // Fits in one chunk
            log::debug!("Creating single chunk: Start={}, End={}, Type={}", section_start_line, section_end_line, base_element_type);
            chunks.push(CodeChunk {
                content: full_content_combined,
                file_path: file_path.to_string(),
                start_line: section_start_line, // Logical start is the heading (or line 1 for root)
                end_line: section_end_line,      // Use section_end_line which is calculated as next_heading_start_line - 1
                language: "markdown".to_string(),
                element_type: base_element_type,
            });
        } else {
            // Needs splitting
            log::debug!(
                "Section starting line {} (type {}) is too large ({} chars), splitting.",
                section_start_line, base_element_type, full_content_combined.len()
            );

            let mut remaining_content_to_split = section_content_trimmed; // Split the content part only
            let mut current_chunk_idx = 1;
            let mut current_chunk_start_line = section_start_line; // Start line for the first split chunk

            while !remaining_content_to_split.is_empty() {
                let chunk_element_type = format!("{}_split_{}", base_element_type, current_chunk_idx);
                
                let available_content_len = MAX_SECTION_SIZE.saturating_sub(full_header.len());
                if available_content_len == 0 {
                    log::error!(
                        "Header for section starting at line {} is longer ({}) than MAX_SECTION_SIZE ({}). Cannot split content.", 
                        section_start_line, full_header.len(), MAX_SECTION_SIZE
                    );
                    break; 
                }
                
                // --- START REVISED SPLIT LOGIC ---
                let mut current_chunk_content_part = String::new();
                let mut current_byte_len = 0;
                let mut last_newline_byte_pos: Option<usize> = None;
                let mut last_processed_byte_pos = 0;

                // Iterate by char indices to ensure we respect boundaries
                for (byte_idx, ch) in remaining_content_to_split.char_indices() {
                    let char_byte_len = ch.len_utf8();
                    
                    // Check if adding this character exceeds the available byte limit
                    if current_byte_len + char_byte_len > available_content_len {
                        // We need to split *before* this character.
                        // Check if we found a newline within the limit.
                        if let Some(newline_pos) = last_newline_byte_pos {
                             // Take content up to and including the newline
                             current_chunk_content_part = remaining_content_to_split[..=newline_pos].to_string();
                             last_processed_byte_pos = newline_pos + 1; // Next chunk starts after newline
                        } else {
                            // No newline found within the limit, split at the last safe character boundary
                            current_chunk_content_part = remaining_content_to_split[..last_processed_byte_pos].to_string(); 
                            // next chunk starts at last_processed_byte_pos
                        }
                        break; // Stop processing for this chunk
                    }

                    // Add the character to the current chunk part
                    // current_chunk_content_part.push(ch); // Inefficient for large strings
                    current_byte_len += char_byte_len;
                    last_processed_byte_pos = byte_idx + char_byte_len; // Update the end position

                    // Track the last newline position *within* the current byte limit
                    if ch == '\n' {
                        last_newline_byte_pos = Some(byte_idx);
                    }
                }

                // If the loop finished without breaking, the entire remaining content fits
                if last_processed_byte_pos == remaining_content_to_split.len() {
                     current_chunk_content_part = remaining_content_to_split.to_string();
                } else if current_chunk_content_part.is_empty() && last_processed_byte_pos > 0 {
                     // This happens if the first character itself exceeded the limit,
                     // or if available_content_len was very small.
                     // Take up to the last safe boundary before the character that broke the limit.
                     current_chunk_content_part = remaining_content_to_split[..last_processed_byte_pos].to_string();
                } else if current_chunk_content_part.is_empty() && last_processed_byte_pos == 0 {
                     // Edge case: available_content_len was 0 or negative (checked above), or first char invalid?
                     log::warn!("Unable to extract any content for split chunk {} in {}. Header length might be too close to MAX_SECTION_SIZE.", current_chunk_idx, file_path);
                     break; // Avoid infinite loop
                }


                // Use the determined content_part and update remaining_content_to_split
                let content_part = current_chunk_content_part.trim_end(); // Trim trailing whitespace
                let final_chunk_content = format!("{}{}", full_header, content_part);

                // Check if we actually consumed any bytes
                if last_processed_byte_pos == 0 && !remaining_content_to_split.is_empty() {
                     log::error!("Split logic failed to advance in chunk {}. File: {}, Heading Line: {}. Remaining content starts with: '{}'", 
                                current_chunk_idx, file_path, section_start_line, remaining_content_to_split.chars().take(20).collect::<String>());
                     break; // Prevent potential infinite loop
                }

                // Update remaining_content_to_split safely using the last processed byte position
                if last_processed_byte_pos >= remaining_content_to_split.len() {
                    remaining_content_to_split = "";
                } else {
                    // Ensure the start of the next slice is valid (should be, as it's based on char_indices)
                    remaining_content_to_split = remaining_content_to_split[last_processed_byte_pos..].trim_start();
                }
                // --- END REVISED SPLIT LOGIC ---


                // Line number calculation needs care. We only have the start/end of the original section.
                // Approximating based on chunk index for now.
                // TODO: Improve line number accuracy for split chunks.
                let lines_in_content_part = content_part.lines().count();
                let header_lines = full_header.lines().count(); // Approx lines in header
                 // Estimate start/end lines - this is tricky!
                 // The `section_start_line` refers to the original heading.
                 // The physical content starts later. Let's try to be slightly more accurate.
                 let physical_content_start_line = current_heading.map_or(1, |h| h.end_line + 1);
                 let chunk_start_line_approx = if current_chunk_idx == 1 { section_start_line } else { current_chunk_start_line };
                 // This end line calc is still flawed. 
                 let chunk_end_line_approx = chunk_start_line_approx + header_lines + lines_in_content_part - 1; 

                chunks.push(CodeChunk {
                    content: final_chunk_content,
                    file_path: file_path.to_string(),
                    start_line: chunk_start_line_approx, // Approximate start line
                    end_line: chunk_end_line_approx,     // Approximate end line
                    language: "markdown".to_string(),
                    element_type: chunk_element_type,
                });

                // The start line for the *next* chunk needs to relate to the previous end line.
                current_chunk_start_line = chunk_end_line_approx + 1; 
                current_chunk_idx += 1;
            }
        }
    }
}

impl SyntaxParser for MarkdownParser {
    fn parse(&mut self, code: &str, file_path: &str) -> Result<Vec<CodeChunk>> {
        log::debug!("Parsing markdown file: {}", file_path);
        // 1. Handle empty or whitespace-only content right away
        if code.trim().is_empty() {
            log::debug!("Content is empty, returning zero chunks.");
            return Ok(Vec::new());
        }

        // 2. Set language and parse
        let language = tree_sitter_md::language();
        self.parser
            .set_language(&language)
            .context("Failed to set markdown language for parsing")?;

        // Attempt to parse the document fully
        let tree = match self.parser.parse(code, None) {
            Some(t) => t,
            None => {
                log::warn!("Tree-sitter failed to parse {}, falling back to plain text handling.", file_path);
                // Fallback if initial parse fails
                return Ok(self.handle_plain_text(code, file_path));
            }
        };

        // 3. Extract headings
        let headings = match self.extract_headings(&tree, code) {
             Ok(h) => h,
             Err(e) => {
                 log::error!("Failed to extract headings from {}: {}. Falling back to plain text.", file_path, e);
                 return Ok(self.handle_plain_text(code, file_path));
             }
        };

        // 4. Determine chunking strategy
        if headings.is_empty() {
            // No headings found, treat as plain text
            log::debug!("No headings found in {}, using plain text handler.", file_path);
            Ok(self.handle_plain_text(code, file_path))
        } else {
            // Headings found, build chunks based on sections
            log::debug!("Found {} headings in {}. Building section chunks.", headings.len(), file_path);
            let code_lines: Vec<&str> = code.lines().collect();
            let total_lines = code_lines.len();
            let chunks = self.build_section_chunks(&code_lines, total_lines, &headings, file_path);
            log::debug!("Finalizing with {} chunks for file: {}", chunks.len(), file_path);
            Ok(chunks)
        }
    }
}

#[cfg(test)]
#[path = "markdown_tests.rs"]
mod tests; 