use anyhow::{Context, Result};
use tree_sitter::{Node, Parser, Query, QueryCursor};

use crate::syntax::parser::{CodeChunk, SyntaxParser};

const FALLBACK_CHUNK_SIZE: usize = 200; // Define chunk size in lines for fallback

pub struct MarkdownParser {
    parser: Parser,
    query: Query,
}

impl MarkdownParser {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        let language = tree_sitter_md::language(); // Using tree-sitter-md
        parser
            .set_language(&language)
            .expect("Error loading Markdown grammar");

        // Query for headings and fenced code blocks
        // Capture ATX headings directly
        // Capture paragraph within Setext headings
        // Capture fenced code blocks directly
        let query = Query::new(
            &language,
            r#"
            [
              (atx_heading) @heading
              (setext_heading) @heading ;; Capture the whole setext node
              (fenced_code_block) @code
            ]
            "#,
        )
        .expect("Error creating Markdown query");

        MarkdownParser { parser, query }
    }

    // Helper to get text content of a node
    fn get_node_text<'a>(node: Node<'a>, code: &'a str) -> &'a str {
        code.get(node.start_byte()..node.end_byte()).unwrap_or("")
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

        let element_type = match node.kind() {
            "atx_heading" => {
                let marker_node = node.child(0)?;
                let level = marker_node.end_byte() - marker_node.start_byte();
                format!("heading_{}", level)
            }
            "paragraph" => { // Handle paragraphs captured by the query
                if let Some(parent) = node.parent() {
                    if parent.kind() == "setext_heading" {
                        let has_h1_underline = parent.children(&mut node.walk()).any(|child| child.kind() == "setext_h1_underline");
                        if has_h1_underline {
                            "heading_1".to_string()
                        } else {
                            "heading_2".to_string()
                        }
                    } else {
                        return None; // Skip non-heading paragraphs
                    }
                } else {
                    return None;
                }
            }
            "fenced_code_block" => "code_block".to_string(),
            _ => "unknown".to_string(),
        };

        Some(CodeChunk {
            content,
            file_path: file_path.to_string(),
            start_line,
            end_line,
            language: "markdown".to_string(),
            element_type,
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

        let mut chunks = Vec::new();
        let mut cursor = QueryCursor::new();
        let code_bytes = code.as_bytes();

        let matches = cursor.matches(&self.query, root_node, code_bytes);

        for mat in matches {
            // Use the first capture per match (we only have one capture per pattern)
            if let Some(capture) = mat.captures.first() {
                 if let Some(chunk) = self.node_to_chunk(capture.node, code, file_path) {
                    chunks.push(chunk);
                }
            }
        }

        // Fallback: Split into smaller chunks if the file is large and no elements were found
        if chunks.is_empty() && !code.trim().is_empty() {
            log::debug!(
                "No Markdown items found in {}, splitting into smaller chunks.",
                file_path
            );
            let lines: Vec<&str> = code.lines().collect();
            let num_lines = lines.len();
            for (i, chunk_lines) in lines.chunks(FALLBACK_CHUNK_SIZE).enumerate() {
                 let start_line = i * FALLBACK_CHUNK_SIZE + 1;
                 let end_line = std::cmp::min(start_line + FALLBACK_CHUNK_SIZE - 1, num_lines);
                 let chunk_content = chunk_lines.join("\n");

                 if chunk_content.trim().is_empty() {
                     continue;
                 }

                 chunks.push(CodeChunk {
                     content: chunk_content,
                     file_path: file_path.to_string(),
                     start_line,
                     end_line,
                     language: "markdown".to_string(), // Fallback is still markdown
                     element_type: "file_chunk".to_string(),
                 });
            }
        }

        Ok(chunks)
    }
} 