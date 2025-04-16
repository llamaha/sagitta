// src/syntax/markdown.rs
use anyhow::{Context, Result};
use tree_sitter::{Node, Parser, Query, QueryCursor};

use crate::syntax::parser::{CodeChunk, SyntaxParser};

// Reuse the constant from the fallback parser or define locally if preferred
const MAX_CHUNK_LINES: usize = 500;
const MAX_SECTION_SIZE: usize = 2000; // Characters limit for sections before splitting

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
        if self.elements.is_empty() {
            return Vec::new();
        }
        
        // Create the content with proper context
        let mut section_content = String::new();
        
        // Add parent headings for context
        for heading in &self.parent_headings {
            let heading_prefix = "#".repeat(heading.level);
            section_content.push_str(&format!("{} {}\n\n", heading_prefix, heading.content));
        }
        
        // Add this section's heading
        if let Some(heading) = &self.heading {
            let heading_prefix = "#".repeat(heading.level);
            section_content.push_str(&format!("{} {}\n\n", heading_prefix, heading.content));
        }
        
        // Add elements
        for element in &self.elements {
            section_content.push_str(&element.content);
            section_content.push_str("\n\n");
        }
        
        let section_content = section_content.trim().to_string();
        
        // If section is too large, split it
        if section_content.len() > MAX_SECTION_SIZE {
            let lines: Vec<&str> = section_content.lines().collect();
            let chunks_count = (section_content.len() as f64 / MAX_SECTION_SIZE as f64).ceil() as usize;
            let lines_per_chunk = (lines.len() as f64 / chunks_count as f64).ceil() as usize;
            
            let mut chunks = Vec::new();
            for (i, lines_chunk) in lines.chunks(lines_per_chunk).enumerate() {
                let chunk_content = lines_chunk.join("\n");
                let start_line = if i == 0 {
                    self.start_line
                } else {
                    // Approximate start line based on chunk index
                    self.start_line + i * lines_per_chunk
                };
                
                let end_line = if i == chunks_count - 1 {
                    // Last chunk ends at the section end
                    self.end_line
                } else {
                    // Approximate end line based on chunk index
                    self.start_line + (i + 1) * lines_per_chunk - 1
                };
                
                chunks.push(CodeChunk {
                    content: chunk_content,
                    file_path: file_path.to_string(),
                    start_line,
                    end_line,
                    language: "markdown".to_string(),
                    element_type: format!("section_chunk_{}", i + 1),
                });
            }
            chunks
        } else {
            // Create a single chunk
            let element_type = match &self.heading {
                Some(h) => format!("h{}_section", h.level),
                None => "root_section".to_string(),
            };
            
            vec![CodeChunk {
                content: section_content,
                file_path: file_path.to_string(),
                start_line: self.start_line,
                end_line: self.end_line,
                language: "markdown".to_string(),
                element_type,
            }]
        }
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
            _ => "unknown",
        };

        Some(MarkdownElement {
            content,
            element_type: element_type.to_string(),
            start_line,
            end_line,
        })
    }

    /// Converts a tree-sitter node to a markdown heading
    fn node_to_heading(
        &self,
        node: Node,
        code: &str,
    ) -> Option<MarkdownHeading> {
        let start_byte = node.start_byte();
        let end_byte = node.end_byte();
        let content = code.get(start_byte..end_byte)?.to_string();
        let start_line = node.start_position().row + 1;
        let end_line = node.end_position().row + 1;

        let level = self.get_heading_level(&node, code);
        
        // Extract just the heading text, removing the # signs
        let heading_text = match node.kind() {
            "atx_heading" => {
                let mut text = content.trim_start().to_string();
                // Remove leading #s and spaces
                while text.starts_with('#') {
                    text = text[1..].to_string();
                }
                text.trim().to_string()
            },
            "setext_heading" => {
                // Get the part before the underline
                content.split('\n').next().unwrap_or("").trim().to_string()
            },
            _ => content.trim().to_string(),
        };

        Some(MarkdownHeading {
            level,
            content: heading_text,
            start_line,
            end_line,
        })
    }
}

