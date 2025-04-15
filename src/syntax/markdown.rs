// src/syntax/markdown.rs
use anyhow::{Context, Result};
use tree_sitter::{Node, Parser, Query, QueryCursor};

use crate::syntax::parser::{CodeChunk, SyntaxParser};

// Reuse the constant from the fallback parser or define locally if preferred
const MAX_CHUNK_LINES: usize = 500; 

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

        // Query using potential tree-sitter-md node types
        // (Guessing based on common patterns - adjust after checking grammar)
        let query = Query::new(
            &language,
            r#"
            [
              (atx_heading) @item
              (setext_heading) @item
              (fenced_code_block) @item
              (list_item) @item
              (paragraph) @item       ; Capture all paragraphs initially
            ]
            "#,
        )
        .expect("Error creating Markdown (md) query");

        MarkdownParser { parser, query }
    }

    fn node_to_chunk(
        &self,
        node: Node,
        code: &str,
        file_path: &str,
    ) -> Option<CodeChunk> {
        let start_byte = node.start_byte();
        let end_byte = node.end_byte();
        let content = code.get(start_byte..end_byte)?.to_string();
        let start_line = node.start_position().row + 1;
        let end_line = node.end_position().row + 1;

        let node_kind = node.kind();

        // Map potential tree-sitter-md types
        let element_type = match node_kind {
            "atx_heading" => {
                // Basic level detection based on content
                if content.starts_with("###") {
                    "h3"
                } else if content.starts_with("##") {
                    "h2"
                } else if content.starts_with('#') {
                    "h1"
                } else {
                    "heading"
                }
            },
            "setext_heading" => {
                // Level detection based on underline character
                if content.contains("\n===") {
                    "h1"
                } else if content.contains("\n---") {
                    "h2"
                } else {
                    "heading"
                }
            },
            "paragraph" => "paragraph",
            "fenced_code_block" => "code_block",
            "list_item" => "list_item",
            _ => "unknown",
        };

        Some(CodeChunk {
            content,
            file_path: file_path.to_string(),
            start_line,
            end_line,
            language: "markdown".to_string(),
            element_type: element_type.to_string(),
        })
    }
}

impl SyntaxParser for MarkdownParser {
    fn parse(&mut self, code: &str, file_path: &str) -> Result<Vec<CodeChunk>> {
        let tree = self
            .parser
            .parse(code, None)
            .context("Failed to parse Markdown code")?;
        let root_node = tree.root_node();

        let mut potential_chunks = Vec::new();
        let mut cursor = QueryCursor::new();
        let code_bytes = code.as_bytes();

        let matches = cursor.matches(&self.query, root_node, code_bytes);

        for mat in matches {
            for capture in mat.captures {
                let capture_name = self.query.capture_names()[capture.index as usize];
                if capture_name == "item" {
                    if let Some(chunk) = self.node_to_chunk(capture.node, code, file_path) {
                        potential_chunks.push((capture.node, chunk)); // Store node along with chunk
                    }
                }
            }
        }

        // Filter based on parent node kind
        let mut chunks: Vec<CodeChunk> = potential_chunks
            .into_iter()
            .filter_map(|(node, chunk)| {
                if let Some(parent) = node.parent() {
                    // Filter 1: Paragraphs directly inside list items
                    if chunk.element_type == "paragraph" && parent.kind() == "list_item" {
                        return None;
                    }
                    // Filter 2: List items whose immediate parent is a list (nested lists)
                    if chunk.element_type == "list_item" && parent.kind() == "list" {
                        // Further check: Ensure this 'list' node is itself nested within a 'list_item'
                        if let Some(grandparent) = parent.parent() {
                            if grandparent.kind() == "list_item" {
                                return None;
                            }
                        }
                    }
                }
                // Keep the chunk if none of the filtering conditions match
                Some(chunk)
            })
            .collect();

        // If no chunks were extracted by tree-sitter OR if the code is just whitespace,
        // apply line-based chunking as a fallback.
        if chunks.is_empty() && !code.trim().is_empty() {
            log::debug!(
                "No specific Markdown elements found or only whitespace in {}. Applying line-based fallback chunking.",
                file_path
            );
            let lines: Vec<&str> = code.lines().collect();
            let mut current_line_start = 1;
            for (i, line_chunk) in lines.chunks(MAX_CHUNK_LINES).enumerate() {
                let content = line_chunk.join("\n");
                let start_line = current_line_start;
                let end_line = start_line + line_chunk.len() - 1;

                chunks.push(CodeChunk {
                    content,
                    file_path: file_path.to_string(),
                    start_line,
                    end_line,
                    language: "markdown".to_string(), // Keep language as markdown
                    element_type: format!("fallback_line_chunk_{}", i), // Indicate it's a fallback chunk
                });
                current_line_start = end_line + 1;
            }
        }

        Ok(chunks)
    }
} 