// Placeholder for Ruby syntax parsing
use anyhow::{Context, Result};
use tree_sitter::{Node, Parser, Query, QueryCursor};

// Use super::parser instead of crate::syntax::parser
use super::parser::{CodeChunk, SyntaxParser};

/// Parser for Ruby language files using Tree-sitter.
pub struct RubyParser {
    parser: Parser,
    query: Query,
}

impl RubyParser {
    /// Creates a new `RubyParser` with the Ruby grammar and queries.
    pub fn new() -> Self {
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
                    // Basic filtering: Avoid capturing very small trivial blocks if desired
                    // if chunk.element_type == "block" && chunk.content.len() < 20 {
                    //     continue;
                    // }
                    chunks.push(chunk);
                }
            }
        }

        // Fallback: If no chunks found in non-empty file, index whole file
        if chunks.is_empty() && !code.trim().is_empty() {
            log::debug!(
                "No top-level Ruby items found in {}, indexing as whole file.",
                file_path
            );
             chunks.push(CodeChunk {
                 content: code.to_string(),
                 file_path: file_path.to_string(),
                 start_line: 1,
                 end_line: code.lines().count(),
                 language: "ruby".to_string(),
                 element_type: "file".to_string(),
             });
        }

        Ok(chunks)
    }
}

#[cfg(test)]
#[path = "ruby_tests.rs"]
mod tests; 