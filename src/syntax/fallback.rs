// Fallback parser for unsupported languages or errors
use anyhow::Result;
use crate::syntax::parser::{CodeChunk, SyntaxParser};

const MAX_CHUNK_LINES: usize = 500;

pub struct FallbackParser;

impl FallbackParser {
    pub fn new() -> Self {
        FallbackParser
    }
}

impl SyntaxParser for FallbackParser {
    fn parse(&mut self, code: &str, file_path: &str) -> Result<Vec<CodeChunk>> {
        let lines: Vec<&str> = code.lines().collect();
        let mut chunks = Vec::new();
        let mut current_line_start = 1;

        if lines.is_empty() {
            // Handle empty file: Create one empty chunk
            chunks.push(CodeChunk {
                content: "".to_string(),
                file_path: file_path.to_string(),
                start_line: 1,
                end_line: 1,
                language: "fallback".to_string(),
                element_type: "fallback_chunk".to_string(),
            });
            return Ok(chunks);
        }

        for (i, line_chunk) in lines.chunks(MAX_CHUNK_LINES).enumerate() {
            let content = line_chunk.join("\n");
            let start_line = current_line_start;
            let end_line = start_line + line_chunk.len() - 1;

            chunks.push(CodeChunk {
                content,
                file_path: file_path.to_string(),
                start_line,
                end_line,
                language: "fallback".to_string(),
                element_type: format!("fallback_chunk_{}", i),
            });

            current_line_start = end_line + 1;
        }

        Ok(chunks)
    }
} 