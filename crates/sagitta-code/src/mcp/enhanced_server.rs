use crate::mcp::types::*;
use crate::tools::registry::ToolRegistry;
use crate::tools::types::{ToolDefinition as SagittaToolDef, ToolResult};
use crate::utils::errors::SagittaCodeError;
use anyhow::{anyhow, Context, Result};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::RwLock;
use log::{error, info, warn};
use serde_json::{json, Value};

/// Enhanced MCP server that exposes tools from ToolRegistry
#[derive(Debug)]
pub struct EnhancedMcpServer {
    tool_registry: Arc<ToolRegistry>,
}

impl EnhancedMcpServer {
    pub fn new(tool_registry: Arc<ToolRegistry>) -> Self {
        Self { tool_registry }
    }

    pub async fn run(self) -> Result<()> {
        info!("Enhanced MCP server starting...");
        info!("MCP: {} tools registered", self.tool_registry.count().await);
        
        // Log all available tools
        let tools = self.tool_registry.get_definitions().await;
        info!("MCP: Available tools:");
        for tool in &tools {
            info!("  - {}: {}", tool.name, tool.description);
        }
        
        info!("Enhanced MCP server reading from stdin and writing to stdout.");
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();

        let mut reader = BufReader::new(stdin);
        let mut writer = BufWriter::new(stdout);
        let mut line_buf = String::new();

        loop {
            line_buf.clear();
            match reader.read_line(&mut line_buf).await {
                Ok(0) => {
                    info!("Stdin closed, shutting down.");
                    break;
                }
                Ok(_) => {
                    let trimmed_line = line_buf.trim();
                    if trimmed_line.is_empty() {
                        continue;
                    }

                    info!("Received request: {}", trimmed_line);

                    let response = match serde_json::from_str::<MCPRequest>(trimmed_line) {
                        Ok(request) => {
                            let request_id = request.id.clone();
                            
                            let result = self.handle_request(request).await;

                            match result {
                                Ok(Some(result)) => Some(MCPResponse {
                                    jsonrpc: "2.0".to_string(),
                                    result: Some(result),
                                    error: None,
                                    id: request_id,
                                }),
                                Ok(None) => None,
                                Err(err_obj) => Some(MCPResponse {
                                    jsonrpc: "2.0".to_string(),
                                    result: None,
                                    error: Some(err_obj),
                                    id: request_id,
                                }),
                            }
                        }
                        Err(e) => {
                            warn!("Failed to parse request JSON: {}", e);
                            Some(MCPResponse {
                                jsonrpc: "2.0".to_string(),
                                result: None,
                                error: Some(ErrorObject {
                                    code: error_codes::PARSE_ERROR,
                                    message: format!("Failed to parse request: {}", e),
                                    data: None,
                                }),
                                id: None,
                            })
                        }
                    };

                    if let Some(response_to_send) = response {
                        match serde_json::to_string(&response_to_send) {
                            Ok(response_json) => {
                                info!("Sending response: {}", response_json);
                                if let Err(e) = writer.write_all(response_json.as_bytes()).await {
                                    error!("Failed to write response to stdout: {}", e);
                                    break;
                                }
                                if let Err(e) = writer.write_all(b"\n").await {
                                    error!("Failed to write newline to stdout: {}", e);
                                    break;
                                }
                                if let Err(e) = writer.flush().await {
                                    error!("Failed to flush stdout: {}", e);
                                    break;
                                }
                            }
                            Err(e) => {
                                error!("Failed to serialize response: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Error reading from stdin: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    pub async fn handle_request(
        &self,
        request: MCPRequest,
    ) -> Result<Option<serde_json::Value>, ErrorObject> {
        if request.jsonrpc != "2.0" {
            return Err(ErrorObject {
                code: error_codes::INVALID_REQUEST,
                message: "Invalid jsonrpc version".to_string(),
                data: None,
            });
        }

        match request.method.as_str() {
            "initialize" => {
                let _params: InitializeParams = deserialize_params(request.params)?;
                let result = self.handle_initialize().await?;
                Ok(Some(serde_json::to_value(result).unwrap()))
            }
            "initialized" => {
                info!("Received initialized notification");
                Ok(None)
            }
            "ping" => {
                let _params: PingParams = deserialize_params(request.params)?;
                let result = PingResult {
                    message: "pong".to_string(),
                };
                Ok(Some(serde_json::to_value(result).unwrap()))
            }
            "tools/list" => {
                let _params: ListToolsParams = deserialize_params(request.params)?;
                let tools = self.get_tool_definitions().await;
                let result = ListToolsResult { tools };
                Ok(Some(serde_json::to_value(result).unwrap()))
            }
            "tools/call" => {
                let params: CallToolParams = deserialize_params(request.params)?;
                self.handle_tool_call(params).await
            }
            _ => Err(ErrorObject {
                code: error_codes::METHOD_NOT_FOUND,
                message: format!("Method not found: {}", request.method),
                data: None,
            }),
        }
    }

    async fn handle_initialize(&self) -> Result<InitializeResult, ErrorObject> {
        info!("Initializing Enhanced MCP server");
        
        Ok(InitializeResult {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
                }),
                resources: Some(ResourcesCapability {
                    list_changed: Some(false),
                    subscribe: Some(false),
                }),
            },
            server_info: ServerInfo {
                name: "sagitta-mcp-enhanced".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        })
    }

    async fn get_tool_definitions(&self) -> Vec<ToolDefinition> {
        let sagitta_defs = self.tool_registry.get_definitions().await;
        
        // Convert Sagitta tool definitions to MCP tool definitions
        sagitta_defs.into_iter().map(|def| {
            ToolDefinition {
                name: def.name,
                description: def.description,
                input_schema: def.parameters,
            }
        }).collect()
    }

    async fn handle_tool_call(
        &self,
        params: CallToolParams,
    ) -> Result<Option<serde_json::Value>, ErrorObject> {
        // Strip MCP prefix if present (format: mcp__servername__toolname)
        let tool_name = if params.name.starts_with("mcp__") {
            // Find the last occurrence of "__" to get the actual tool name
            if let Some(pos) = params.name.rfind("__") {
                &params.name[pos + 2..]
            } else {
                &params.name
            }
        } else {
            &params.name
        };
        
        log::debug!("MCP: Handling tool call for '{}' (original: '{}')", tool_name, params.name);
        
        // Get the tool from registry
        let tool = self.tool_registry.get(tool_name).await
            .ok_or_else(|| ErrorObject {
                code: error_codes::METHOD_NOT_FOUND,
                message: format!("Tool not found: {} (original name: {})", tool_name, params.name),
                data: None,
            })?;
        
        // Execute the tool
        let result = tool.execute(params.arguments.unwrap_or(Value::Object(serde_json::Map::new())))
            .await
            .map_err(|e| ErrorObject {
                code: error_codes::INTERNAL_ERROR,
                message: format!("Tool execution failed: {}", e),
                data: None,
            })?;
        
        // Convert result to MCP format
        let (content, is_error) = match &result {
            ToolResult::Success(data) => {
                (vec![ContentBlock {
                    content_type: "text".to_string(),
                    text: serde_json::to_string_pretty(data).unwrap_or_else(|_| "{}".to_string()),
                }], false)
            }
            ToolResult::Error { error } => {
                (vec![ContentBlock {
                    content_type: "text".to_string(),
                    text: format!("Error: {}", error),
                }], true)
            }
        };
        
        let call_result = CallToolResult {
            content,
            is_error: Some(is_error),
        };
        
        Ok(Some(serde_json::to_value(call_result).unwrap()))
    }
}

fn deserialize_params<T: serde::de::DeserializeOwned + Default>(params: Option<Value>) -> Result<T, ErrorObject> {
    params.map_or(Ok(T::default()), |p| {
        serde_json::from_value(p).map_err(|e| ErrorObject {
            code: error_codes::INVALID_PARAMS,
            message: format!("Invalid parameters: {}", e),
            data: None,
        })
    })
}