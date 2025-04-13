pub mod parser;
pub mod rust;
pub mod fallback;
pub mod ruby;
pub mod golang;
pub mod javascript;
pub mod typescript;
// pub mod markdown;
pub mod yaml;

use anyhow::{Context, Result};
use std::path::Path;
// Remove tree-sitter imports added for inlining
// use tree_sitter::{Node, Parser, Query, QueryCursor};

use self::parser::SyntaxParser;

use crate::syntax::fallback::FallbackParser;
use crate::syntax::rust::RustParser;
use crate::syntax::ruby::RubyParser;
use crate::syntax::golang::GolangParser;
// use crate::syntax::markdown::MarkdownParser;

// Re-export the CodeChunk type for easier access
pub use self::parser::CodeChunk;


// --- START Inlined Markdown Parser --- START --- //
// --- REMOVE ALL INLINED MARKDOWN PARSER CODE --- //
// --- END Inlined Markdown Parser --- END --- //


/// Determines the appropriate parser based on file extension and returns code chunks.
pub fn get_chunks(file_path: &Path) -> Result<Vec<CodeChunk>> {
    let extension = file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|s| s.to_lowercase()) // Convert to lowercase
        .unwrap_or_default();
    let code = std::fs::read_to_string(file_path)?;

    let mut parser: Box<dyn SyntaxParser> = match extension.as_str() { // Match against the lowercase string
        "rs" => Box::new(RustParser::new()),
        "rb" => Box::new(RubyParser::new()),
        "go" => Box::new(GolangParser::new()),
        // "md" | "mdx" => Box::new(MarkdownParser::new()), // Ensure this is commented out or removed
        // Add cases for other languages here as they are implemented
        // "js" | "jsx" => Box::new(JavaScriptParser::new()),
        // "ts" | "tsx" => Box::new(TypeScriptParser::new()),
        // "yaml" | "yml" => Box::new(YamlParser::new()),
        _ => Box::new(FallbackParser::new()),
    };

    parser.parse(&code, file_path.to_str().unwrap_or(""))
}

// Add test module declaration
#[cfg(test)]
mod rust_tests;
#[cfg(test)]
mod ruby_tests;
#[cfg(test)]
mod golang_tests;
// #[cfg(test)]
// mod markdown_tests; // Ensure this is commented out or removed 