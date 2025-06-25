use tiktoken_rs::{cl100k_base, CoreBPE};
use anyhow::Result;

/// Token counter for LLM message content
#[derive(Clone)]
pub struct TokenCounter {
    encoder: CoreBPE,
}

impl std::fmt::Debug for TokenCounter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenCounter").finish()
    }
}

impl TokenCounter {
    /// Create a new token counter with cl100k_base encoding (used by GPT-4, etc.)
    pub fn new() -> Result<Self> {
        Ok(Self {
            encoder: cl100k_base()?,
        })
    }
    
    /// Count tokens in a text string
    pub fn count_tokens(&self, text: &str) -> usize {
        self.encoder.encode_with_special_tokens(text).len()
    }
    
    /// Count tokens for a complete message including role
    pub fn count_message_tokens(&self, role: &str, content: &str) -> usize {
        // Account for message structure overhead (role, content separators)
        // This is an approximation based on OpenAI's token counting
        let message_overhead = 4; // <|im_start|>{role}\n{content}<|im_end|>\n
        let role_tokens = self.count_tokens(role);
        let content_tokens = self.count_tokens(content);
        
        message_overhead + role_tokens + content_tokens
    }
}

impl Default for TokenCounter {
    fn default() -> Self {
        Self::new().expect("Failed to initialize token counter")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_token_counting() {
        let counter = TokenCounter::new().unwrap();
        
        // Basic test
        let text = "Hello, world!";
        let tokens = counter.count_tokens(text);
        assert!(tokens > 0);
        
        // Test message tokens
        let message_tokens = counter.count_message_tokens("user", text);
        assert!(message_tokens > tokens); // Should include overhead
    }
}