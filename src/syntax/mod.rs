pub mod parser;
pub mod rust;
pub mod fallback;
pub mod ruby;
pub mod golang;
pub mod javascript;
pub mod typescript;
pub mod markdown;
pub mod yaml;

use anyhow::Result;
use std::path::Path;

use self::parser::SyntaxParser;

use crate::syntax::fallback::FallbackParser;
use crate::syntax::rust::RustParser;

// Re-export the CodeChunk type for easier access
pub use self::parser::CodeChunk;


/// Determines the appropriate parser based on file extension and returns code chunks.
pub fn get_chunks(file_path: &Path) -> Result<Vec<CodeChunk>> {
    let extension = file_path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
    let code = std::fs::read_to_string(file_path)?;

    let mut parser: Box<dyn SyntaxParser> = match extension {
        "rs" => Box::new(RustParser::new()),
        // Add cases for other languages here as they are implemented
        // "rb" => Box::new(RubyParser::new()),
        // "go" => Box::new(GolangParser::new()),
        // "js" | "jsx" => Box::new(JavaScriptParser::new()),
        // "ts" | "tsx" => Box::new(TypeScriptParser::new()),
        // "md" | "mdx" => Box::new(MarkdownParser::new()),
        // "yaml" | "yml" => Box::new(YamlParser::new()),
        _ => Box::new(FallbackParser::new()),
    };

    parser.parse(&code, file_path.to_str().unwrap_or(""))
}

// Add test module declaration
#[cfg(test)]
mod rust_tests; 