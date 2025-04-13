// Placeholder for TypeScript syntax parsing
use anyhow::{Context, Result};
use tree_sitter::{Node, Parser, Query, QueryCursor};

use crate::syntax::parser::{CodeChunk, SyntaxParser};

pub struct TypeScriptParser {
    parser: Parser,
    query: Query,
}

impl TypeScriptParser {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        // Use the TypeScript-specific grammar
        let language = tree_sitter_typescript::language_typescript();
        parser
            .set_language(&language)
            .expect("Error loading TypeScript grammar");

        // Extended query for TypeScript: functions, classes, methods, interfaces, enums, types,
        // plus assignments of functions/arrows
        let query = Query::new(
            &language,
            r#"
            [
              (function_declaration) @item
              (class_declaration) @item
              (method_definition) @item
              (interface_declaration) @item
              (enum_declaration) @item
              (type_alias_declaration) @item
              (lexical_declaration (variable_declarator value: (_) @item))
              (variable_declaration (variable_declarator value: (_) @item))
              (expression_statement (assignment_expression right: (_) @item))
            ]
            "#,
        )
        .expect("Error creating TypeScript query");

        TypeScriptParser { parser, query }
    }

    // Re-use the chunking logic, adding TypeScript types
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

        let element_type = match node_kind {
            "function_declaration" => "function",
            "class_declaration" => "class",
            "method_definition" => "method",
            "interface_declaration" => "interface",
            "enum_declaration" => "enum",
            "type_alias_declaration" => "type",
            "arrow_function" => "function",
            "function_expression" => "function",
            _ => "unknown", // Other captured items (like identifiers in assignments) are ignored below
        };

        // Filter out non-code elements captured by the general assignment/declaration queries
        if !matches!(element_type, "function" | "class" | "method" | "interface" | "enum" | "type") {
             return None;
        }

        Some(CodeChunk {
            content,
            file_path: file_path.to_string(),
            start_line,
            end_line,
            language: "typescript".to_string(), // Set language correctly
            element_type: element_type.to_string(),
        })
    }
}

impl SyntaxParser for TypeScriptParser {
    fn parse(&mut self, code: &str, file_path: &str) -> Result<Vec<CodeChunk>> {
        let tree = self
            .parser
            .parse(code, None)
            .context("Failed to parse TypeScript code")?;
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

        // Fallback logic
        if chunks.is_empty() && !code.trim().is_empty() {
            log::debug!(
                "No top-level TypeScript items found in {}, indexing as whole file.",
                file_path
            );
             chunks.push(CodeChunk {
                 content: code.to_string(),
                 file_path: file_path.to_string(),
                 start_line: 1,
                 end_line: code.lines().count(),
                 language: "typescript".to_string(),
                 element_type: "file".to_string(),
             });
        }

        Ok(chunks)
    }
} 