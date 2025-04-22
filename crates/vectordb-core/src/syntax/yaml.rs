// Placeholder for YAML syntax parsing
use anyhow::{Context, Result};
use tree_sitter::{Node, Parser, Query, QueryCursor};

// Use super::parser instead of crate::syntax::parser
use super::parser::{CodeChunk, SyntaxParser};

pub struct YamlParser {
    parser: Parser,
    query: Query, // Query to find YAML documents
}

impl YamlParser {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        let language = tree_sitter_yaml::language();
        parser
            .set_language(&language)
            .expect("Error loading YAML grammar");

        // Query for top-level documents (separated by ---)
        let query = Query::new(
            &language,
            r#"
            (document) @item
            "#,
        )
        .expect("Error creating YAML query");

        YamlParser { parser, query }
    }

    // Convert a document node into a chunk
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

        // We only capture documents, so the type is fixed
        let element_type = "document";

        Some(CodeChunk {
            content,
            file_path: file_path.to_string(),
            start_line,
            end_line,
            language: "yaml".to_string(),
            element_type: element_type.to_string(),
        })
    }
}

impl SyntaxParser for YamlParser {
    fn parse(&mut self, code: &str, file_path: &str) -> Result<Vec<CodeChunk>> {
        let tree = self
            .parser
            .parse(code, None)
            .context("Failed to parse YAML code")?;
        let root_node = tree.root_node();

        let mut chunks = Vec::new();
        let mut cursor = QueryCursor::new();
        let code_bytes = code.as_bytes();

        // Run query once to get matches
        let matches: Vec<_> = cursor.matches(&self.query, root_node, code_bytes).collect();

        // Collect chunks from query matches
        for mat in matches.iter() { // Iterate over the collected matches
            for (_i, capture) in mat.captures.iter().enumerate() { // Enumerate captures, mark i as unused
                if let Some(chunk) = self.node_to_chunk(capture.node, code, file_path) {
                    chunks.push(chunk);
                }
            }
        }

        // Fallback: Only if NO documents were found by the query
        // AND the file is not empty, treat the whole file as one document chunk.
        if chunks.is_empty() && !code.trim().is_empty() {
            let fallback_chunk = CodeChunk {
                content: code.to_string(),
                file_path: file_path.to_string(),
                start_line: 1,
                end_line: code.lines().count(),
                language: "yaml".to_string(),
                element_type: "document".to_string(), // Treat whole file as a document
            };
            chunks.push(fallback_chunk);
        }

        // Deduplicate chunks (Workaround for potential grammar/query issues)
        chunks.dedup_by(|a, b| {
            a.file_path == b.file_path &&
            a.start_line == b.start_line &&
            a.end_line == b.end_line &&
            a.element_type == b.element_type
            // Optionally compare content if needed, but range should be sufficient
            // a.content == b.content
        });

        Ok(chunks)
    }
}

#[cfg(test)]
#[path = "yaml_tests.rs"]
mod tests; 