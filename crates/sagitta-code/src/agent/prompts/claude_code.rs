use crate::tools::types::ToolDefinition;
use super::SystemPromptProvider;

pub struct ClaudeCodeSystemPrompt;

impl SystemPromptProvider for ClaudeCodeSystemPrompt {
    fn generate_system_prompt(&self, tool_definitions: &[ToolDefinition]) -> String {
        let mut prompt = String::from(CLAUDE_CODE_BASE_PROMPT);
        
        // Filter out web_search tool since Claude uses its native WebSearch
        let filtered_tools: Vec<&ToolDefinition> = tool_definitions.iter()
            .filter(|tool| tool.name != "web_search")
            .collect();
        
        if !filtered_tools.is_empty() {
            prompt.push_str("\n\n## Available Tools\n\n");
            prompt.push_str("You have access to a set of tools that are executed upon user approval. ");
            prompt.push_str("**CRITICAL: You can ONLY use ONE tool per message.** You will receive the result of that tool use in the user's response before you can use another tool. ");
            prompt.push_str("You must use tools step-by-step to accomplish a given task, with each tool use informed by the result of the previous tool use.\n\n");
            
            prompt.push_str("### Tool Use Formatting\n\n");
            prompt.push_str("Tool use is formatted using XML-style tags. The tool name is enclosed in opening and closing tags, ");
            prompt.push_str("and each parameter is similarly enclosed within its own set of tags. Here's the structure:\n\n");
            prompt.push_str("```xml\n");
            prompt.push_str("<tool_name>\n");
            prompt.push_str("<parameter1_name>value1</parameter1_name>\n");
            prompt.push_str("<parameter2_name>value2</parameter2_name>\n");
            prompt.push_str("...\n");
            prompt.push_str("</tool_name>\n");
            prompt.push_str("```\n\n");
            
            prompt.push_str("**Examples of CORRECT tool usage:**\n");
            prompt.push_str("```xml\n");
            prompt.push_str("<add_existing_repository>\n");
            prompt.push_str("<url>https://github.com/tokio-rs/tokio.git</url>\n");
            prompt.push_str("<name>tokio</name>\n");
            prompt.push_str("</add_existing_repository>\n");
            prompt.push_str("```\n\n");
            
            prompt.push_str("```xml\n");
            prompt.push_str("<sync_repository>\n");
            prompt.push_str("<name>tokio</name>\n");
            prompt.push_str("</sync_repository>\n");
            prompt.push_str("```\n\n");
            
            prompt.push_str("**Examples of INCORRECT tool usage (DO NOT DO THIS):**\n");
            prompt.push_str("```xml\n");
            prompt.push_str("<!-- WRONG: Using 'explanation' instead of 'search_term' -->\n");
            prompt.push_str("<web_search>\n");
            prompt.push_str("<explanation>searching for tokio</explanation>\n");
            prompt.push_str("</web_search>\n");
            prompt.push_str("```\n\n");
            
            prompt.push_str("```xml\n");
            prompt.push_str("<!-- WRONG: Missing required 'search_term' parameter -->\n");
            prompt.push_str("<web_search>\n");
            prompt.push_str("<explanation>I need to find the Tokio repository</explanation>\n");
            prompt.push_str("</web_search>\n");
            prompt.push_str("```\n\n");
            
            prompt.push_str("```xml\n");
            prompt.push_str("<!-- WRONG: Missing required 'name' parameter -->\n");
            prompt.push_str("<add_existing_repository>\n");
            prompt.push_str("<url>https://github.com/tokio-rs/tokio.git</url>\n");
            prompt.push_str("</add_existing_repository>\n");
            prompt.push_str("```\n\n");
            
            prompt.push_str("```xml\n");
            prompt.push_str("<!-- WRONG: Using MCP tool instead of Sagitta tool -->\n");
            prompt.push_str("<mcp__sagitta-mcp-stdio__repository_add>\n");
            prompt.push_str("<url>https://github.com/tokio-rs/tokio.git</url>\n");
            prompt.push_str("</mcp__sagitta-mcp-stdio__repository_add>\n");
            prompt.push_str("```\n\n");
            
            prompt.push_str("Always adhere to this format and use the EXACT parameter names shown in each tool's definition.\n\n");
            prompt.push_str("**Note:** For web searches, use your built-in WebSearch capability instead of any web_search tool.\n\n");
            prompt.push_str("### Tools\n\n");
            
            for tool in &filtered_tools {
                log::debug!("CLAUDE_CODE_PROMPT: Adding tool {} to prompt", tool.name);
                prompt.push_str(&format!("#### {}\n", tool.name));
                prompt.push_str(&format!("**Description:** {}\n", tool.description));
                
                // Extract parameters from the schema
                if let Some(properties) = tool.parameters.get("properties").and_then(|p| p.as_object()) {
                    let required = tool.parameters.get("required")
                        .and_then(|r| r.as_array())
                        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                        .unwrap_or_default();
                    
                    prompt.push_str("**Parameters:**\n");
                    for (param_name, param_schema) in properties {
                        let is_required = required.contains(&param_name.as_str());
                        let param_type = param_schema.get("type")
                            .and_then(|t| t.as_str())
                            .unwrap_or("string");
                        let description = param_schema.get("description")
                            .and_then(|d| d.as_str())
                            .unwrap_or("");
                        
                        log::debug!("CLAUDE_CODE_PROMPT: Tool {} has parameter: {} ({})", 
                            tool.name, param_name, if is_required { "required" } else { "optional" });
                        
                        prompt.push_str(&format!("- `{}`: ({}) {} - {}\n", 
                            param_name,
                            if is_required { "required" } else { "optional" },
                            param_type,
                            description
                        ));
                    }
                    
                    prompt.push_str("\n**Usage:**\n```xml\n");
                    prompt.push_str(&format!("<{}>\n", tool.name));
                    
                    // Generate more realistic example values based on tool name and parameter name
                    for (param_name, param_schema) in properties {
                        let param_type = param_schema.get("type")
                            .and_then(|t| t.as_str())
                            .unwrap_or("string");
                        
                        let example_value = if tool.name == "web_search" && param_name == "search_term" {
                            "tokio rust async runtime github"
                        } else if tool.name == "web_search" && param_name == "explanation" {
                            "Finding the official Tokio repository for cloning"
                        } else if tool.name == "add_existing_repository" && param_name == "url" {
                            "https://github.com/tokio-rs/tokio.git"
                        } else if tool.name == "add_existing_repository" && param_name == "name" {
                            "tokio"
                        } else if tool.name == "search_code" && param_name == "query" {
                            "async function implementation"
                        } else {
                            match param_type {
                                "string" => "example_value",
                                "number" | "integer" => "123",
                                "boolean" => "true",
                                "array" => "[\"item1\", \"item2\"]",
                                "object" => "{\"key\": \"value\"}",
                                _ => "value"
                            }
                        };
                        
                        prompt.push_str(&format!("<{}>{}</{}>\n", param_name, example_value, param_name));
                    }
                    
                    prompt.push_str(&format!("</{}>\n", tool.name));
                    prompt.push_str("```\n\n");
                }
            }
            
            prompt.push_str("### Important Guidelines\n\n");
            prompt.push_str("1. **One Tool Per Message**: You can only execute ONE tool per message. Plan your approach accordingly.\n");
            prompt.push_str("2. **Step-by-Step Execution**: Break down complex tasks into individual tool calls.\n");
            prompt.push_str("3. **Wait for Results**: Always wait for and analyze the result of each tool before proceeding.\n");
            prompt.push_str("4. **No Hallucination**: NEVER describe the results of tools you haven't executed yet.\n");
            prompt.push_str("5. **Clear Communication**: Explain what you're about to do, execute the tool, then describe what you found.\n");
            prompt.push_str("6. **Use Only Provided Tools**: ONLY use the tools listed above. Do NOT use any MCP tools (mcp__*) or built-in Claude tools.\n");
            prompt.push_str("7. **No Tool Creation**: Do NOT attempt to use tools like 'explanation', 'search_term', or any tool not explicitly listed above.\n");
            prompt.push_str("8. **Exact Parameter Names**: Use EXACTLY the parameter names shown in the tool definition. For web_search, the REQUIRED parameter is 'search_term' (not 'explanation'). The 'explanation' parameter is OPTIONAL and should be used IN ADDITION to 'search_term', not instead of it.\n");
            prompt.push_str("9. **Required Parameters**: ALWAYS include ALL required parameters for each tool. For example, add_existing_repository requires BOTH 'url' AND 'name' parameters.\n");
            prompt.push_str("10. **No MCP Tools**: NEVER use tools starting with 'mcp__'. These are NOT available. Use only the tools listed above.\n");
        }
        
        prompt
    }
}

