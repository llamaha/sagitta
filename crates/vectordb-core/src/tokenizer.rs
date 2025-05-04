//! A simple tokenizer for Rust code, designed for TF-IDF generation.

use lazy_static::lazy_static;
use regex::Regex;

/// Represents the type of token identified.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum TokenKind {
    Identifier, // Includes keywords for now
    Symbol,
    Literal, // String, char, number
    Comment,
    Whitespace,
    Unknown,
}

/// Represents a distinct token identified in the source code.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Token {
    pub text: String,
    pub kind: TokenKind,
    // Optional: Add positional info if needed later
    // pub line: usize,
    // pub column: usize,
}

/// Configuration for the code tokenizer.
#[derive(Debug, Clone, Copy)]
pub struct TokenizerConfig {
    /// If true, include whitespace tokens in the output.
    pub include_whitespace: bool,
    /// If true, include comment tokens in the output.
    pub include_comments: bool,
    /// If true, convert all identifier/keyword tokens to lowercase.
    pub lowercase_identifiers: bool,
}

impl Default for TokenizerConfig {
    fn default() -> Self {
        Self {
            include_whitespace: false,
            include_comments: true, // Often useful context
            lowercase_identifiers: true,
        }
    }
}

lazy_static! {
    // Order matters: Match longer symbols first (e.g., :: before :)
    // TODO: Refine patterns, especially for floats, hex, octal numbers
    static ref TOKEN_REGEX: Regex = Regex::new(r#"(?x)
        (?P<Comment>//[^\n]*|/\*(?:[^*]|\*[^/])*\*/) | # Line or Block comments
        (?P<String>"(?:\\.|[^"\\])*") | # String literals
        (?P<Char>'(?:\\.|[^'\\])') | # Char literals (basic)
        (?P<Lifetime>'[a-zA-Z_][a-zA-Z0-9_]*) | # Lifetimes like 'a
        (?P<Identifier>[a-zA-Z_][a-zA-Z0-9_]*) | # Identifiers and keywords
        (?P<Number>\d+(\.\d+)?) | # Numbers (basic integer/float)
        (?P<Symbol>::|->|==|>=|<=|!=|&&|\|\||[(){}\[\]<>=+\-*/%&|^!~,;:.]) | # Symbols (multi then single)
        (?P<Whitespace>\s+) | # Whitespace
        (?P<Unknown>.) # Catch any other single character as Unknown
    "#).unwrap();
}

/// Tokenizes a Rust code snippet into a vector of typed tokens based on config.
///
/// TODO: Add better error handling for unclosed comments/strings if needed.
pub fn tokenize_code(code: &str, config: &TokenizerConfig) -> Vec<Token> {
    let mut tokens = Vec::new();
    for cap in TOKEN_REGEX.captures_iter(code) {
        let (kind, text_str) = if let Some(m) = cap.name("Comment") {
            (TokenKind::Comment, m.as_str())
        } else if let Some(m) = cap.name("String") {
            (TokenKind::Literal, m.as_str())
        } else if let Some(m) = cap.name("Char") {
            (TokenKind::Literal, m.as_str())
        } else if let Some(m) = cap.name("Lifetime") {
            (TokenKind::Identifier, m.as_str()) // Treat lifetimes like identifiers
        } else if let Some(m) = cap.name("Identifier") {
            (TokenKind::Identifier, m.as_str()) // Keywords are also matched here
        } else if let Some(m) = cap.name("Number") {
            (TokenKind::Literal, m.as_str())
        } else if let Some(m) = cap.name("Symbol") {
            (TokenKind::Symbol, m.as_str())
        } else if let Some(m) = cap.name("Whitespace") {
            (TokenKind::Whitespace, m.as_str())
        } else if let Some(m) = cap.name("Unknown") {
            (TokenKind::Unknown, m.as_str())
        } else {
            (TokenKind::Unknown, "")
        };

        if text_str.is_empty() {
            continue;
        }

        // Apply filtering
        if !config.include_whitespace && kind == TokenKind::Whitespace {
            continue;
        }
        if !config.include_comments && kind == TokenKind::Comment {
            continue;
        }
        // Optionally filter Unknown later?
        // if kind == TokenKind::Unknown { continue; }

        // Apply normalization
        let text = if config.lowercase_identifiers && kind == TokenKind::Identifier {
            text_str.to_lowercase()
        } else {
            text_str.to_string()
        };

        tokens.push(Token {
            text,
            kind,
        });
    }
    tokens
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tokenization_default_config() {
        let code = "fn main() { let Mutex = 1; } // Example with Cap Keyword";
        let config = TokenizerConfig::default(); // lowercase_identifiers=true, include_comments=true
        let tokens = tokenize_code(code, &config);

        let expected_texts = vec![
            "fn", "main", "(", ")", "{", "let", "mutex", // Lowercased
            "=", "1", ";", "}", "// Example with Cap Keyword", // Comment included
        ];
        let expected_kinds = vec![
            TokenKind::Identifier, TokenKind::Identifier, TokenKind::Symbol, TokenKind::Symbol,
            TokenKind::Symbol, TokenKind::Identifier, TokenKind::Identifier, TokenKind::Symbol,
            TokenKind::Literal, TokenKind::Symbol, TokenKind::Symbol, TokenKind::Comment,
        ];

        assert_eq!(tokens.len(), expected_texts.len(), "Token count mismatch");
        for (i, token) in tokens.iter().enumerate() {
            assert_eq!(token.text, expected_texts[i], "Text mismatch at index {}", i);
            assert_eq!(token.kind, expected_kinds[i], "Kind mismatch at index {}", i);
        }
    }

     #[test]
    fn test_tokenization_no_lowercase() {
        let code = "let Mutex = 1;";
        let config = TokenizerConfig { lowercase_identifiers: false, ..Default::default() };
        let tokens = tokenize_code(code, &config);
        let expected_texts = vec!["let", "Mutex", "=", "1", ";"];
        assert_eq!(tokens.len(), expected_texts.len());
        assert_eq!(tokens[1].text, "Mutex"); // Check case preserved
    }

    #[test]
    fn test_tokenization_no_comments() {
        let code = "let x = 1; // comment";
        let config = TokenizerConfig { include_comments: false, ..Default::default() };
        let tokens = tokenize_code(code, &config);
        let expected_texts = vec!["let", "x", "=", "1", ";"];
        assert_eq!(tokens.len(), expected_texts.len());
        for (i, token) in tokens.iter().enumerate() {
             assert_eq!(token.text, expected_texts[i], "Text mismatch at index {}", i);
             assert_ne!(token.kind, TokenKind::Comment);
        }
    }

    #[test]
    fn test_tokenization_include_whitespace() {
        let code = "let x = 1;";
        let config = TokenizerConfig { include_whitespace: true, ..Default::default() };
        let tokens = tokenize_code(code, &config);
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Whitespace));
    }

    // Existing tests adapted for default config (no whitespace, comments included)
    #[test]
    fn test_symbols() {
        let code = "x::y->z != a&&b || c";
        let config = TokenizerConfig::default();
        let tokens = tokenize_code(code, &config);
        let expected_texts = vec!["x", "::", "y", "->", "z", "!=", "a", "&&", "b", "||", "c"];
        let expected_kinds = vec![
            TokenKind::Identifier, TokenKind::Symbol, TokenKind::Identifier, TokenKind::Symbol, TokenKind::Identifier,
            TokenKind::Symbol, TokenKind::Identifier, TokenKind::Symbol,
            TokenKind::Identifier, TokenKind::Symbol, TokenKind::Identifier,
        ];
        assert_eq!(tokens.len(), expected_texts.len());
         for (i, token) in tokens.iter().enumerate() {
             assert_eq!(token.text, expected_texts[i], "Text mismatch at index {}", i);
             assert_eq!(token.kind, expected_kinds[i], "Kind mismatch at index {}", i);
        }
    }

     #[test]
    fn test_string_literal() {
        let code = r#"let s = "hello \"world\" \n";"#;
        let config = TokenizerConfig::default();
        let tokens = tokenize_code(code, &config);
        assert_eq!(tokens[3].kind, TokenKind::Literal); // Specifically check string literal
    }

    #[test]
    fn test_block_comment() {
        let code = "/* comment */ fn /* nested? */";
        let config = TokenizerConfig::default();
        let tokens = tokenize_code(code, &config);
        let expected_texts = vec!["/* comment */", "fn", "/* nested? */"];
        let expected_kinds = vec![
            TokenKind::Comment, TokenKind::Identifier, TokenKind::Comment,
        ];
         assert_eq!(tokens.len(), expected_texts.len());
        for (i, token) in tokens.iter().enumerate() {
             assert_eq!(token.text, expected_texts[i], "Text mismatch at index {}", i);
             assert_eq!(token.kind, expected_kinds[i], "Kind mismatch at index {}", i);
        }
    }

    // TODO: Add more tests for floats, char literals, lifetimes, other edge cases
} 