use anyhow::{Context, Result};
use std::error::Error as StdError;
use tree_sitter::{Node, Parser, Query, QueryCursor};

// Use super::parser instead of crate::syntax::parser
use super::parser::{CodeChunk, SyntaxParser};

// Reuse the constant from the fallback parser or define locally if preferred
const MAX_CHUNK_LINES: usize = 500;
const MAX_SECTION_SIZE: usize = 3000; // Characters limit for sections before splitting. Increased to match test.

/// Represents a heading in a markdown document
#[derive(Debug, Clone)]
struct MarkdownHeading {
    level: usize,         // Heading level (1-6)
    content: String,      // Heading content
    start_line: usize,    // Start line in the document
    end_line: usize,      // End line in the document
}

/// Represents a content element in a markdown document
#[derive(Debug, Clone)]
struct MarkdownElement {
    content: String,      // Element content
    element_type: String, // Element type (paragraph, code_block, list_item, etc.)
    start_line: usize,    // Start line in the document
    end_line: usize,      // End line in the document
}

/// Represents a section in the document
#[derive(Debug, Clone)]
struct MarkdownSection {
    heading: Option<MarkdownHeading>,  // Section heading (optional)
    elements: Vec<MarkdownElement>,    // Content elements
    start_line: usize,                 // Start line
    end_line: usize,                   // End line
    parent_headings: Vec<MarkdownHeading>, // Parent headings for context
}

impl MarkdownSection {
    /// Creates a new section
    fn new(heading: Option<MarkdownHeading>, parent_headings: Vec<MarkdownHeading>) -> Self {
        let (start_line, end_line) = match &heading {
            Some(h) => (h.start_line, h.end_line),
            None => (1, 1),
        };
        
        MarkdownSection {
            heading,
            elements: Vec::new(),
            start_line,
            end_line,
            parent_headings,
        }
    }
    
    /// Adds a content element to this section
    fn add_element(&mut self, element: MarkdownElement) {
        // Update section end line if needed
        if element.end_line > self.end_line {
            self.end_line = element.end_line;
        }
        self.elements.push(element);
    }
    
