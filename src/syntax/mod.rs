//!
//! Handles language-specific parsing using `tree-sitter` to extract
//! meaningful code chunks from source files.

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

// Placeholder for analysis results
#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisResult {
    // TODO: Define fields to store analysis results (e.g., symbols, scopes, relationships)
}

// --- START Inlined Markdown Parser --- START --- //
// --- REMOVE ALL INLINED MARKDOWN PARSER CODE --- //
// --- END Inlined Markdown Parser --- END --- //


/// Analyzes the file content to extract meaningful structures.
///
/// Based on the file extension, it selects the appropriate language parser
/// and performs the analysis.
///
/// # Arguments
///
/// * `file_path` - The path to the source file to analyze.
///
/// # Returns
///
/// A `Result` containing an `AnalysisResult` on success, or an error.
pub fn analyze_file(file_path: &Path) -> Result<AnalysisResult> {
    let extension = file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|s| s.to_lowercase()) // Convert to lowercase
        .unwrap_or_default();
    let _code = std::fs::read_to_string(file_path)?; // Read code, mark as unused for now

    let _parser: Box<dyn SyntaxParser> = match extension.as_str() { // Match against the lowercase string
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

    // TODO: Implement actual analysis using the selected parser
    // For now, return a default/empty result
    Ok(AnalysisResult {})
}

/// Determines the appropriate parser based on file extension, reads the file,
/// and returns a vector of `CodeChunk`s.
///
/// Files with unrecognized or unsupported extensions will be processed using
/// a fallback parser that treats the entire file content as a single chunk.
///
/// # Arguments
///
/// * `file_path` - The path to the source file to parse.
///
/// # Returns
///
/// A `Result` containing a vector of `CodeChunk`s on success, or an error
/// (e.g., if the file cannot be read).
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
mod fallback_tests; // Keep fallback_tests from HEAD/main

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_analyze_file_basic() -> Result<()> {
        // Create a temporary directory
        let dir = tempdir()?;
        let file_path = dir.path().join("test.rs");

        // Create a dummy Rust file
        let mut file = File::create(&file_path)?;
        writeln!(file, "fn main() {{}}")?;

        // Analyze the file
        let result = analyze_file(&file_path)?;

        // Assert that the result is an empty AnalysisResult for now
        assert_eq!(result, AnalysisResult {});

        Ok(())
    }

    #[test]
    fn test_analyze_file_unknown_extension() -> Result<()> {
        let dir = tempdir()?;
        let file_path = dir.path().join("test.unknown");

        let mut file = File::create(&file_path)?;
        writeln!(file, "Some content")?;

        let result = analyze_file(&file_path)?;

        // Expecting the fallback parser to be used, returning an empty result
        assert_eq!(result, AnalysisResult {});

        Ok(())
    }
}