impl SyntaxParser for MarkdownParser {
    fn parse(&mut self, code: &str, file_path: &str) -> Result<Vec<CodeChunk>> {
        // Handle empty or whitespace-only content right away
        if code.trim().is_empty() {
            return Ok(Vec::new());
        }

        let tree = self
            .parser
            .parse(code, None)
            .context("Failed to parse Markdown code")?;
        let root_node = tree.root_node();

        // Extract all headings and elements
        let mut headings = Vec::new();
        let mut elements = Vec::new();
        let mut cursor = QueryCursor::new();
        let code_bytes = code.as_bytes();

        let matches = cursor.matches(&self.query, root_node, code_bytes);

        for mat in matches {
            for capture in mat.captures {
                let capture_name = self.query.capture_names()[capture.index as usize];
                match capture_name {
                    "heading" => {
                        if let Some(heading) = self.node_to_heading(capture.node, code) {
                            headings.push(heading);
                        }
                    },
                    _ => {
                        if let Some(element) = self.node_to_element(capture.node, code, file_path) {
                            elements.push(element);
                        }
                    }
                }
            }
        }

        // Sort headings and elements by line number
        headings.sort_by_key(|h| h.start_line);
        elements.sort_by_key(|e| e.start_line);
        
        // If there are no headings, create a single root section with all elements
        if headings.is_empty() {
            let mut root_section = MarkdownSection::new(None, Vec::new());
            root_section.start_line = 1;
            root_section.end_line = code.lines().count();
            
            for element in elements {
                root_section.add_element(element);
            }
            
            let chunks = root_section.to_chunks(file_path);
            if !chunks.is_empty() {
                return Ok(chunks);
            }
        } else {
            // Otherwise, organize content into sections based on headings
            let mut sections = Vec::new();
            let _current_parent_headings: Vec<MarkdownHeading> = Vec::new();
            let mut stack = Vec::new(); // Stack of (heading_level, parent_headings)
            
            // First, create a section for each heading
            for (i, heading) in headings.iter().enumerate() {
                // Update parent headings stack based on heading level
                while let Some((level, _)) = stack.last() {
                    if *level >= heading.level {
                        stack.pop();
                    } else {
                        break;
                    }
                }
                
                // Build parent headings list for this section
                let parent_headings = stack.iter()
                    .map(|(_, h): &(usize, MarkdownHeading)| h.clone())
                    .collect::<Vec<_>>();
                
                // Create section for this heading
                let section = MarkdownSection::new(Some(heading.clone()), parent_headings);
                sections.push(section);
                
                // Add this heading to the stack for future headings
                stack.push((heading.level, heading.clone()));
                
                // Calculate the end of this section
                let section_end_line = if i < headings.len() - 1 {
                    headings[i + 1].start_line - 1
                } else {
                    code.lines().count()
                };
                
                // Get the last added section and update its end line
                if let Some(section) = sections.last_mut() {
                    section.end_line = section_end_line;
                }
            }
            
            // Assign elements to the appropriate sections
            for element in elements {
                let element_line = element.start_line;
                
                // Find the section this element belongs to
                let section_index = sections
                    .iter()
                    .position(|s| 
                        s.start_line <= element_line && 
                        s.end_line >= element_line
                    );
                
                if let Some(idx) = section_index {
                    // Add the element to this section
                    sections[idx].add_element(element);
                }
            }
            
            // Convert sections to chunks
            let mut chunks = Vec::new();
            for section in sections {
                chunks.extend(section.to_chunks(file_path));
            }
            
            if !chunks.is_empty() {
                return Ok(chunks);
            }
        }
        
        // Fallback to line-based chunking if we couldn't extract meaningful content
        log::debug!(
            "No specific Markdown elements found or only whitespace in {}. Applying line-based fallback chunking.",
            file_path
        );
        let lines: Vec<&str> = code.lines().collect();
        let mut current_line_start = 1;
        let mut fallback_chunks = Vec::new();
        
        for (i, line_chunk) in lines.chunks(MAX_CHUNK_LINES).enumerate() {
            let content = line_chunk.join("\n");
            let start_line = current_line_start;
            let end_line = start_line + line_chunk.len() - 1;

            fallback_chunks.push(CodeChunk {
                content,
                file_path: file_path.to_string(),
                start_line,
                end_line,
                language: "markdown".to_string(),
                element_type: format!("fallback_line_chunk_{}", i),
            });
            current_line_start = end_line + 1;
        }
        
        Ok(fallback_chunks)
    }
} 