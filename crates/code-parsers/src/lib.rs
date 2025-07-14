//! # Code Parsers
//! 
//! This crate provides code parsing and chunking functionality for various programming languages
//! using tree-sitter parsers. It extracts meaningful code chunks with metadata for indexing
//! and search purposes.

use std::path::Path;

// Language-specific parsers
pub mod rust;
pub mod python;
pub mod cpp_tests;
pub mod javascript;
pub mod typescript;
pub mod golang;
pub mod cpp;
pub mod ruby;
pub mod markdown;
pub mod yaml;
pub mod html;
pub mod fallback;

// Core parser trait and types
pub mod parser;
pub mod languages;
pub mod element_filter;

// Re-export core types for convenience
pub use parser::{CodeChunk, SyntaxParser};
pub use languages::get_language_from_extension;

/// Main entry point for parsing files into code chunks
/// 
/// This function determines the appropriate parser based on file extension
/// and returns a vector of code chunks with metadata.
pub fn get_chunks(file_path: &Path) -> Result<Vec<CodeChunk>, Box<dyn std::error::Error + Send + Sync>> {
    let extension = file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");

    let language = get_language_from_extension(extension);
    
    // Read the file content
    let content = std::fs::read_to_string(file_path)?;
    let file_path_str = file_path.to_string_lossy();
    
    match language.as_str() {
        "rust" => {
            let mut parser = rust::RustParser::new();
            Ok(parser.parse(&content, &file_path_str)?)
        }
        "python" => {
            let mut parser = python::PythonParser::new();
            Ok(parser.parse(&content, &file_path_str)?)
        }
        "javascript" => {
            let mut parser = javascript::JavaScriptParser::new();
            Ok(parser.parse(&content, &file_path_str)?)
        }
        "typescript" => {
            let mut parser = typescript::TypeScriptParser::new();
            Ok(parser.parse(&content, &file_path_str)?)
        }
        "go" => {
            let mut parser = golang::GolangParser::new();
            Ok(parser.parse(&content, &file_path_str)?)
        }
        "cpp" => {
            let mut parser = cpp::CppParser::new();
            Ok(parser.parse(&content, &file_path_str)?)
        }
        "ruby" => {
            let mut parser = ruby::RubyParser::new();
            Ok(parser.parse(&content, &file_path_str)?)
        }
        "markdown" => {
            let mut parser = markdown::MarkdownParser::new();
            Ok(parser.parse(&content, &file_path_str)?)
        }
        "yaml" => {
            let mut parser = yaml::YamlParser::new();
            Ok(parser.parse(&content, &file_path_str)?)
        }
        "html" => {
            let mut parser = html::HtmlParser::new();
            Ok(parser.parse(&content, &file_path_str)?)
        }
        _ => {
            let mut parser = fallback::FallbackParser::new();
            Ok(parser.parse(&content, &file_path_str)?)
        }
    }
} 