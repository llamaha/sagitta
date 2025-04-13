// Placeholder for Golang syntax parsing
use anyhow::{Context, Result};
use tree_sitter::{Node, Parser, Query, QueryCursor};

use crate::syntax::parser::{CodeChunk, SyntaxParser};

const FALLBACK_CHUNK_SIZE: usize = 200; // Define chunk size in lines

pub struct GolangParser {
    parser: Parser,
    query: Query,
}

impl GolangParser {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        let language = tree_sitter_go::language();
        parser
            .set_language(&language)
            .expect("Error loading Go grammar");

        // Query for functions, methods, types (structs/interfaces), consts, vars
        let query = Query::new(
            &language,
            r#"
            [
                (function_declaration) @item
                (method_declaration) @item
                (type_declaration) @item
                (const_declaration) @item
                (var_declaration) @item
            ]
            "#,
        )
        .expect("Error creating Go query");

        GolangParser { parser, query }
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
            "function_declaration" => "function",
            "method_declaration" => "method",
            "type_declaration" => "type", // Could try to inspect inner node (e.g. struct_type) for more specificity
            "const_declaration" => "const",
            "var_declaration" => "var",
            _ => "unknown",
        };

        Some(CodeChunk {
            content,
            file_path: file_path.to_string(),
            start_line,
            end_line,
            language: "go".to_string(), // Changed language
            element_type: element_type.to_string(),
        })
    }
}

impl SyntaxParser for GolangParser {
    fn parse(&mut self, code: &str, file_path: &str) -> Result<Vec<CodeChunk>> {
        let tree = self
            .parser
            .parse(code, None)
            .context("Failed to parse Go code")?;
        let root_node = tree.root_node();

        let mut chunks = Vec::new();
        let mut cursor = QueryCursor::new();

        let code_bytes = code.as_bytes();
        let matches = cursor.matches(&self.query, root_node, code_bytes);

        for mat in matches {
            for capture in mat.captures {
                if let Some(chunk) = self.node_to_chunk(capture.node, code, file_path) {
                    chunks.push(chunk);
                }
            }
        }

        // Fallback: If no chunks found in non-empty file, index whole file
        if chunks.is_empty() && !code.trim().is_empty() {
            log::debug!(
                "No top-level Go items found in {}, splitting into smaller chunks.",
                file_path
            );
            let lines: Vec<&str> = code.lines().collect();
            let num_lines = lines.len();
            for (i, chunk_lines) in lines.chunks(FALLBACK_CHUNK_SIZE).enumerate() {
                let start_line = i * FALLBACK_CHUNK_SIZE + 1;
                // Adjust end_line calculation to prevent going past the actual number of lines
                let end_line = std::cmp::min(start_line + FALLBACK_CHUNK_SIZE - 1, num_lines);
                let chunk_content = chunk_lines.join("\n"); // Use \n to simulate newline within the chunk content string

                // Skip empty chunks that might result from trailing newlines etc.
                if chunk_content.trim().is_empty() {
                    continue;
                }

                chunks.push(CodeChunk {
                    content: chunk_content,
                    file_path: file_path.to_string(),
                    start_line,
                    end_line,
                    language: "go".to_string(),
                    element_type: "file_chunk".to_string(), // Indicate it's a fallback chunk
                });
            }
        }

        Ok(chunks)
    }
} 