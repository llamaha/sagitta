pub mod claude_code;

// Tool types removed - tools now via MCP
use crate::llm::client::ToolDefinition;

/// Provider-specific system prompt generation
pub trait SystemPromptProvider {
    /// Generate system prompt for the provider with tool definitions
    fn generate_system_prompt(&self, tool_definitions: &[ToolDefinition]) -> String;
}