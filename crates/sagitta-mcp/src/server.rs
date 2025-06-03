use crate::mcp::types::{
    ErrorObject, InitializeParams, InitializeResult, MCPRequest, MCPResponse, PingParams, PingResult, QueryParams,
    QueryResult, RepositoryAddParams, RepositoryAddResult, RepositoryInfo, RepositoryListParams,
    RepositoryListResult, RepositorySyncParams, RepositorySyncResult, RepositoryRemoveParams, RepositoryRemoveResult, SearchResultItem,
    Request, Response, ServerInfo, ServerCapabilities, ListToolsParams, ListToolsResult, ToolDefinition, InitializedNotificationParams, ToolAnnotations,
    CallToolParams, CallToolResult, ContentBlock, RepositorySearchFileParams, RepositoryViewFileParams,
    RepositorySwitchBranchParams, RepositorySwitchBranchResult, RepositoryListBranchesParams, RepositoryListBranchesResult,
};
use crate::mcp::error_codes;
use anyhow::{anyhow, Context, Result};
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::RwLock;
use tracing::{error, info, warn, instrument};
use sagitta_search::{
    config::{AppConfig, get_repo_base_path, save_config, load_config, RepositoryConfig},
    constants::{
        FIELD_BRANCH, FIELD_CHUNK_CONTENT, FIELD_END_LINE, FIELD_FILE_PATH, FIELD_START_LINE,
    },
    EmbeddingHandler,
    error::SagittaError,
    qdrant_client_trait::QdrantClientTrait,
    repo_add::{AddRepoArgs, handle_repo_add, AddRepoError},
    repo_helpers::{
        get_collection_name, delete_repository_data, index_files,
    },
    search_collection,
    indexing::{self, index_repo_files, gather_files},
    sync::{sync_repository, SyncOptions},
};
use qdrant_client::{
    qdrant::{Filter, Condition, value::Kind, FieldCondition, Match, Value as QdrantValue},
    Qdrant, QdrantError
};
use serde_json::json;
use serde_json::Value;
use crate::handlers::ping::handle_ping;
use crate::handlers::query::handle_query;
use crate::handlers::repository::{
    handle_repository_add,
    handle_repository_list,
    handle_repository_remove,
    handle_repository_sync,
    handle_repository_search_file,
    handle_repository_view_file,
    handle_repository_switch_branch,
    handle_repository_list_branches,
};
use crate::handlers::tool::{handle_tools_call, get_tool_definitions};
use crate::handlers::initialize::handle_initialize;

#[derive(Debug)]
pub struct Server<C: QdrantClientTrait + Send + Sync + 'static> {
    config: Arc<RwLock<AppConfig>>,
    qdrant_client: Arc<C>,
}