    /// Converts this section to a code chunk
    fn to_chunks(&self, file_path: &str) -> Vec<CodeChunk> {
        if self.elements.is_empty() && self.heading.is_none() {
            return Vec::new();
        }
        
        // Build the content of elements within this section *only*
        let mut elements_only_content = String::new();
        if let Some(heading) = &self.heading {
             let heading_prefix = "#".repeat(heading.level);
             // Add heading marker and content only, no extra newlines here
             elements_only_content.push_str(&format!("{} {}\n", heading_prefix, heading.content)); 
        }
        for element in &self.elements {
            elements_only_content.push_str(&element.content);
            elements_only_content.push_str("\n"); // Use single newline separator
        }
        let elements_only_content = elements_only_content.trim().to_string();
        
        // Determine the base element type for this section
        let base_element_type = if let Some(h) = &self.heading {
            format!("h{}_section", h.level)
        } else {
             "root_section".to_string() // Default to root section if no heading
        };

        // Split the elements_only_content if it exceeds the size limit
        let mut element_chunks_content = Vec::new();
        let is_split = elements_only_content.len() > MAX_SECTION_SIZE;
        
        if is_split {
            log::debug!(
                "Section starting line {} is too large ({} chars), splitting into chunks <= {}.", 
                self.start_line, elements_only_content.len(), MAX_SECTION_SIZE
            );
            let mut current_pos = 0;
            while current_pos < elements_only_content.len() {
                let remaining_len = elements_only_content.len() - current_pos;
                let end_pos = if remaining_len <= MAX_SECTION_SIZE {
                    elements_only_content.len()
                } else {
                    // Find the last newline within the limit to avoid splitting mid-line
                    match elements_only_content[current_pos..current_pos + MAX_SECTION_SIZE].rfind('\n') {
                        Some(newline_pos) => current_pos + newline_pos + 1, // Split after newline
                        None => current_pos + MAX_SECTION_SIZE, // Split at limit if no newline found
                    }
                };
                let chunk_str = elements_only_content[current_pos..end_pos].trim();
                if !chunk_str.is_empty() {
                    element_chunks_content.push(chunk_str.to_string());
                }
                current_pos = end_pos;
            }
            if element_chunks_content.is_empty() {
                 log::warn!("Splitting large section resulted in zero element chunks. Original len: {}", elements_only_content.len());
                 // Add the whole thing if splitting failed somehow
                 element_chunks_content.push(elements_only_content.clone());
             }
        } else {
            // No splitting needed, use the whole content
            element_chunks_content.push(elements_only_content);
        }
        
        // Now, build the final CodeChunk objects, adding parent context to each
        let mut final_chunks = Vec::new();
        let total_chunks = element_chunks_content.len();
        let mut current_chunk_start_line = self.start_line;
        
        for (i, element_chunk_str) in element_chunks_content.iter().enumerate() {
             // Build the full content WITH parent context for this specific chunk
            let mut full_chunk_content = String::new();
            
            // Include parent headings for context in all chunks
            for parent_heading in &self.parent_headings {
                let heading_prefix = "#".repeat(parent_heading.level);
                full_chunk_content.push_str(&format!("{} {}\n\n", heading_prefix, parent_heading.content));
            }
            
            // For split chunks, only include the actual heading in the first chunk
            let is_first_chunk = i == 0;
            
            // Don't repeat the main heading in subsequent chunks if this is a split section
            if !is_split || is_first_chunk || self.heading.is_none() {
                full_chunk_content.push_str(element_chunk_str);
            } else {
                // For split sections (after the first chunk), don't include the heading line
                let lines: Vec<&str> = element_chunk_str.lines().collect();
                let content_without_heading = if !lines.is_empty() && lines[0].starts_with('#') {
                    lines[1..].join("\n")
                } else {
                    element_chunk_str.to_string()
                };
                full_chunk_content.push_str(&content_without_heading);
            }
            
            full_chunk_content = full_chunk_content.trim().to_string();
            
            let chunk_line_count = element_chunk_str.lines().count().max(1); // At least 1 line
            let chunk_end_line = current_chunk_start_line + chunk_line_count - 1;
            
            // For split chunks, assign sequential numbers from 1 to total_chunks
            let element_type = if is_split {
                format!("{}_split_{}", base_element_type, i + 1)
            } else {
                base_element_type.clone()
            };
            
            final_chunks.push(CodeChunk {
                content: full_chunk_content,
                file_path: file_path.to_string(),
                start_line: current_chunk_start_line,
                end_line: chunk_end_line,
                language: "markdown".to_string(),
                element_type,
            });
            
            // Update start line for the next chunk
            current_chunk_start_line = chunk_end_line + 1;
        }
        
        final_chunks
    }
}

pub struct MarkdownParser {
    parser: Parser,
    query: Query,
}

impl MarkdownParser {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        // Use tree-sitter-md
        let language = tree_sitter_md::language();
        parser
            .set_language(&language)
            .expect("Error loading Markdown (md) grammar");

        // Enhanced query to capture more markdown elements for better structure understanding
        let query = Query::new(
            &language,
            r#"
            [
              (atx_heading) @heading
              (setext_heading) @heading
              (fenced_code_block) @code_block
              (indented_code_block) @code_block
              (list_item) @list_item
              (paragraph) @paragraph
              (block_quote) @block_quote
              (document) @document
            ]
            "#,
        )
        .expect("Error creating Markdown (md) query");

