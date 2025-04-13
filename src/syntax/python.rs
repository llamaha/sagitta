use anyhow::{Context, Result};
use tree_sitter::{Node, Parser, Query, QueryCursor};

use crate::syntax::parser::{CodeChunk, SyntaxParser};

pub struct PythonParser {
    parser: Parser,
    query: Query,
}

impl PythonParser {
    pub fn new() -> Self {
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
                     // Check if this node is the first NAMED child of the module
                    if parent.named_child(0) == Some(node) {
                        // Must be an expression_statement containing only a string node
                        if node.kind() == "expression_statement" && node.named_child_count() == 1 {
                            if let Some(inner) = node.named_child(0) {
                                return inner.kind() == "string";
                            }
                        }
                    }
                }
            }
            false
        };

         // Helper to check if a node is a top-level 'pass' statement
        let is_pass_stmt = |node: Node, code_bytes: &[u8]| -> bool {
            if node.kind() == "expression_statement" && node.child_count() == 1 {
                if let Some(inner) = node.child(0) {
                    if inner.kind() == "identifier" {
                         return code_bytes.get(inner.start_byte()..inner.end_byte()) == Some(b"pass");
                    }
                }
            }
            false
        };

        let matches = cursor.matches(&self.query, root_node, code_bytes);

        for mat in matches {
            for capture in mat.captures {
                let node = capture.node;
                let capture_name = self.query.capture_names()[capture.index as usize];

                // ** Filter: Ensure the node's parent is the module **
                if node.parent().map_or(true, |p| p.kind() != "module") {
                    continue; // Skip nodes not directly under the module
                }

                // Apply filters based on node type and content for module children
                if capture_name == "expr_stmt" {
                    if is_docstring(node) || is_pass_stmt(node, code_bytes) {
                        continue; // Skip docstrings and top-level 'pass'
                    }
                }

                if let Some(chunk) = self.node_to_chunk(node, code, file_path, capture_name) {
                    chunks.push(chunk);
                }
            }
        }

        // Sort chunks by start line as query matches might not be ordered
        chunks.sort_by_key(|c| c.start_line);

        // Fallback: If no specific items found, chunk the whole file
        if chunks.is_empty() && !code.trim().is_empty() {
            log::debug!(
                "No top-level Python elements found in {}, indexing as whole file.",
                file_path
            );
            chunks.push(CodeChunk {
                content: code.to_string(),
                file_path: file_path.to_string(),
                start_line: 1,
                end_line: code.lines().count(),
                language: "python".to_string(),
                element_type: "file".to_string(),
            });
        }

        Ok(chunks)
    }
} 