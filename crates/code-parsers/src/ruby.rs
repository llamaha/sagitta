// Placeholder for Ruby syntax parsing
use anyhow::{Context, Result};
use tree_sitter::{Node, Parser, Query, QueryCursor};

// Use super::parser instead of crate::syntax::parser
use super::parser::{CodeChunk, SyntaxParser};
use super::element_filter::is_core_element_type;

/// Parser for Ruby language files using Tree-sitter.
pub struct RubyParser {
    parser: Parser,
    query: Query,
}

impl Default for RubyParser {
    fn default() -> Self {
        let mut parser = Parser::new();
        let language = tree_sitter_ruby::language();
        parser
            .set_language(&language)
            .expect("Error loading Ruby grammar");

        // Query for modules, classes, methods (instance and singleton/class methods)
        let query = Query::new(
            &language,
            r#"
            [
                (module) @item
                (class) @item
                (method) @item
                (singleton_method) @item
            ]
            "#,
        )
        .expect("Error creating Ruby query");

        RubyParser { parser, query }
    }
}

impl RubyParser {
    /// Creates a new `RubyParser` with the Ruby grammar and queries.
    pub fn new() -> Self {
        Self::default()
    }

    // Re-use a helper similar to Rust's, but specific to Ruby element types
    fn node_to_chunk(
        &self,
        node: Node,
        code: &str,
        file_path: &str,
    ) -> Option<CodeChunk> {
        let start_byte = node.start_byte();
        let end_byte = node.end_byte();
        let content = code.get(start_byte..end_byte)?.to_string();
        let start_line = node.start_position().row + 1; // tree-sitter is 0-indexed
        let end_line = node.end_position().row + 1;

        let element_type = match node.kind() {
            "module" => "module",
            "class" => "class",
            "method" => "method",
            "singleton_method" => "singleton_method", // Class method
            // "block" => "block", // Removed block mapping
            _ => "unknown",
        };

        Some(CodeChunk {
            content,
            file_path: file_path.to_string(),
            start_line,
            end_line,
            language: "ruby".to_string(),
            element_type: element_type.to_string(),
        })
    }
}

impl SyntaxParser for RubyParser {
    fn parse(&mut self, code: &str, file_path: &str) -> Result<Vec<CodeChunk>> {
        let tree = self
            .parser
            .parse(code, None)
            .context("Failed to parse Ruby code")?;
        let root_node = tree.root_node();

        let mut chunks = Vec::new();
        let mut cursor = QueryCursor::new();

        let code_bytes = code.as_bytes();

        let matches = cursor.matches(&self.query, root_node, code_bytes);

        for mat in matches {
            for capture in mat.captures {
                if let Some(chunk) = self.node_to_chunk(capture.node, code, file_path) {
                    // Only add chunks for core element types
                    if is_core_element_type(&chunk.element_type, Some("ruby")) {
                        chunks.push(chunk);
                    }
                }
            }
        }

        // Fallback: If no chunks found in non-empty file, split into smaller chunks
        if chunks.is_empty() && !code.trim().is_empty() {
            log::debug!(
                "No top-level Ruby items found in {file_path}, splitting into smaller chunks."
            );
            let lines: Vec<&str> = code.lines().collect();
            let num_lines = lines.len();
            const RUBY_FALLBACK_CHUNK_SIZE: usize = 200; // Define local constant

            for (i, chunk_lines) in lines.chunks(RUBY_FALLBACK_CHUNK_SIZE).enumerate() {
                let start_line = i * RUBY_FALLBACK_CHUNK_SIZE + 1;
                let end_line = std::cmp::min(start_line + RUBY_FALLBACK_CHUNK_SIZE - 1, num_lines);
                let chunk_content = chunk_lines.join("\n");

                if chunk_content.trim().is_empty() {
                    continue;
                }

                chunks.push(CodeChunk {
                    content: chunk_content,
                    file_path: file_path.to_string(),
                    start_line,
                    end_line,
                    language: "ruby".to_string(),
                    element_type: format!("fallback_chunk_{i}"),
                });
            }
        }

        Ok(chunks)
    }
}

#[cfg(test)]
#[path = "ruby_tests.rs"]
mod tests; 