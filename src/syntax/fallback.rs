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
        // Simple line-based chunking as a fallback
        // TODO: Improve this fallback - maybe larger fixed-size chunks?
        let chunks = code
            .lines()
            .enumerate()
            .map(|(i, line)| CodeChunk {
                content: line.to_string(),
                file_path: file_path.to_string(),
                start_line: i + 1,
                end_line: i + 1,
                language: "fallback".to_string(),
                element_type: "fallback_chunk".to_string(),
            })
            .collect();
        Ok(chunks)
    }
} 