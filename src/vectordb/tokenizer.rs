use anyhow::Result;
use std::collections::HashMap;

pub struct CodeTokenizer {
    vocab: HashMap<String, u32>,
    unk_token: u32,
}

impl CodeTokenizer {
    pub fn new() -> Result<Self> {
        let mut vocab = HashMap::new();
        
        // Special tokens
        vocab.insert("[UNK]".to_string(), 0);
        vocab.insert("[PAD]".to_string(), 1);
        
        // Programming keywords
        let keywords = vec![
            "fn", "let", "mut", "const", "pub", "struct", "enum", "impl", "trait",
            "use", "mod", "return", "if", "else", "for", "while", "loop", "match",
            "break", "continue", "async", "await", "unsafe", "extern", "crate",
            "self", "super", "where", "type", "as", "ref", "box", "move", "static",
            "dyn", "true", "false", "None", "Some", "Result", "Option", "String",
            "Vec", "HashMap", "u32", "i32", "f32", "bool", "char", "str",
        ];
        
        for (idx, keyword) in keywords.into_iter().enumerate() {
            vocab.insert(keyword.to_string(), (idx + 2) as u32);
        }
        
        // Add ASCII characters
        for c in 32..127 {
            let s = c.to_string();
            if !vocab.contains_key(&s) {
                vocab.insert(s, vocab.len() as u32);
            }
        }
        
        Ok(Self {
            vocab,
            unk_token: 0,
        })
    }

    pub fn tokenize(&self, text: &str) -> Result<Vec<u32>> {
        let mut tokens = Vec::new();
        let mut current = String::new();
        
        for c in text.chars() {
            if c.is_whitespace() || c.is_ascii_punctuation() {
                if !current.is_empty() {
                    if let Some(&token) = self.vocab.get(&current) {
                        tokens.push(token);
                    } else {
                        // Handle unknown tokens by splitting into characters
                        for c in current.chars() {
                            if let Some(&token) = self.vocab.get(&c.to_string()) {
                                tokens.push(token);
                            } else {
                                tokens.push(self.unk_token);
                            }
                        }
                    }
                    current.clear();
                }
                // Add punctuation as separate tokens
                if !c.is_whitespace() {
                    if let Some(&token) = self.vocab.get(&c.to_string()) {
                        tokens.push(token);
                    }
                }
            } else {
                current.push(c);
            }
        }
        
        // Handle any remaining token
        if !current.is_empty() {
            if let Some(&token) = self.vocab.get(&current) {
                tokens.push(token);
            } else {
                for c in current.chars() {
                    if let Some(&token) = self.vocab.get(&c.to_string()) {
                        tokens.push(token);
                    } else {
                        tokens.push(self.unk_token);
                    }
                }
            }
        }
        
        Ok(tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenizer_creation() {
        let tokenizer = CodeTokenizer::new().unwrap();
        assert!(!tokenizer.vocab.is_empty());
    }

    #[test]
    fn test_basic_tokenization() {
        let tokenizer = CodeTokenizer::new().unwrap();
        let text = "fn main() {";
        let tokens = tokenizer.tokenize(text).unwrap();
        assert!(!tokens.is_empty());
    }

    #[test]
    fn test_unknown_tokens() {
        let tokenizer = CodeTokenizer::new().unwrap();
        let text = "fn main() { let x = 42; }";
        let tokens = tokenizer.tokenize(text).unwrap();
        assert!(!tokens.is_empty());
        assert!(tokens.iter().any(|&t| t == tokenizer.unk_token));
    }
} 