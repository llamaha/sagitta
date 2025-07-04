// Placeholder for JavaScript syntax parsing
use anyhow::{Context, Result};
use tree_sitter::{Node, Parser, Query, QueryCursor};

// Use super::parser instead of crate::syntax::parser
use super::parser::{CodeChunk, SyntaxParser};
use super::element_filter::is_core_element_type;

/// Parser for JavaScript language files using Tree-sitter.
pub struct JavaScriptParser {
    parser: Parser,
    query: Query,
}

impl Default for JavaScriptParser {
    fn default() -> Self {
        let mut parser = Parser::new();
        let language = tree_sitter_javascript::language();
        parser
            .set_language(&language)
            .expect("Error loading JavaScript grammar");

        // Query for functions, classes, methods, arrow functions assigned to vars/consts
        let query = Query::new(
            &language,
            r#"
            [
              (function_declaration) @item
              (class_declaration) @item
              (method_definition) @item
              (lexical_declaration (variable_declarator value: (_) @item))
              (variable_declaration (variable_declarator value: (_) @item))
              (expression_statement (assignment_expression right: (_) @item))
            ]
            "#,
        )
        .expect("Error creating JavaScript query");

        JavaScriptParser { parser, query }
    }
}

impl JavaScriptParser {
    /// Creates a new `JavaScriptParser` with the JS grammar and queries.
    pub fn new() -> Self {
        Self::default()
    }

    fn node_to_chunk(
        &self,
        node: Node,
        code: &str,
        file_path: &str,
    ) -> Option<CodeChunk> {
        let start_byte = node.start_byte();
        let end_byte = node.end_byte();
        // --- Debug Print ---
        // if file_path == "test.js" && node.kind() == "function_expression" {
        //     println!("DEBUG (JS): Kind: {}, Range: {}..{}, Code Len: {}", node.kind(), start_byte, end_byte, code.len());
        //     println!("DEBUG (JS): Node Text: {:?}", code.get(start_byte..end_byte));
        // }
        // --- End Debug ---
        let content = code.get(start_byte..end_byte)?.to_string();
        let start_line = node.start_position().row + 1; // tree-sitter is 0-indexed
        let end_line = node.end_position().row + 1;

        // Determine element type based on the actual node captured as @item
        let element_type = match node.kind() {
            "function_declaration" => "function",
            "class_declaration" => "class",
            "method_definition" => "method",
            "arrow_function" => "function",
            "function_expression" => "function",
            _ => {
                // Use the existing debug print
                // println!("DEBUG node_to_chunk: Unknown @item kind: {}", node_kind);
                "unknown"
            }
        };
        // println!("DEBUG node_to_chunk: Determined Type: {}", element_type); // Keep type debug

        // Filter out non-function/class/method captures before creating a chunk
        if !matches!(element_type, "function" | "class" | "method") {
            // println!("DEBUG node_to_chunk: Skipping non-code element type: {}", element_type);
            return None;
        }

        // Filter out potentially small or trivial nodes if necessary (e.g., empty functions)
        // Example: if content.trim() == "{}" || content.trim() == "=> {}" || content.lines().count() < 2 { return None; }

        Some(CodeChunk {
            content,
            file_path: file_path.to_string(),
            start_line,
            end_line,
            language: "javascript".to_string(),
            element_type: element_type.to_string(),
        })
    }
}

impl SyntaxParser for JavaScriptParser {
    fn parse(&mut self, code: &str, file_path: &str) -> Result<Vec<CodeChunk>> {
        let tree = self
            .parser
            .parse(code, None)
            .context("Failed to parse JavaScript code")?;
        let root_node = tree.root_node();

        let mut chunks = Vec::new();
        let mut cursor = QueryCursor::new();
        let code_bytes = code.as_bytes();
        let matches = cursor.matches(&self.query, root_node, code_bytes);

        for mat in matches {
            for capture in mat.captures {
                 if let Some(chunk) = self.node_to_chunk(capture.node, code, file_path) {
                     // Only add chunks for core element types
                     if is_core_element_type(&chunk.element_type, Some("javascript")) {
                         chunks.push(chunk);
                     }
                 }
            }
        }

        // Fallback: If no chunks found in non-empty file, split into smaller chunks
        if chunks.is_empty() && !code.trim().is_empty() {
            log::debug!(
                "No top-level JavaScript items found in {file_path}, splitting into smaller chunks."
            );
            let lines: Vec<&str> = code.lines().collect();
            let num_lines = lines.len();
            const JS_FALLBACK_CHUNK_SIZE: usize = 200; // Define local constant

            for (i, chunk_lines) in lines.chunks(JS_FALLBACK_CHUNK_SIZE).enumerate() {
                let start_line = i * JS_FALLBACK_CHUNK_SIZE + 1;
                let end_line = std::cmp::min(start_line + JS_FALLBACK_CHUNK_SIZE - 1, num_lines);
                let chunk_content = chunk_lines.join("\n");

                if chunk_content.trim().is_empty() {
                    continue;
                }

                chunks.push(CodeChunk {
                    content: chunk_content,
                    file_path: file_path.to_string(),
                    start_line,
                    end_line,
                    language: "javascript".to_string(),
                    element_type: format!("fallback_chunk_{i}"),
                });
            }
        }

        Ok(chunks)
    }
}

#[cfg(test)]
#[path = "javascript_tests.rs"]
mod tests; 