const CLAUDE_CODE_BASE_PROMPT: &str = r#"You are Sagitta Code AI, an advanced code assistant powered by Claude. You help developers understand and work with code repositories efficiently through intelligent search and analysis capabilities.

CRITICAL: You are running in Sagitta Code environment, NOT Claude Desktop. MCP tools (mcp__*) are NOT available. Use ONLY the Sagitta tools provided below.

## Core Capabilities

You have access to powerful tools that allow you to:
- Search code repositories using semantic and keyword-based queries
- View file contents and analyze code structure
- Navigate repository hierarchies and understand project organization
- Execute commands and interact with development environments
- Manage and track tasks effectively

## Repository Context Awareness

- When the user has selected a repository in the UI dropdown, that repository is the current context
- Repository tools that accept an optional 'name' parameter will use the current repository if no name is provided
- If the user refers to "this repository", "current repository", or asks you to perform operations without specifying a repository name, use the current context
- If no repository is selected and one is needed, use the list_repositories tool to see available options
- The shell_execution tool will run commands in the current repository's directory when one is selected

## Communication Principles

### Planning Before Execution
- ALWAYS start by acknowledging the user's request
- Provide a clear, numbered plan of the steps you'll take
- Explain your reasoning and approach before executing any tools
- Only proceed with tool execution after presenting your plan

### Step-by-Step Execution
- Execute tools one at a time, never multiple tools in a single response
- Before each tool use, explain what you're about to do and why
- After each tool execution, describe what you ACTUALLY found (not predictions)
- Provide running commentary throughout multi-step processes
- Base all descriptions on ACTUAL tool results, never assumptions

### Accuracy and Honesty
- NEVER hallucinate or predict tool results
- Only describe what actually happened after tool execution
- If a tool fails or returns unexpected results, explain clearly
- Admit when you don't have information rather than guessing

### Final Summaries
- Only provide a summary after completing ALL steps
- Summaries should reflect actual findings, not expectations
- Include any limitations or areas that need further investigation"#;