        MarkdownParser { parser, query }
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
            },
            "setext_heading" => {
                // = underline is h1, - underline is h2
                if content.contains("\n===") {
                    1
                } else {
                    2
                }
            },
            _ => 0, // Not a heading
        }
    }

    /// Converts a tree-sitter node to a markdown element
    fn node_to_element(
        &self,
        node: Node,
        code: &str,
        _file_path: &str,
    ) -> Option<MarkdownElement> {
        let start_byte = node.start_byte();
        let end_byte = node.end_byte();
        let content = code.get(start_byte..end_byte)?.to_string();
        let start_line = node.start_position().row + 1;
        let end_line = node.end_position().row + 1;

        let node_kind = node.kind();

        // Map tree-sitter-md types to element types
        let element_type = match node_kind {
            "paragraph" => "paragraph",
            "fenced_code_block" => "code_block",
            "indented_code_block" => "code_block",
            "list_item" => "list_item",
            "block_quote" => "block_quote",
            "document" => "document",
            _ => "unknown",
        };

        // Filter out unknown types and empty content
        if element_type == "unknown" || content.trim().is_empty() {
            return None;
        }

        Some(MarkdownElement {
            content,
            element_type: element_type.to_string(),
            start_line,
            end_line,
        })
    }

    /// Converts a heading node to a MarkdownHeading
    fn node_to_heading(
        &self,
        node: Node,
        code: &str,
    ) -> Option<MarkdownHeading> {
        let level = self.get_heading_level(&node, code);
        if level == 0 {
            return None; // Not a valid heading
        }

        let start_line = node.start_position().row + 1;
        let end_line = node.end_position().row + 1;
        
        // Extract the heading text content correctly
        let heading_content_node = node.child_by_field_name("content").unwrap_or(node); // Fallback to node if no 'content' field
        let heading_text = code[heading_content_node.start_byte()..heading_content_node.end_byte()].trim();
        
        // Remove leading # and whitespace for ATX headings
        let final_text = if node.kind() == "atx_heading" {
            heading_text.trim_start_matches(|c: char| c == '#' || c.is_whitespace()).to_string()
        } else {
             heading_text.to_string()
        };

        Some(MarkdownHeading {
            level,
            content: final_text,
            start_line,
            end_line,
        })
    }

    /// Handles plain text splitting when no markdown structure is found
    fn handle_plain_text(&self, code: &str, file_path: &str) -> Vec<CodeChunk> {
        let lines: Vec<&str> = code.lines().collect();
        let mut fallback_chunks = Vec::new();
        let mut current_line_start = 1;
        
        // If there's no content after splitting into lines, return empty vec
        if lines.is_empty() {
            return fallback_chunks;
        }
        
        for (i, line_chunk) in lines.chunks(MAX_CHUNK_LINES).enumerate() {
            let content = line_chunk.join("\n");
            let start_line = current_line_start;
            let end_line = start_line + line_chunk.len() - 1;
            
            // Use plain element_type without index when there's only one chunk
            let element_type = if lines.len() <= MAX_CHUNK_LINES {
                "root_plain_text_chunk".to_string()
            } else {
                format!("root_plain_text_chunk_{}", i + 1)
            };
            
            fallback_chunks.push(CodeChunk {
                content,
                file_path: file_path.to_string(),
                start_line,
                end_line,
                language: "markdown".to_string(),
                element_type,
            });
            current_line_start = end_line + 1;
        }
        
        // Post-process: If there are multiple chunks, make sure they have correct split indices
        if fallback_chunks.len() > 1 {
            for (i, chunk) in fallback_chunks.iter_mut().enumerate() {
                chunk.element_type = format!("root_plain_text_chunk_split_{}", i + 1);
            }
        }
        
        fallback_chunks
    }
    
    /// Processes heading nodes recursively (placeholder)
    fn process_headings(&self, node: Node, code: &str) -> Vec<MarkdownHeading> {
        let mut headings = Vec::new();
        if node.kind() == "atx_heading" || node.kind() == "setext_heading" {
            if let Some(heading) = self.node_to_heading(node, code) {
                headings.push(heading);
            }
        }
        for child in node.children(&mut node.walk()) {
            headings.extend(self.process_headings(child, code));
        }
        headings
    }
    
    /// Extracts headings from the tree
    fn extract_headings(&self, tree: &tree_sitter::Tree, code: &str) -> Vec<MarkdownHeading> {
        self.process_headings(tree.root_node(), code)
    }

    /// Extracts paragraphs and converts them to CodeChunks
    fn extract_paragraphs(
        &self,
        tree: &tree_sitter::Tree,
        headings: &[MarkdownHeading],
        code: &str,
        language: tree_sitter::Language
    ) -> Vec<CodeChunk> {
        let mut paragraphs = Vec::new();
        let mut cursor = QueryCursor::new();
        let paragraph_query = match Query::new(&language, "(paragraph) @p") {
            Ok(q) => q,
            Err(e) => {
                log::error!("Failed to create paragraph query: {}", e);
                return paragraphs;
            }
        };
        let matches = cursor.matches(&paragraph_query, tree.root_node(), code.as_bytes());

        for mat in matches {
            if let Some(capture) = mat.captures.first() {
                let node = capture.node;
                let start_line = node.start_position().row + 1;
                let end_line = node.end_position().row + 1;
                let content = code[node.start_byte()..node.end_byte()].trim().to_string();

                // Skip if empty after trimming
                if content.is_empty() {
                    continue;
                }
                
                // Find parent heading context
                let mut parent_context = String::new();
                let mut current_level = 7; // Start higher than max level
                for heading in headings.iter().rev() { // Check from nearest heading backwards
                    if heading.start_line < start_line && heading.level < current_level {
                         let prefix = "#".repeat(heading.level);
                         parent_context.insert_str(0, &format!("{} {}\n\n", prefix, heading.content));
                         current_level = heading.level;
                         if current_level == 1 { break; } // Stop at top level
                    }
                }
                
                let final_content = format!("{}{}", parent_context, content).trim().to_string();

                paragraphs.push(CodeChunk {
                    content: final_content,
                    file_path: String::new(), // Will be set later
                    start_line,
                    end_line,
                    language: "markdown".to_string(),
                    element_type: "paragraph".to_string(),
                });
            }
        }
        paragraphs
    }

    /// Extracts code blocks and converts them to CodeChunks
    fn extract_code_blocks(
        &self,
        tree: &tree_sitter::Tree,
        code: &str,
        language: tree_sitter::Language
    ) -> Vec<CodeChunk> {
        let mut code_blocks = Vec::new();
        let mut cursor = QueryCursor::new();
        let code_query = match Query::new(&language, "[(fenced_code_block) @cb (indented_code_block) @cb]") {
            Ok(q) => q,
            Err(e) => {
                log::error!("Failed to create code block query: {}", e);
                return code_blocks;
            }
        };
        let matches = cursor.matches(&code_query, tree.root_node(), code.as_bytes());

        for mat in matches {
            if let Some(capture) = mat.captures.first() {
                let node = capture.node;
                let start_line = node.start_position().row + 1;
                let end_line = node.end_position().row + 1;
                let mut content = code[node.start_byte()..node.end_byte()].to_string();
                let mut element_type = "code_block".to_string();

                if node.kind() == "fenced_code_block" {
                    // Attempt to extract language from info string
                    if let Some(info_node) = node.child_by_field_name("info_string") {
                         let lang_str = code[info_node.start_byte()..info_node.end_byte()].trim();
                         if !lang_str.is_empty() {
                             element_type = format!("code_block_{}", lang_str);
                         }
                    }
                    // Extract content between fences
                    if let Some(content_node) = node.child_by_field_name("content") {
                        content = code[content_node.start_byte()..content_node.end_byte()].to_string();
                    }
                }
                
                code_blocks.push(CodeChunk {
                    content: content.trim().to_string(),
                    file_path: String::new(), // Will be set later
                    start_line,
                    end_line,
                    language: "markdown".to_string(),
                    element_type,
                });
            }
        }
        code_blocks
    }

    /// Extracts list items and converts them to CodeChunks
    fn extract_lists(
        &self,
        tree: &tree_sitter::Tree,
        code: &str,
        language: tree_sitter::Language
    ) -> Vec<CodeChunk> {
        let mut list_chunks = Vec::new();
        let mut cursor = QueryCursor::new();
        let list_query = match Query::new(&language, "(list_item) @li") {
            Ok(q) => q,
            Err(e) => {
                log::error!("Failed to create list query: {}", e);
                return list_chunks;
            }
        };
        let matches = cursor.matches(&list_query, tree.root_node(), code.as_bytes());

        for mat in matches {
            if let Some(capture) = mat.captures.first() {
                let node = capture.node;
                let start_line = node.start_position().row + 1;
                let end_line = node.end_position().row + 1;
                let content = code[node.start_byte()..node.end_byte()].trim().to_string();

                // Skip if empty after trimming
                if content.is_empty() {
                    continue;
                }
                
                list_chunks.push(CodeChunk {
                    content,
                    file_path: String::new(), // Will be set later
                    start_line,
                    end_line,
                    language: "markdown".to_string(),
                    element_type: "list_item".to_string(),
                });
            }
        }
        list_chunks
    }
    
    /// Direct conversion from Markdown to chunks without section hierarchy
    fn direct_markdown_to_chunks(
        &mut self, 
        code: &str, 
        file_path: &str,
        language: tree_sitter::Language
    ) -> Result<Vec<CodeChunk>> {
         log::debug!("Running direct_markdown_to_chunks for file: {}", file_path);
         // Parse the markdown using tree-sitter
         let tree = self.parser.parse(code, None)
             .context("Failed to parse markdown content in direct_markdown_to_chunks")?;
        
         let mut chunks = Vec::new();
         let mut cursor = QueryCursor::new();

         // Query for headings, code blocks, paragraphs, list items, block quotes
         let content_query = Query::new(
             &language, 
             r#"
             [
               (atx_heading) @item
               (setext_heading) @item
               (fenced_code_block) @item
               (indented_code_block) @item
               (list_item) @item
               (paragraph) @item
               (block_quote) @item
             ]
             "#,
         ).expect("Error creating direct markdown content query");

         let matches = cursor.matches(&content_query, tree.root_node(), code.as_bytes());

         for mat in matches {
             if let Some(capture) = mat.captures.first() {
                 let node = capture.node;
                 let start_line = node.start_position().row + 1;
                 let end_line = node.end_position().row + 1;
                 let mut content = code[node.start_byte()..node.end_byte()].trim().to_string();
                 let mut language = "markdown".to_string();
                 let mut element_type = node.kind().to_string();

                 // Skip empty content
                 if content.is_empty() {
                     continue;
                 }

                 match node.kind() {
                     "atx_heading" | "setext_heading" => {
                         let level = self.get_heading_level(&node, code);
                         element_type = format!("heading_{}", level);
                         // Clean heading content
                         let heading_content_node = node.child_by_field_name("content").unwrap_or(node);
                         content = code[heading_content_node.start_byte()..heading_content_node.end_byte()].trim().to_string();
                         if node.kind() == "atx_heading" {
                             content = content.trim_start_matches(|c: char| c == '#' || c.is_whitespace()).to_string();
                         }
                     },
                     "fenced_code_block" => {
                         element_type = "code_block".to_string();
                         // Extract language and inner content
                         if let Some(info_node) = node.child_by_field_name("info_string") {
                             let lang_str = code[info_node.start_byte()..info_node.end_byte()].trim();
                             if !lang_str.is_empty() {
                                 language = lang_str.to_string();
                                 element_type = format!("code_block_{}", lang_str);
                             }
                         }
                         if let Some(content_node) = node.child_by_field_name("content") {
                             content = code[content_node.start_byte()..content_node.end_byte()].to_string();
                         } else {
                            // Handle case where content might be empty or missing
                             content = "".to_string(); 
                         }
                         content = content.trim().to_string(); // Trim the code content itself
                     },
                      "indented_code_block" => {
                         element_type = "code_block".to_string();
                         // Content is the whole node, trim it
                         content = content.trim().to_string();
                     },
                     "list_item" => {
                         element_type = "list_item".to_string();
                         // Content is the whole node, trim it
                         content = content.trim().to_string();
                     },
                     "paragraph" => {
                         element_type = "paragraph".to_string();
                         // Content is the whole node, trim it
                         content = content.trim().to_string();
                     },
                     "block_quote" => {
                         element_type = "block_quote".to_string();
                         // Content is the whole node, trim it
                         content = content.trim().to_string();
                     },
                     _ => { /* Ignore others */ }
                 }
                 
                 // Final check to skip empty content after processing
                 if content.is_empty() {
                     continue;
                 }

                 chunks.push(CodeChunk {
                     content,
                     file_path: file_path.to_string(),
                     start_line,
                     end_line,
                     language,
                     element_type,
                 });
             }
         }
         
         // Fallback if still no chunks
         if chunks.is_empty() && !code.trim().is_empty() {
            log::debug!("Direct markdown processing found no items, falling back to plain text chunking for {}", file_path);
            return Ok(self.handle_plain_text(code, file_path));
         }
         
         log::debug!("Direct markdown processing yielded {} chunks for {}", chunks.len(), file_path);
         Ok(chunks)
     }

    fn to_chunks(
        &mut self, 
        code: &str, 
        file_path: &str,
        language: tree_sitter::Language
    ) -> Result<Vec<CodeChunk>, Box<dyn StdError>> {
        log::debug!("Converting markdown content to chunks for file: {}", file_path);
        log::trace!("Markdown content: \n{}", code);

        // Parse the markdown using tree-sitter
        let tree = self.parser.parse(code, None)
            .context("Failed to parse markdown content")?;
        
        // Extract all headings with their positions
        let headings = self.extract_headings(&tree, code);
        log::debug!("Found {} headings", headings.len());

        // Create section chunks based on heading hierarchy
        let section_chunks = self.create_section_hierarchy(&headings, code, file_path, language.clone());
        log::debug!("Created {} section chunks from hierarchy", section_chunks.len());

        // If no heading sections were created and the code is not empty, treat as plain text immediately.
        if section_chunks.is_empty() && code.trim().len() > 0 {
             log::debug!("No heading sections found, treating content as plain text root section");
            return Ok(self.handle_plain_text(code, file_path));
        }

        // Extract paragraphs, code blocks, and lists that are *not* part of sections
        let mut standalone_elements = Vec::new();
        let mut captured_lines = std::collections::HashSet::new();
        for chunk in &section_chunks {
             for line in chunk.start_line..=chunk.end_line {
                 captured_lines.insert(line);
             }
         }

        let paragraphs = self.extract_paragraphs(&tree, &headings, code, language.clone());
        let code_blocks = self.extract_code_blocks(&tree, code, language.clone());
        let list_chunks = self.extract_lists(&tree, code, language.clone());
        
        for p in paragraphs {
            // Check if *any* part of the element is outside captured lines
            if (p.start_line..=p.end_line).any(|line| !captured_lines.contains(&line)) {
                standalone_elements.push(p);
            }
        }
         for b in code_blocks {
            if (b.start_line..=b.end_line).any(|line| !captured_lines.contains(&line)) {
                standalone_elements.push(b);
            }
        }
         for l in list_chunks {
            if (l.start_line..=l.end_line).any(|line| !captured_lines.contains(&line)) {
                standalone_elements.push(l);
            }
        }
        log::debug!(
            "Extracted {} standalone elements (paragraphs, code blocks, lists) not within sections", 
            standalone_elements.len()
        );
        
        // Combine section chunks and standalone elements
        let mut all_chunks: Vec<CodeChunk> = section_chunks;
        all_chunks.extend(standalone_elements);
        
        // This check is likely redundant now but kept as a safeguard
        if all_chunks.is_empty() && code.trim().len() > 0 {
            log::warn!("All chunks were empty unexpectedly after processing sections and standalone elements. Falling back to plain text.");
            return Ok(self.handle_plain_text(code, file_path));
        }
        
        // Sort final chunks by start line
        all_chunks.sort_by_key(|c| c.start_line);

        // Simple final pass to assign file path
        for chunk in &mut all_chunks {
            chunk.file_path = file_path.to_string();
        }
        
        log::debug!("Finalizing with {} chunks for file: {}", all_chunks.len(), file_path);
        Ok(all_chunks)
    }

    /// Creates sections based on heading hierarchy
    fn create_section_hierarchy(
        &mut self, 
        headings: &[MarkdownHeading], 
        code: &str, 
        file_path: &str,
        language: tree_sitter::Language
    ) -> Vec<CodeChunk> {
        let mut sections = Vec::new();
        let mut heading_stack: Vec<MarkdownHeading> = Vec::new();
        let mut current_section: Option<MarkdownSection> = None;
        let mut last_processed_line = 0;

        // Parse the document to extract all elements
        let tree = match self.parser.parse(code, None) {
            Some(t) => t,
            None => {
                log::error!("Failed to parse markdown for section hierarchy.");
                return Vec::new();
            }
        };
        let mut cursor = QueryCursor::new();
        let all_elements_query = Query::new(
            &language, 
            r#"
            [
              (paragraph) @element
              (fenced_code_block) @element
              (indented_code_block) @element
              (list_item) @element
              (block_quote) @element
              (atx_heading) @heading
              (setext_heading) @heading
            ]
            "#,
        ).expect("Error creating element query");
        
        let matches = cursor.matches(&all_elements_query, tree.root_node(), code.as_bytes());
        
        // Iterate through sorted matches (nodes)
        let mut sorted_nodes: Vec<Node> = matches.flat_map(|m| m.captures.iter().map(|c| c.node)).collect();
        sorted_nodes.sort_by_key(|n| n.start_byte());
        
        // Handle content before the first heading
        let mut root_elements = Vec::new();
        let first_heading_start_line = headings.first().map_or(usize::MAX, |h| h.start_line);
        
        for node in &sorted_nodes {
             let start_line = node.start_position().row + 1;
             let is_heading = node.kind() == "atx_heading" || node.kind() == "setext_heading";
             
             if start_line < first_heading_start_line && !is_heading {
                 if let Some(element) = self.node_to_element(*node, code, file_path) {
                      root_elements.push(element.clone());
                      last_processed_line = element.end_line;
                 }
             } else {
                  break; // Stop once we reach the first heading or beyond
             }
         }
         
         if !root_elements.is_empty() {
             let mut root_section = MarkdownSection::new(None, Vec::new());
             root_section.elements = root_elements;
             root_section.end_line = last_processed_line;
             sections.push(root_section);
         }
        
        // Process headings and the elements under them
        let mut element_iter = sorted_nodes.iter().peekable();
        
        for heading in headings {
             // Finalize previous section if it exists
             if let Some(mut section) = current_section.take() {
                 // Add elements between the previous heading/start and this heading
                 while let Some(node) = element_iter.peek() {
                     let node_start_line = node.start_position().row + 1;
                     if node_start_line >= heading.start_line {
                         break; // Stop adding elements when we reach the current heading
                     }
                     if let Some(element) = self.node_to_element(**node, code, file_path) {
                          if element.start_line > last_processed_line {
                             section.add_element(element);
                             last_processed_line = section.end_line;
                          }
                     }
                     element_iter.next(); // Consume the element
                 }
                 sections.push(section);
             }
             
            // Adjust heading stack
            while let Some(last_heading) = heading_stack.last() {
                if last_heading.level >= heading.level {
                    heading_stack.pop();
                } else {
                    break;
                }
            }
            
            // Start new section
            current_section = Some(MarkdownSection::new(Some(heading.clone()), heading_stack.clone()));
            last_processed_line = heading.end_line; // Update last processed line
            
            // Add current heading to stack
            heading_stack.push(heading.clone());
        }
        
        // Finalize the last section
        if let Some(mut section) = current_section.take() {
             // Add remaining elements
             while let Some(node) = element_iter.next() {
                 if let Some(element) = self.node_to_element(*node, code, file_path) {
                      if element.start_line > last_processed_line {
                         section.add_element(element);
                      }
                 }
             }
             sections.push(section);
         }
        
        // Convert sections to chunks
        sections.into_iter().flat_map(|s| s.to_chunks(file_path)).collect()
    }
}

