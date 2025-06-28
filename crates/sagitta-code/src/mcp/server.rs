use crate::mcp::types::*;
use anyhow::{anyhow, Context, Result};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::RwLock;
use tracing::{error, info, warn, instrument};
use sagitta_search::{
    config::AppConfig,
    qdrant_client_trait::QdrantClientTrait,
};
use qdrant_client::{Qdrant, QdrantError};
use serde_json::json;
use serde_json::Value;

#[derive(Debug)]
pub struct McpServer<C: QdrantClientTrait + Send + Sync + 'static> {
    config: Arc<RwLock<AppConfig>>,
    qdrant_client: Arc<C>,
}

impl McpServer<Qdrant> {
    pub async fn new(config: AppConfig) -> Result<Self> {
        let qdrant_instance = Qdrant::from_url(&config.qdrant_url)
            .build()
            .map_err(|e: QdrantError| anyhow!(e).context("Failed to build Qdrant client"))?;
        
        qdrant_instance.health_check().await.context("Qdrant health check failed")?;
        info!("Qdrant connection successful.");

        let qdrant_client: Arc<Qdrant> = Arc::new(qdrant_instance);
        let config_arc = Arc::new(RwLock::new(config));

        Ok(Self {
            config: config_arc,
            qdrant_client,
        })
    }
}

impl<C: QdrantClientTrait + Send + Sync + 'static> McpServer<C> {
    pub fn new_for_test(
        config: Arc<RwLock<AppConfig>>,
        qdrant_client: Arc<C>,
    ) -> Self {
        Self {
            config,
            qdrant_client,
        }
    }

    pub async fn run(self) -> Result<()> {
        info!("MCP server reading from stdin and writing to stdout.");
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

                    info!(request = %trimmed_line, "Received request");

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
                            warn!(error = %e, "Failed to parse request JSON");
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
                                info!(response = %response_json, "Sending response");
                                if let Err(e) = writer.write_all(response_json.as_bytes()).await {
                                    error!(error = %e, "Failed to write response to stdout");
                                    break;
                                }
                                if let Err(e) = writer.write_all(b"\n").await {
                                    error!(error = %e, "Failed to write newline to stdout");
                                    break;
                                }
                                if let Err(e) = writer.flush().await {
                                    error!(error = %e, "Failed to flush stdout");
                                    break;
                                }
                            }
                            Err(e) => {
                                error!(error = %e, "Failed to serialize response");
                            }
                        }
                    }
                }
                Err(e) => {
                    error!(error = %e, "Error reading from stdin");
                    break;
                }
            }
        }

        Ok(())
    }

    #[instrument(skip(self), fields(request_id = ?request.id, method = %request.method))]
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
                let tools = self.get_tool_definitions();
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
        info!("Initializing MCP server");
        
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
                name: "sagitta-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        })
    }

    fn get_tool_definitions(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                name: "ping".to_string(),
                description: "Simple ping test".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            ToolDefinition {
                name: "repository_list".to_string(),
                description: "List all indexed repositories".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
        ]
    }

    async fn handle_tool_call(
        &self,
        params: CallToolParams,
    ) -> Result<Option<serde_json::Value>, ErrorObject> {
        match params.name.as_str() {
            "ping" => {
                let result = PingResult {
                    message: "pong from tool".to_string(),
                };
                let call_result = CallToolResult {
                    content: vec![ContentBlock {
                        content_type: "text".to_string(),
                        text: serde_json::to_string_pretty(&result).unwrap(),
                    }],
                    is_error: Some(false),
                };
                Ok(Some(serde_json::to_value(call_result).unwrap()))
            }
            "repository_list" => {
                let result = self.handle_repository_list().await?;
                let call_result = CallToolResult {
                    content: vec![ContentBlock {
                        content_type: "text".to_string(),
                        text: serde_json::to_string_pretty(&result).unwrap(),
                    }],
                    is_error: Some(false),
                };
                Ok(Some(serde_json::to_value(call_result).unwrap()))
            }
            _ => Err(ErrorObject {
                code: error_codes::METHOD_NOT_FOUND,
                message: format!("Tool not found: {}", params.name),
                data: None,
            }),
        }
    }

    async fn handle_repository_list(&self) -> Result<RepositoryListResult, ErrorObject> {
        let config_read = self.config.read().await;
        let repositories = config_read.repositories.iter()
            .map(|repo| RepositoryInfo {
                name: repo.name.clone(),
                url: Some(repo.url.clone()),
                local_path: Some(repo.local_path.to_string_lossy().to_string()),
                branch: Some(repo.default_branch.clone()),
            })
            .collect();

        Ok(RepositoryListResult { repositories })
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