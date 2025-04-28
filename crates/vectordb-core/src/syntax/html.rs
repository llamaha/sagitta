use anyhow::Result;
use tree_sitter::Parser;

use super::parser::{CodeChunk, SyntaxParser};
use super::fallback::FallbackParser; // Use fallback logic internally for now

pub struct HtmlParser {
    // We can use the fallback parser's logic initially
    fallback_parser: FallbackParser,
}

impl HtmlParser {
    pub fn new() -> Self {
        // Although we have the grammar, implementing a proper tree-sitter query 
        // for meaningful HTML chunks can be complex (scripts, styles, nested tags).
        // Start by using the fallback line-based chunking logic,
        // but identify the language correctly as "html".
        HtmlParser {
            fallback_parser: FallbackParser::new(),
        }
    }
}

impl SyntaxParser for HtmlParser {
    fn parse(&mut self, code: &str, file_path: &str) -> Result<Vec<CodeChunk>> {
        // Use the fallback parser's logic
        let mut chunks = self.fallback_parser.parse(code, file_path)?;

        // Override the language identifier to "html"
        for chunk in chunks.iter_mut() {
            chunk.language = "html".to_string();
            // Optionally change element_type too, though "fallback_chunk_N" might be okay
            // chunk.element_type = "html_chunk".to_string(); 
        }

        Ok(chunks)
    }
}

// TODO: Implement actual HTML tree-sitter parsing later if needed
// for more granular chunks (e.g., <script>, <style>, specific tags). 