impl Server<Qdrant> {
    pub async fn new(config: AppConfig) -> Result<Self> {
        let api_key = if let Some(path_str) = &config.server_api_key_path {
            match fs::read_to_string(path_str) {
                Ok(key) => Some(key.trim().to_string()),
                Err(e) => {
                    warn!(path = %path_str, error = %e, "Failed to read API key file, proceeding without API key");
                    None
                }
            }
        } else {
            None
        };

        let mut client_builder = Qdrant::from_url(&config.qdrant_url);
        if let Some(key) = api_key {
            if !key.is_empty() {
                client_builder.set_api_key(&key);
            }
        }
        let qdrant_instance = client_builder.build()
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

impl<C: QdrantClientTrait + Send + Sync + 'static> Server<C> {
    /// Creates a new Server instance, primarily for testing purposes.
    ///
    /// Takes pre-initialized components as Arcs.
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

                    let response = match serde_json::from_str::<Request>(trimmed_line) {
                        Ok(request) => {
                            let request_id = request.id.clone();
                            
                            let result = self.handle_request(request).await;

                            match result {
                                Ok(Some(result)) => Some(Response::success(result, request_id)),
                                Ok(None) => None,
                                Err(err_obj) => Some(Response::error(err_obj, request_id)),
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, "Failed to parse request JSON");
                            Some(Response::error(
                                ErrorObject {
                                    code: error_codes::PARSE_ERROR,
                                    message: format!("Failed to parse request: {}", e),
                                    data: None,
                                },
                                None,
                            ))
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
        request: Request,
    ) -> Result<Option<serde_json::Value>, ErrorObject> {
        if request.jsonrpc != "2.0" {
            return Err(ErrorObject {
                code: error_codes::INVALID_REQUEST,
                message: "Invalid jsonrpc version".to_string(),
                data: None,
            });
        }

        let config = Arc::clone(&self.config);
        let qdrant_client = Arc::clone(&self.qdrant_client);

        match request.method.as_str() {
            "initialize" | "mcp_sagitta_mcp_initialize" => {
                let params: InitializeParams = deserialize_params(request.params, "initialize")?;
                let result = handle_initialize(params).await?;
                ok_some(result)
            }
            "initialized" | "notifications/initialized" | "mcp_sagitta_mcp_initialized" => {
                let _params: InitializedNotificationParams = deserialize_params(request.params, "initialized")?;
                info!("Received initialized notification");
                Ok(None)
            }
            "ping" | "mcp_sagitta_mcp_ping" => {
                let params: PingParams = deserialize_params(request.params, "ping")?;
                let result = handle_ping(params).await?;
                ok_some(result)
            }
            "query" | "mcp_sagitta_mcp_query" => {
                let params: QueryParams = deserialize_params(request.params, "query")?;
                let result = handle_query(params, config, qdrant_client, None).await?;
                ok_some(result)
            }
            "repository/add" | "mcp_sagitta_mcp_repository_add" => {
                let params: RepositoryAddParams = deserialize_params(request.params, "repository/add")?;
                let result = handle_repository_add(params, config, qdrant_client, None).await?;
                ok_some(result)
            }
            "repository/list" | "mcp_sagitta_mcp_repository_list" => {
                let params: RepositoryListParams = deserialize_params(request.params, "repository/list")?;
                let result = handle_repository_list(params, config, None).await?;
                ok_some(result)
            }
            "repository/remove" | "mcp_sagitta_mcp_repository_remove" => {
                let params: RepositoryRemoveParams = deserialize_params(request.params, "repository/remove")?;
                let result = handle_repository_remove(params, config, qdrant_client, None).await?;
                ok_some(result)
            }
            "repository/sync" | "mcp_sagitta_mcp_repository_sync" => {
                let params: RepositorySyncParams = deserialize_params(request.params, "repository/sync")?;
                let result = handle_repository_sync(params, config, qdrant_client, None).await?;
                ok_some(result)
            }
            "repository/search_file" | "mcp_sagitta_mcp_repository_search_file" => {
                let params: RepositorySearchFileParams = deserialize_params(request.params, "repository/search_file")?;
                let result = handle_repository_search_file(params, config, None).await?;
                ok_some(result)
            }
            "repository/view_file" | "mcp_sagitta_mcp_repository_view_file" => {
                let params: RepositoryViewFileParams = deserialize_params(request.params, "repository/view_file")?;
                let result = handle_repository_view_file(params, config, None).await?;
                ok_some(result)
            }
            "repository/switch_branch" | "mcp_sagitta_mcp_repository_switch_branch" => {
                let params: RepositorySwitchBranchParams = deserialize_params(request.params, "repository/switch_branch")?;
                let result = handle_repository_switch_branch(params, config, qdrant_client, None).await?;
                ok_some(result)
            }
            "repository/list_branches" | "mcp_sagitta_mcp_repository_list_branches" => {
                let params: RepositoryListBranchesParams = deserialize_params(request.params, "repository/list_branches")?;
                let result = handle_repository_list_branches(params, config, None).await?;
                ok_some(result)
            }
            "tools/list" | "mcp_sagitta_mcp_tools_list" => {
                let _params: ListToolsParams = deserialize_params(request.params, "tool/list")?;
                let tools = get_tool_definitions();
                let result = ListToolsResult { tools };
                ok_some(result)
            }
            "tools/call" => {
                let params: CallToolParams = deserialize_params(request.params, "tool/call")?;
                handle_tools_call(params, config, qdrant_client).await
            }
            _ => Err(ErrorObject {
                code: error_codes::METHOD_NOT_FOUND,
                message: format!("Method not found: {}", request.method),
                data: None,
            }),
        }
    }

    /// Processes a raw JSON-RPC request string, calls handle_request, and returns a raw JSON-RPC response string.
    pub async fn process_json_rpc_request_str(&self, json_request_str: &str) -> Option<String> {
        match serde_json::from_str::<crate::mcp::types::Request>(json_request_str) {
            Ok(request_to_process) => {
                let request_id_for_response = request_to_process.id.clone();
                match self.handle_request(request_to_process).await {
                    Ok(Some(result_value)) => {
                        let response = crate::mcp::types::Response::success(result_value, request_id_for_response.clone());
                        Some(serde_json::to_string(&response).unwrap_or_else(|e| {
                            warn!(error = %e, "Failed to serialize successful MCP response");
                            let err_obj = ErrorObject {
                                code: error_codes::INTERNAL_ERROR,
                                message: "Failed to serialize response".to_string(),
                                data: None,
                            };
                            serde_json::to_string(&crate::mcp::types::Response::error(err_obj, request_id_for_response)).unwrap()
                        }))
                    }
                    Ok(None) => { 
                        None 
                    }
                    Err(error_object) => { 
                        let response = crate::mcp::types::Response::error(error_object, request_id_for_response.clone());
                        Some(serde_json::to_string(&response).unwrap_or_else(|e| {
                             warn!(error = %e, "Failed to serialize error MCP response");
                             let err_obj = ErrorObject {
                                code: error_codes::INTERNAL_ERROR,
                                message: "Failed to serialize error response object".to_string(),
                                data: None,
                            };
                             serde_json::to_string(&crate::mcp::types::Response::error(err_obj, request_id_for_response.or(Some(serde_json::Value::Null)))).unwrap()
                        }))
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to parse JSON-RPC request string into mcp::types::Request");
                let err_obj = ErrorObject {
                    code: error_codes::PARSE_ERROR,
                    message: format!("Failed to parse request: {}", e),
                    data: None,
                };
                let response = crate::mcp::types::Response::error(err_obj, Some(serde_json::Value::Null));
                Some(serde_json::to_string(&response).unwrap_or_else(|serialize_err| {
                     warn!(error = %serialize_err, "Failed to serialize parse error response");
                     format!("{{\"jsonrpc\": \"2.0\", \"error\": {{\"code\": {}, \"message\": \"{}\"}}, \"id\": null}}", error_codes::PARSE_ERROR, "Parse error and failed to serialize error object")
                 }))
            }
        }
    }

    pub async fn get_config(&self) -> Result<AppConfig> {
        let config = self.config.read().await;
        Ok(config.clone())
    }
}

pub fn map_add_repo_error(e: AddRepoError) -> ErrorObject {
    let (code, message) = match &e {
        AddRepoError::InvalidArgs(msg) => (error_codes::INVALID_PARAMS, msg.clone()),
        AddRepoError::RepoExists(name) => (error_codes::REPO_ALREADY_EXISTS, format!("Repository '{}' already exists.", name)),
        AddRepoError::NameDerivationError(from) => (error_codes::NAME_DERIVATION_FAILED, format!("Could not derive repository name from {}", from)),
        AddRepoError::IoError(io_err) => (error_codes::INTERNAL_ERROR, format!("Filesystem error: {}", io_err)),
        AddRepoError::ConfigError(cfg_err) => (error_codes::CONFIG_SAVE_FAILED, format!("Configuration error: {}", cfg_err)),
        AddRepoError::GitError(git_err) => (error_codes::GIT_OPERATION_FAILED, format!("Git operation failed: {}", git_err)),
        AddRepoError::RepoOpenError(path, open_err) => (error_codes::INTERNAL_ERROR, format!("Failed to open repository at {}: {}", path.display(), open_err)),
        AddRepoError::BranchDetectionError(branch_err) => (error_codes::BRANCH_DETECTION_FAILED, format!("Failed to determine default branch: {}", branch_err)),
        AddRepoError::QdrantError(q_err) => (error_codes::QDRANT_OPERATION_FAILED, format!("Qdrant operation failed: {}", q_err)),
        AddRepoError::EmbeddingError(emb_err) => (error_codes::EMBEDDING_ERROR, format!("Embedding logic error: {}", emb_err)),
        AddRepoError::UrlDeterminationError => (error_codes::URL_DETERMINATION_FAILED, "Failed to determine repository URL.".to_string()),
    };
    
    let error_data = json!({
        "error_type": format!("{:?}", e),
        "details": e.to_string(),
        "source": e.source().map(|s| s.to_string()), 
    });

    ErrorObject {
        code,
        message,
        data: Some(error_data),
    }
}

/// Run the server with a configuration file at the specified path.
pub async fn run_server(config_path: PathBuf) -> Result<()> {
    let config = load_config(Some(&config_path)).context("Failed to load configuration")?;
    
    let server = Server::new(config).await?;
    
    info!("Starting MCP server with config from: {}", config_path.display());
    server.run().await
}

pub fn deserialize_params<T: serde::de::DeserializeOwned + Default>(params: Option<Value>, method_name: &str) -> Result<T, ErrorObject> {
    params.map_or(Ok(T::default()), |p| deserialize_value(p, method_name))
}

pub fn deserialize_value<T: serde::de::DeserializeOwned>(value: Value, method_name: &str) -> Result<T, ErrorObject> {
    serde_json::from_value(value).map_err(|e| ErrorObject {
        code: error_codes::INVALID_PARAMS,
        message: format!("Invalid params/arguments for {}: {}", method_name, e),
        data: None,
    })
}

pub fn ok_some<T: serde::Serialize>(value: T) -> Result<Option<serde_json::Value>, ErrorObject> {
    serde_json::to_value(value)
        .map_err(|e| ErrorObject {
            code: error_codes::INTERNAL_ERROR,
            message: format!("Failed to serialize result: {}", e),
            data: None,
        })
        .map(Some)
}

pub fn result_to_call_result<T: serde::Serialize>(result: T) -> Result<CallToolResult, ErrorObject> {
    match serde_json::to_string_pretty(&result) {
        Ok(text) => Ok(CallToolResult {
            is_error: Some(false),
            content: vec![ContentBlock { block_type: "text".to_string(), text }],
        }),
        Err(e) => Err(ErrorObject {
            code: error_codes::INTERNAL_ERROR,
            message: format!("Failed to serialize tool result: {}", e),
            data: None,
        }),
    }
}

/// Creates a structured JSON Value containing details about the error chain.
pub fn create_error_data(e: &anyhow::Error) -> serde_json::Value {
    let mut sources = Vec::new();
    let mut current: Option<&(dyn std::error::Error + 'static)> = Some(e.as_ref());
    while let Some(err) = current {
        sources.push(err.to_string());
        current = err.source();
    }

    json!({
        "message": e.to_string(),
        "root_cause": e.root_cause().to_string(),
        "sources": sources,
        "sagitta_error_type": e.source()
            .and_then(|source| source.downcast_ref::<SagittaError>())
            .map(|specific| format!("{:?}", specific)),
    })
}