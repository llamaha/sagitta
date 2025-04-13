pub mod parser;
pub mod rust;
pub mod fallback;
pub mod ruby;
pub mod golang;
pub mod javascript;
pub mod languages;
pub mod markdown;
pub mod python;
pub mod typescript;
pub mod yaml;

use anyhow::{Result};
use std::path::Path;
// Remove tree-sitter imports added for inlining
// use tree_sitter::{Node, Parser, Query, QueryCursor};

use self::parser::SyntaxParser;

use crate::syntax::fallback::FallbackParser;
use crate::syntax::rust::RustParser;
use crate::syntax::ruby::RubyParser;
use crate::syntax::golang::GolangParser;
use crate::syntax::python::PythonParser;

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
        "js" | "jsx" => Box::new(crate::syntax::javascript::JavaScriptParser::new()),
        "ts" | "tsx" => Box::new(crate::syntax::typescript::TypeScriptParser::new()),
        "yaml" | "yml" => Box::new(crate::syntax::yaml::YamlParser::new()),
        "md" | "mdx" => Box::new(crate::syntax::markdown::MarkdownParser::new()),
        "py" => Box::new(PythonParser::new()),
        _ => Box::new(FallbackParser::new()),
    };

    parser.parse(&code, file_path.to_str().unwrap_or(""))
}

// Ensure all test module declarations are present and conditional
#[cfg(test)]
mod rust_tests;
#[cfg(test)]
mod ruby_tests;
#[cfg(test)]
mod golang_tests;
#[cfg(test)]
mod javascript_tests;
#[cfg(test)]
mod typescript_tests;
#[cfg(test)]
mod yaml_tests;
#[cfg(test)]
mod markdown_tests;
#[cfg(test)]
mod python_tests; // Ensure python_tests is declared conditionally 
#[cfg(test)]
mod fallback_tests; // Add the new fallback tests module 