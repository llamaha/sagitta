use crate::mcp::types::{
    ErrorObject, InitializeParams, InitializeResult, ServerCapabilities, ServerInfo,
};
use crate::handlers::tool::get_tool_definitions; // Import tool definitions
use anyhow::Result;
use std::collections::HashMap;

/// Handles the MCP initialize request.
pub async fn handle_initialize(
    params: InitializeParams,
) -> Result<InitializeResult, ErrorObject> {
    // TODO: Potentially validate params.protocol_version against supported versions
    
    // Get tool definitions and format them for the response
    let tools = get_tool_definitions()
        .into_iter()
        .map(|tool| (tool.name.clone(), tool))
        .collect::<HashMap<_, _>>();

    let result = InitializeResult {
        protocol_version: params.protocol_version, // Echo back client version for now
        server_info: ServerInfo {
            name: "sagitta-mcp".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(), // Use crate version
        },
        capabilities: ServerCapabilities {
            tools, 
            // Initialize other capability fields if added later
        },
    };

    Ok(result)
} 