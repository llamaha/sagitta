// Placeholder for Rust syntax parsing
use anyhow::{Context, Result};
use tree_sitter::{Node, Parser, Query, QueryCursor};

// Use super::parser instead of crate::syntax::parser
use super::parser::{CodeChunk, SyntaxParser};

pub struct RustParser {
    parser: Parser,
    query: Query,
}

impl RustParser {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        let language = tree_sitter_rust::language();
        parser
            .set_language(&language)
            .expect("Error loading Rust grammar");

        // Query to find top-level items like functions, structs, impls, traits, enums, modules
        // We capture the entire node for these items.
        let query = Query::new(
            &language,
            r#"
            [
                (function_item) @item
                (struct_item) @item
                (enum_item) @item
                (impl_item) @item
                (trait_item) @item
                (mod_item) @item
                (macro_definition) @item
                (macro_invocation) @item
                (use_declaration) @item
                (extern_crate_declaration) @item
                (type_item) @item
                (union_item) @item
                (static_item) @item
                (const_item) @item
            ]
            "#,
        )
        .expect("Error creating Rust query");

        RustParser { parser, query }
    }

    fn node_to_chunk(
        &self,
        node: Node,
        code: &str,
        file_path: &str,
        language: &str,
        element_type: &str,
    ) -> Option<CodeChunk> {
        let start_byte = node.start_byte();
        let end_byte = node.end_byte();
        let content = code.get(start_byte..end_byte)?.to_string();
        let start_line = node.start_position().row + 1; // tree-sitter is 0-indexed
        let end_line = node.end_position().row + 1;

        Some(CodeChunk {
            content,
            file_path: file_path.to_string(),
            start_line,
            end_line,
            language: language.to_string(),
            element_type: element_type.to_string(),
        })
    }
}

impl SyntaxParser for RustParser {
    fn parse(&mut self, code: &str, file_path: &str) -> Result<Vec<CodeChunk>> {
        let tree = self
            .parser
            .parse(code, None)
            .context("Failed to parse Rust code")?;
        let root_node = tree.root_node();

        let mut chunks = Vec::new();
        let mut cursor = QueryCursor::new();

        let code_bytes = code.as_bytes();

        let matches = cursor.matches(&self.query, root_node, code_bytes);

        for mat in matches {
            for capture in mat.captures {
                let node = capture.node;
                let kind = node.kind();

                // Map the tree-sitter node kind to our element_type string
                let element_type = match kind {
                    "function_item" => "function",
                    "struct_item" => "struct",
                    "enum_item" => "enum",
                    "impl_item" => "impl",
                    "trait_item" => "trait",
                    "mod_item" => "module", // Use 'module' for mod_item
                    "macro_definition" => "macro_definition",
                    "macro_invocation" => "macro_invocation",
                    "use_declaration" => "use",
                    "extern_crate_declaration" => "extern_crate",
                    "type_item" => "type_alias",
                    "union_item" => "union",
                    "static_item" => "static",
                    "const_item" => "const",
                    _ => "unknown", // Should not happen with the current query
                };

                if let Some(chunk) = self.node_to_chunk(
                    node,
                    code,
                    file_path,
                    "rust",
                    element_type,
                ) {
                    chunks.push(chunk);
                }
            }
        }

        // Fallback: If no chunks found, split into smaller fixed-size chunks
        if chunks.is_empty() && !code.trim().is_empty() {
            log::debug!(
                "No top-level Rust items found in {}, splitting into smaller chunks.",
                file_path
            );
            let lines: Vec<&str> = code.lines().collect();
            let num_lines = lines.len();
            const RUST_FALLBACK_CHUNK_SIZE: usize = 200; // Define local constant

            for (i, chunk_lines) in lines.chunks(RUST_FALLBACK_CHUNK_SIZE).enumerate() {
                 let start_line = i * RUST_FALLBACK_CHUNK_SIZE + 1;
                 let end_line = std::cmp::min(start_line + RUST_FALLBACK_CHUNK_SIZE - 1, num_lines);
                 let chunk_content = chunk_lines.join("\n");

                 if chunk_content.trim().is_empty() {
                     continue;
                 }

                 chunks.push(CodeChunk {
                     content: chunk_content,
                     file_path: file_path.to_string(),
                     start_line,
                     end_line,
                     language: "rust".to_string(),
                     element_type: "file_chunk".to_string(), // Use file_chunk type
                 });
            }
        }

        Ok(chunks)
    }
}

#[cfg(test)]
#[path = "rust_tests.rs"]
mod tests; 