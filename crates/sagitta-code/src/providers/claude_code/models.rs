use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeCodeModel {
    pub id: &'static str,
    pub name: &'static str,
    pub context_window: usize,
    pub max_output_tokens: usize,
    pub supports_thinking: bool,
    pub supports_images: bool,
    pub supports_prompt_cache: bool,
}

/// All available Claude Code models
pub const CLAUDE_CODE_MODELS: &[ClaudeCodeModel] = &[
    ClaudeCodeModel {
        id: "claude-sonnet-4-20250514",
        name: "Claude 4 Sonnet",
        context_window: 200000,
        max_output_tokens: 8192,
        supports_thinking: true,
        supports_images: false, // CLI limitation
        supports_prompt_cache: false, // CLI limitation
    },
    ClaudeCodeModel {
        id: "claude-opus-4-20250514",
        name: "Claude 4 Opus",
        context_window: 200000,
        max_output_tokens: 4096,
        supports_thinking: true,
        supports_images: false,
        supports_prompt_cache: false,
    },
    ClaudeCodeModel {
        id: "claude-3-7-sonnet-20250219",
        name: "Claude 3.7 Sonnet",
        context_window: 200000,
        max_output_tokens: 8192,
        supports_thinking: true,
        supports_images: false,
        supports_prompt_cache: false,
    },
    ClaudeCodeModel {
        id: "claude-3-5-sonnet-20241022",
        name: "Claude 3.5 Sonnet",
        context_window: 200000,
        max_output_tokens: 8192,
        supports_thinking: true,
        supports_images: false,
        supports_prompt_cache: false,
    },
    ClaudeCodeModel {
        id: "claude-3-5-haiku-20241022",
        name: "Claude 3.5 Haiku",
        context_window: 200000,
        max_output_tokens: 8192,
        supports_thinking: true,
        supports_images: false,
        supports_prompt_cache: false,
    },
];

impl ClaudeCodeModel {
    pub fn find_by_id(id: &str) -> Option<&'static ClaudeCodeModel> {
        CLAUDE_CODE_MODELS.iter().find(|m| m.id == id)
    }
    
    pub fn default() -> &'static ClaudeCodeModel {
        &CLAUDE_CODE_MODELS[0] // claude-sonnet-4-20250514
    }
}