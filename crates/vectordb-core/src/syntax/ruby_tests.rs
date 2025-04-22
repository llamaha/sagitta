// src/syntax/ruby_tests.rs
#[cfg(test)]
mod tests {
    // Use super::... to access items from the parent syntax module
    use crate::syntax::parser::{CodeChunk, SyntaxParser};
    use crate::syntax::ruby::RubyParser; // Corrected import
    use anyhow::Result;

    // Helper function to create a parser instance
    fn create_parser() -> RubyParser {
        RubyParser::new()
    }

    // Helper to assert chunk properties
    // ... existing code ...
}