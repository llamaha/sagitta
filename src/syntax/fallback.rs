// Fallback parser for unsupported languages or errors
use anyhow::Result;
use crate::syntax::parser::{CodeChunk, SyntaxParser};

pub struct FallbackParser;

impl FallbackParser {
    pub fn new() -> Self {
        FallbackParser
    }
}

impl SyntaxParser for FallbackParser {
    fn parse(&mut self, code: &str, file_path: &str) -> Result<Vec<CodeChunk>> {
        // Treat the entire file as a single chunk in the fallback case.
        let num_lines = code.lines().count();
        let chunk = CodeChunk {
            content: code.to_string(),
            file_path: file_path.to_string(),
            start_line: 1,
            // Use max(1, num_lines) to handle empty files correctly (line count 0).
            end_line: std::cmp::max(1, num_lines),
            language: "fallback".to_string(),
            element_type: "fallback_chunk".to_string(),
        };
        Ok(vec![chunk])
    }
} 