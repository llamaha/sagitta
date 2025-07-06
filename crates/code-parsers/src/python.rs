use anyhow::{Context, Result};
use tree_sitter::{Node, Parser, Query, QueryCursor};

// Use super::parser instead of crate::syntax::parser
use super::parser::{CodeChunk, SyntaxParser};
use super::element_filter::is_core_element_type;

/// Parser for Python language files using Tree-sitter.
pub struct PythonParser {
    parser: Parser,
    query: Query,
}

impl Default for PythonParser {
    fn default() -> Self {
        let mut parser = Parser::new();
        let language = tree_sitter_python::language();
        parser
            .set_language(&language)
            .expect("Error loading Python grammar");

        // List top-level node types directly, filtering by parent later
        let query = Query::new(
            &language,
            r#"
            [
              (function_definition) @func
              (class_definition) @class
              (decorated_definition) @decorator
              (import_statement) @import
              (import_from_statement) @import_from
              (if_statement) @if
              (for_statement) @for
              (while_statement) @while
              (try_statement) @try
              (with_statement) @with
              (expression_statement) @expr_stmt
            ]
            "#,
        )
        .expect("Error creating Python query");

        PythonParser { parser, query }
    }
}

impl PythonParser {
    /// Creates a new `PythonParser` with the Python grammar and queries.
    pub fn new() -> Self {
        Self::default()
    }

    fn node_to_chunk(
        &self,
        node: Node,
        code: &str,
        file_path: &str,
        capture_name: &str, // Use capture name to determine type
    ) -> Option<CodeChunk> {
        let start_byte = node.start_byte();
        let end_byte = node.end_byte();
        let content = code.get(start_byte..end_byte)?.to_string();
        let start_line = node.start_position().row + 1;
        let end_line = node.end_position().row + 1;

        let element_type = match capture_name {
            "func" => "function",
            "class" => "class",
            "decorator" => {
                // Look inside decorated_definition for actual type using children iterator
                let mut cursor = node.walk();
                if node.children(&mut cursor).any(|n| n.kind() == "function_definition") {
                    "function"
                } else if node.children(&mut cursor).any(|n| n.kind() == "class_definition") {
                    "class"
                } else {
                    "decorated_definition" // Fallback
                }
            },
            "import" | "import_from" | "if" | "for" | "while" | "try" | "with" | "expr_stmt" => "statement",
            _ => "unknown", // Should not happen
        };

        Some(CodeChunk {
            content,
            file_path: file_path.to_string(),
            start_line,
            end_line,
            language: "python".to_string(),
            element_type: element_type.to_string(),
        })
    }
}

impl SyntaxParser for PythonParser {
    fn parse(&mut self, code: &str, file_path: &str) -> Result<Vec<CodeChunk>> {
        let tree = self
            .parser
            .parse(code, None)
            .context("Failed to parse Python code")?;
        let root_node = tree.root_node();

        let mut chunks = Vec::new();
        let mut cursor = QueryCursor::new();
        let code_bytes = code.as_bytes();

        // Helper to check if a node is the module-level docstring
        let is_docstring = |node: Node| -> bool {
            if let Some(parent) = node.parent() {
                if parent.kind() == "module" {
                    // Check if it's an expression_statement whose first NAMED child is a string
                    if node.kind() == "expression_statement" {
                        let mut cursor = node.walk();
                        // Check for any child that is a string
                        if node.children(&mut cursor).any(|n| n.kind() == "string") {
                            return true;
                        }
                    }
                }
            }
            false
        };

         // Helper to check if a node is a top-level 'pass' statement
        let is_pass_stmt = |node: Node, _code_bytes: &[u8]| -> bool { // code_bytes no longer needed
            if node.kind() == "expression_statement" && node.child_count() == 1 {
                if let Some(inner) = node.child(0) {
                    // Check for the specific pass_statement node kind
                    return inner.kind() == "pass_statement"; 
                }
            }
            false
        };

        let matches = cursor.matches(&self.query, root_node, code_bytes);

        for mat in matches {
            for capture in mat.captures {
                let node = capture.node;
                let capture_name = self.query.capture_names()[capture.index as usize];

                // ** REMOVED Filter: Ensure the node's parent is the module **
                // if node.parent().map_or(true, |p| p.kind() != "module") {
                //     continue; // Skip nodes not directly under the module
                // }

                // Apply filters based on node type and content (e.g., for top-level items)
                if node.parent().is_some_and(|p| p.kind() == "module") 
                    && capture_name == "expr_stmt" 
                    && (is_docstring(node) || is_pass_stmt(node, code_bytes)) {
                    continue; // Skip docstrings and top-level 'pass'
                }

                if let Some(chunk) = self.node_to_chunk(node, code, file_path, capture_name) {
                    // Only add chunks for core element types
                    if is_core_element_type(&chunk.element_type, Some("python")) {
                        chunks.push(chunk);
                    }
                }
            }
        }

        // Sort chunks by start line as query matches might not be ordered
        chunks.sort_by_key(|c| c.start_line);

        // Fallback: If no chunks found, split into fallback chunks
        if chunks.is_empty() && !code.trim().is_empty() {
            log::debug!(
                "No Python items found in {file_path}, splitting into fallback chunks."
            );
            let lines: Vec<&str> = code.lines().collect();
            let num_lines = lines.len();
            const PYTHON_FALLBACK_CHUNK_SIZE: usize = 200;

            for (i, chunk_lines) in lines.chunks(PYTHON_FALLBACK_CHUNK_SIZE).enumerate() {
                let start_line = i * PYTHON_FALLBACK_CHUNK_SIZE + 1;
                let end_line = std::cmp::min(start_line + PYTHON_FALLBACK_CHUNK_SIZE - 1, num_lines);
                let chunk_content = chunk_lines.join("\n");

                if chunk_content.trim().is_empty() {
                    continue;
                }

                chunks.push(CodeChunk {
                    content: chunk_content,
                    file_path: file_path.to_string(),
                    start_line,
                    end_line,
                    language: "python".to_string(),
                    element_type: format!("fallback_chunk_{i}"),
                });
            }
        }

        Ok(chunks)
    }
}

#[cfg(test)]
#[path = "python_tests.rs"]
mod tests; 