impl SyntaxParser for MarkdownParser {
    fn parse(&mut self, code: &str, file_path: &str) -> Result<Vec<CodeChunk>> {
        // Handle empty or whitespace-only content right away
        if code.trim().is_empty() {
            return Ok(Vec::new());
        }
        
        // Get language ONCE here
        let language = tree_sitter_md::language(); // Assuming this is infallible or handled by expect in new()
        self.parser.set_language(&language).context("Failed to set markdown language")?; // Ensure parser has the language

        // Check if this is just plain text without markdown structure
        // If there are no headings and no special markdown elements, treat as plain text
        let is_plain_text = !code.contains('#') && 
                           !code.contains("```") && 
                           !code.contains('*') &&
                           !code.contains('-') &&
                           !code.contains('>');
        
        if is_plain_text {
            return Ok(self.handle_plain_text(code, file_path));
        }

        let _tree = match self.parser.parse(code, None) {
             Some(t) => t,
             None => return Ok(self.handle_plain_text(code, file_path)), // Fallback if initial parse fails
        };

        // Call internal logic, passing the language
        match self.to_chunks(code, file_path, language) {
            Ok(chunks) => Ok(chunks),
            Err(e) => {
                log::error!("Error converting markdown to chunks: {}. Falling back.", e);
                // Fallback to line-based chunking if to_chunks fails
                Ok(self.handle_plain_text(code, file_path))
            }
        }
    }
}

#[cfg(test)]
#[path = "markdown_tests.rs"]
mod tests; 