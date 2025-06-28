pub mod claude_code;

use crate::tools::types::ToolDefinition;

/// Provider-specific system prompt generation
pub trait SystemPromptProvider {
    /// Generate system prompt for the provider with tool definitions
    fn generate_system_prompt(&self, tool_definitions: &[ToolDefinition]) -> String;
}