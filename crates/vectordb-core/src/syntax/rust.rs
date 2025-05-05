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

        // Query to find top-level items AND items within certain containers (like methods in impls).
        // Capture the entire node for these items.
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

        // Helper query to prevent capturing items that are children of other captured items 
        // (e.g., don't capture a function if it's already part of a captured impl function)
        // Note: Tree-sitter queries for checking parent/child relationships can be complex.
        // We might need a post-processing step instead if this isn't robust.
        // Let's try a simpler approach first: process captures and skip if already covered.

        RustParser { parser, query }
    }

    /// Extracts doc comments immediately preceding the given node.
    fn extract_doc_comments(&self, node: &Node, code: &str) -> String {
        let mut doc_lines = Vec::new();
        let mut cur = node.prev_sibling();
        let mut node_start_row = node.start_position().row;
        // Only look at siblings that are comments and directly above
        while let Some(sib) = cur {
            let kind = sib.kind();
            let end_row = sib.end_position().row;
            if (kind == "line_comment" || kind == "block_comment") && end_row + 1 == node_start_row {
                let text = code.get(sib.start_byte()..sib.end_byte()).unwrap_or("");
                // Only include doc comments (/// or /** ... */)
                if text.trim_start().starts_with("///") || text.trim_start().starts_with("/**") {
                    doc_lines.push(text.trim_end().to_string());
                }
                cur = sib.prev_sibling();
                // Update node_start_row to allow for stacked comments
                // (e.g., multiple /// lines)
                node_start_row = sib.start_position().row;
            } else {
                break;
            }
        }
        doc_lines.reverse();
        if !doc_lines.is_empty() {
            format!("{}\n", doc_lines.join("\n"))
        } else {
            String::new()
        }
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
        let mut content = code.get(start_byte..end_byte)?.to_string();
        let start_line = node.start_position().row + 1; // tree-sitter is 0-indexed
        let end_line = node.end_position().row + 1;

        // Prepend doc comments for struct, enum, trait, function
        if matches!(element_type, "struct" | "enum" | "trait" | "function") {
            let doc = self.extract_doc_comments(&node, code);
            if !doc.is_empty() {
                content = format!("{}\n{}", doc, content);
            }
        }

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
        let code_bytes = code.as_bytes();

        let mut chunks = Vec::new();
        let mut query_cursor = QueryCursor::new();

        let mut covered_ranges: Vec<(usize, usize)> = Vec::new();
        let matches = query_cursor.matches(&self.query, root_node, code_bytes);

        for mat in matches {
            if let Some(capture) = mat.captures.iter().find(|c| self.query.capture_names()[c.index as usize] == "item") {
                let node = capture.node;
                let start_byte = node.start_byte();
                let end_byte = node.end_byte();
                let kind = node.kind();

                let is_contained = covered_ranges.iter().any(|(start, end)| {
                    start_byte >= *start && end_byte <= *end
                });
                if is_contained {
                    continue;
                }

                if kind == "impl_item" {
                    // --- Get impl signature --- 
                    let mut impl_signature = "";
                    // Find the body node (declaration_list or associated_type) to get signature end
                    let body_start_byte = node.children(&mut node.walk())
                        .find(|n| n.kind() == "declaration_list" || n.kind() == "associated_type")
                        .map(|n| n.start_byte())
                        .unwrap_or(end_byte); // Fallback to end of impl if no body found
                    
                    // Extract text from start of impl node up to the start of the body
                    impl_signature = code.get(start_byte..body_start_byte).unwrap_or("").trim_end();
                    // Ensure it ends cleanly, trim trailing whitespace or '{'
                    impl_signature = impl_signature.trim_end_matches(|c: char| c.is_whitespace() || c == '{').trim_end();

                    // Iterate children and create chunks for functions, prepending signature
                    let mut tree_cursor = node.walk();
                    for child_node in node.children(&mut tree_cursor) {
                        if child_node.kind() == "function_item" {
                            let func_start = child_node.start_byte();
                            let func_end = child_node.end_byte();
                            let func_is_covered = covered_ranges.iter().any(|(start, end)| {
                                func_start >= *start && func_end <= *end
                            });
                            
                            if !func_is_covered {
                                let func_content = code.get(func_start..func_end).unwrap_or("");
                                let combined_content = format!("{}\n...\n{}", impl_signature, func_content);

                                // Use node_to_chunk logic but with modified content
                                let start_line = child_node.start_position().row + 1;
                                let end_line = child_node.end_position().row + 1;

                                chunks.push(CodeChunk {
                                    content: combined_content,
                                    file_path: file_path.to_string(),
                                    start_line,
                                    end_line,
                                    language: "rust".to_string(),
                                    element_type: "function".to_string(),
                                });

                                covered_ranges.push((func_start, func_end));
                            }
                        }
                    }
                    // Mark entire impl range covered
                    covered_ranges.push((start_byte, end_byte)); 
                    continue; // Skip impl_item itself
                }

                // --- Handling for other item types (including standalone functions) ---
                let already_covered = covered_ranges.iter().any(|(start, end)| {
                    start_byte >= *start && end_byte <= *end
                 });
                 if already_covered {
                    continue;
                 }

                let element_type = match kind {
                    "function_item" => "function",
                    "struct_item" => "struct",
                    "enum_item" => "enum",
                    "trait_item" => "trait",
                    "mod_item" => "module",
                    "macro_definition" => "macro_definition",
                    "macro_invocation" => "macro_invocation",
                    "use_declaration" => "use",
                    "extern_crate_declaration" => "extern_crate",
                    "type_item" => "type_alias",
                    "union_item" => "union",
                    "static_item" => "static",
                    "const_item" => "const",
                    _ => "unknown",
                };

                if element_type != "unknown" {
                    // Create chunk normally for non-impl items
                    if let Some(chunk) = self.node_to_chunk(
                        node,
                        code,
                        file_path,
                        "rust",
                        element_type,
                    ) {
                        covered_ranges.push((start_byte, end_byte));
                        chunks.push(chunk);
                    }
                }
            }
        }

        covered_ranges.sort_by_key(|k| k.0);

        // Fallback: If no chunks found OR significant code remains uncovered
        // We need a more robust way to check for uncovered code.
        // For now, keep the simple fallback logic.
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