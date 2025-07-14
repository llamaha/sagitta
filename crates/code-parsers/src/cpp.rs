use anyhow::{Context, Result};
use tree_sitter::{Node, Parser, Query, QueryCursor};

// Use super::parser instead of crate::syntax::parser
use super::parser::{CodeChunk, SyntaxParser};
use super::element_filter::is_core_element_type;

/// Parser for C++ language files using Tree-sitter.
pub struct CppParser {
    parser: Parser,
    query: Query,
}

impl Default for CppParser {
    fn default() -> Self {
        let mut parser = Parser::new();
        let language = tree_sitter_cpp::language();
        parser
            .set_language(&language)
            .expect("Error loading C++ grammar");

        // List top-level node types directly, filtering by parent later
        let query = Query::new(
            &language,
            r#"
            [
              (function_definition) @func
              (declaration) @declaration
              (class_specifier) @class
              (struct_specifier) @struct
              (namespace_definition) @namespace
              (enum_specifier) @enum
              (template_declaration) @template
              (if_statement) @if
              (for_statement) @for
              (while_statement) @while
              (do_statement) @do
              (switch_statement) @switch
              (try_statement) @try
              (expression_statement) @expr_stmt
              (preproc_include) @include
              (preproc_def) @define
            ]
            "#,
        )
        .expect("Error creating C++ query");

        CppParser { parser, query }
    }
}

impl CppParser {
    /// Creates a new `CppParser` with the C++ grammar and queries.
    pub fn new() -> Self {
        Self::default()
    }

    /// Extracts the name of a C++ function or method.
    fn extract_function_name(&self, node: &Node, source_code: &str) -> Option<String> {
        // C++ function names can be in different places depending on the structure
        if let Some(declarator) = node.child_by_field_name("declarator") {
            return self.extract_name_from_declarator(&declarator, source_code);
        }

        // Try to find function_declarator
        if let Some(func_declarator) = self.find_child_by_type(node, "function_declarator") {
            if let Some(declarator) = func_declarator.child_by_field_name("declarator") {
                return self.extract_name_from_declarator(&declarator, source_code);
            }
        }

        None
    }

    /// Recursively extracts the name from a declarator (handles complex C++ syntax).
    fn extract_name_from_declarator(&self, node: &Node, source_code: &str) -> Option<String> {
        match node.kind() {
            "identifier" => {
                return node.utf8_text(source_code.as_bytes()).ok().map(|s| s.to_string());
            }
            "qualified_identifier" => {
                // For qualified names like MyClass::myMethod, take the last part
                if let Some(name) = node.child_by_field_name("name") {
                    return name.utf8_text(source_code.as_bytes()).ok().map(|s| s.to_string());
                }
            }
            "destructor_name" => {
                // For destructors like ~MyClass
                if let Some(name) = node.child_by_field_name("name") {
                    if let Ok(class_name) = name.utf8_text(source_code.as_bytes()) {
                        return Some(format!("~{}", class_name));
                    }
                }
            }
            "operator_name" => {
                // For operator overloads
                return node.utf8_text(source_code.as_bytes()).ok().map(|s| s.to_string());
            }
            _ => {
                // Recursively check children for nested declarators
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i) {
                        if let Some(name) = self.extract_name_from_declarator(&child, source_code) {
                            return Some(name);
                        }
                    }
                }
            }
        }
        None
    }

    /// Extracts the name of a C++ class, struct, or namespace.
    fn extract_type_name(&self, node: &Node, source_code: &str) -> Option<String> {
        if let Some(name_node) = node.child_by_field_name("name") {
            return name_node.utf8_text(source_code.as_bytes()).ok().map(|s| s.to_string());
        }
        None
    }

    /// Helper function to find a child node by type.
    fn find_child_by_type(&self, node: &Node, type_name: &str) -> Option<Node> {
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                if child.kind() == type_name {
                    return Some(child);
                }
                // Recursively search in children
                if let Some(found) = self.find_child_by_type(&child, type_name) {
                    return Some(found);
                }
            }
        }
        None
    }
}

impl SyntaxParser for CppParser {
    fn parse(&mut self, content: &str, file_path: &str) -> Result<Vec<CodeChunk>> {
        let tree = self.parser.parse(content, None)
            .context("Failed to parse C++ content")?;
        
        let root_node = tree.root_node();
        let mut chunks = Vec::new();
        let mut query_cursor = QueryCursor::new();
        
        // Execute the query on the syntax tree
        let matches = query_cursor.matches(&self.query, root_node, content.as_bytes());
        
        for query_match in matches {
            for capture in query_match.captures {
                let node = capture.node;
                let capture_name = self.query.capture_names()[capture.index as usize];
                
                // Skip if node doesn't have a parent (top-level) or if it's not a core element
                if node.parent().is_some() && !is_core_element_type(&node, content.as_bytes()) {
                    continue;
                }
                
                let start_byte = node.start_byte();
                let end_byte = node.end_byte();
                let node_content = &content[start_byte..end_byte];
                
                let start_point = node.start_position();
                let end_point = node.end_position();
                
                // Extract element name based on type
                let element_name = match capture_name {
                    "func" => self.extract_function_name(&node, content),
                    "class" | "struct" | "namespace" | "enum" => self.extract_type_name(&node, content),
                    "template" => {
                        // For templates, try to get the templated declaration name
                        if let Some(declaration) = node.child_by_field_name("declaration") {
                            match declaration.kind() {
                                "function_definition" => self.extract_function_name(&declaration, content),
                                "class_specifier" | "struct_specifier" => self.extract_type_name(&declaration, content),
                                _ => Some("template".to_string()),
                            }
                        } else {
                            Some("template".to_string())
                        }
                    },
                    "include" => {
                        // Extract included file name
                        node.utf8_text(content.as_bytes()).ok()
                            .and_then(|text| text.split('"').nth(1).or_else(|| text.split('<').nth(1)?.split('>').next()))
                            .map(|s| s.to_string())
                    },
                    "define" => {
                        // Extract macro name
                        if let Some(name) = node.child_by_field_name("name") {
                            name.utf8_text(content.as_bytes()).ok().map(|s| s.to_string())
                        } else {
                            Some("macro".to_string())
                        }
                    },
                    _ => None,
                };
                
                let chunk = CodeChunk {
                    content: node_content.to_string(),
                    file_path: file_path.to_string(),
                    start_line: start_point.row + 1,
                    end_line: end_point.row + 1,
                    start_byte,
                    end_byte,
                    element_type: capture_name.to_string(),
                    element_name,
                    language: "cpp".to_string(),
                };
                
                chunks.push(chunk);
            }
        }
        
        Ok(chunks)
    }
}