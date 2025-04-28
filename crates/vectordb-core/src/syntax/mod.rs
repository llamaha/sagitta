// crates/vectordb-core/src/syntax/mod.rs
use std::fs;
use std::path::Path;
use anyhow::{Context, Result};

// Declare the sub-modules within the syntax module
pub mod fallback;
pub mod golang;
pub mod javascript;
pub mod languages;
pub mod markdown;
pub mod parser;
pub mod python;
pub mod ruby;
pub mod rust;
pub mod typescript;
pub mod yaml;
pub mod html;

// Re-export the core trait and struct
pub use parser::{CodeChunk, SyntaxParser};
pub use languages::SUPPORTED_LANGUAGES; // Make supported languages accessible

/// Determines the language, reads the file, and returns code chunks.
/// This is the main entry point for parsing files based on their syntax.
pub fn get_chunks(file_path: &Path) -> Result<Vec<CodeChunk>> {
    let extension = file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or(""); // Default to empty string if no extension

    // Determine the language identifier (e.g., "rs", "py")
    // TODO: Implement a robust way to get language identifier from extension
    // For now, use the extension directly if supported, otherwise fallback
    let language_id = if SUPPORTED_LANGUAGES.contains(extension) {
        extension
    } else {
        "fallback"
    };

    // Instantiate the appropriate parser
    let mut parser: Box<dyn SyntaxParser> = match language_id {
        "rs" => Box::new(rust::RustParser::new()),
        "md" => Box::new(markdown::MarkdownParser::new()),
        "go" => Box::new(golang::GolangParser::new()),
        "js" | "jsx" => Box::new(javascript::JavaScriptParser::new()),
        "ts" | "tsx" => Box::new(typescript::TypeScriptParser::new()),
        "yaml" | "yml" => Box::new(yaml::YamlParser::new()),
        "rb" => Box::new(ruby::RubyParser::new()),
        "py" => Box::new(python::PythonParser::new()),
        "html" => Box::new(html::HtmlParser::new()),
        _ => Box::new(fallback::FallbackParser::new()), // Fallback for unsupported/unknown
    };

    // Read the file content
    let code = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

    // Parse the code
    let file_path_str = file_path.to_string_lossy().into_owned();
    parser.parse(&code, &file_path_str)
        .with_context(|| format!("Failed to parse file: {}", file_path.display()))
}

// Potentially re-export key items if needed by users of the syntax module
// pub use parser::SyntaxParser; // Already re-exported above
// pub use languages::LanguageIdentifier; // No such struct currently
