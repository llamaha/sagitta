use crate::mcp::{
    error_codes,
    types::{
        CallToolParams, CallToolResult, ContentBlock, ErrorObject, PingParams, QueryParams,
        RepositoryAddParams, RepositoryListParams, RepositoryRemoveParams, RepositorySyncParams,
        ToolAnnotations, ToolDefinition,
        RepositorySearchFileParams, RepositoryViewFileParams,
    },
};
use crate::server::{deserialize_value, ok_some, result_to_call_result}; // Import necessary helpers
use crate::handlers::{ping::handle_ping, query::handle_query, repository::*}; // Import actual handlers

use anyhow::Result;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::instrument;
use vectordb_core::{config::AppConfig, embedding::EmbeddingHandler, qdrant_client_trait::QdrantClientTrait};

#[instrument(skip(config, qdrant_client, embedding_handler), fields(tool_name = %params.name))]
pub async fn handle_tools_call<C: QdrantClientTrait + Send + Sync + 'static>(
    params: CallToolParams,
    config: Arc<RwLock<AppConfig>>,
    qdrant_client: Arc<C>,
    embedding_handler: Arc<EmbeddingHandler>,
) -> Result<Option<serde_json::Value>, ErrorObject> {
    let tool_name = params.name.as_str();
    let arguments = params.arguments;

    let result: Result<CallToolResult, ErrorObject> = match tool_name {
        "ping" => {
            let ping_params: PingParams = deserialize_value(arguments, tool_name)?;
            // Call imported handler
            match handle_ping(ping_params).await {
                Ok(res) => result_to_call_result(res),
                Err(e) => Err(e),
            }
        }
        "repository_add" => {
            let add_params: RepositoryAddParams = deserialize_value(arguments, tool_name)?;
             // Call imported handler
            match handle_repository_add(add_params, config, qdrant_client, embedding_handler).await {
                Ok(res) => result_to_call_result(res),
                Err(e) => Err(e),
            }
        }
        "repository_list" => {
            let list_params: RepositoryListParams = deserialize_value(arguments, tool_name)?;
             // Call imported handler
            match handle_repository_list(list_params, config).await {
                Ok(res) => result_to_call_result(res),
                Err(e) => Err(e),
            }
        }
        "repository_remove" => {
            let remove_params: RepositoryRemoveParams = deserialize_value(arguments, tool_name)?;
             // Call imported handler
            match handle_repository_remove(remove_params, config, qdrant_client).await {
                Ok(res) => result_to_call_result(res),
                Err(e) => Err(e),
            }
        }
        "repository_sync" => {
            let sync_params: RepositorySyncParams = deserialize_value(arguments, tool_name)?;
             // Call imported handler
            match handle_repository_sync(sync_params, config, qdrant_client, embedding_handler).await {
                Ok(res) => result_to_call_result(res),
                Err(e) => Err(e),
            }
        }
        "query" => {
            let query_params: QueryParams = deserialize_value(arguments, tool_name)?;
             // Call imported handler
            match handle_query(query_params, config, qdrant_client, embedding_handler).await {
                Ok(res) => result_to_call_result(res),
                Err(e) => Err(e),
            }
        }
        "repository_search_file" => {
            let search_params: RepositorySearchFileParams = deserialize_value(arguments, tool_name)?;
             match handle_repository_search_file(search_params, config).await {
                Ok(res) => result_to_call_result(res),
                Err(e) => Err(e),
            }
        }
        "repository_view_file" => {
            let view_params: RepositoryViewFileParams = deserialize_value(arguments, tool_name)?;
             match handle_repository_view_file(view_params, config).await {
                Ok(res) => result_to_call_result(res),
                Err(e) => Err(e),
            }
        }
        _ => Err(ErrorObject {
            code: error_codes::METHOD_NOT_FOUND,
            message: format!("Tool '{}' not found or not callable via tools/call", tool_name),
            data: None,
        }),
    };

    match result {
        Ok(call_tool_res) => ok_some(call_tool_res),
        Err(err_obj) => {
            let error_result = CallToolResult {
                is_error: Some(true),
                content: vec![ContentBlock {
                    block_type: "text".to_string(),
                    text: err_obj.message,
                }],
            };
            ok_some(error_result)
        }
    }
}

// Define the tools the server offers
pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        // --- Ping --- 
        ToolDefinition {
            name: "ping".to_string(),
            description: Some("Checks if the server is responsive.".to_string()),
            input_schema: json!({ "type": "object", "properties": {} }),
            annotations: Some(ToolAnnotations {
                title: Some("Ping Server".to_string()),
                read_only_hint: Some(true),
                destructive_hint: None,
                idempotent_hint: Some(true),
                open_world_hint: Some(false),
            }),
        },
        // --- Initialize (Not a typical "tool", but part of MCP handshake) ---
        // Usually not listed as a callable tool by clients after init.

        // --- Repository Add ---
        ToolDefinition {
            name: "repository_add".to_string(),
            description: Some("Clones and adds a new Git repository for indexing.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Unique name for the repository" },
                    "url": { "type": "string", "description": "Remote Git URL (e.g., HTTPS or SSH)" },
                    "local_path": { "type": "string", "description": "Optional path to a pre-existing local repository" },
                    "branch": { "type": "string", "description": "Optional specific branch to checkout" },
                    "ssh_key": { "type": "string", "description": "Optional path to SSH private key for cloning" },
                    "ssh_passphrase": { "type": "string", "description": "Optional passphrase for the SSH key" }
                },
                "required": ["name"] // URL or local_path is implicitly required by core logic
            }),
            annotations: Some(ToolAnnotations {
                title: Some("Add Repository".to_string()),
                read_only_hint: Some(false),
                destructive_hint: Some(false), // Creates data, but not typically destructive
                idempotent_hint: Some(false), // Re-adding might fail or have different results
                open_world_hint: Some(true), // Interacts with Git remotes
            }),
        },
        // --- Repository List ---
        ToolDefinition {
            name: "repository_list".to_string(),
            description: Some("Lists currently configured repositories.".to_string()),
            input_schema: json!({ "type": "object", "properties": {} }), // No input params needed
            annotations: Some(ToolAnnotations {
                title: Some("List Repositories".to_string()),
                read_only_hint: Some(true),
                destructive_hint: Some(false),
                idempotent_hint: Some(true),
                open_world_hint: Some(false),
            }),
        },
        // --- Repository Remove ---
        ToolDefinition {
            name: "repository_remove".to_string(),
            description: Some("Removes a repository configuration and deletes its data.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Name of the repository to remove" }
                },
                "required": ["name"]
            }),
            annotations: Some(ToolAnnotations {
                title: Some("Remove Repository".to_string()),
                read_only_hint: Some(false),
                destructive_hint: Some(true), // Deletes data
                idempotent_hint: Some(true), // Removing a non-existent repo is a no-op (or error)
                open_world_hint: Some(false),
            }),
        },
        // --- Repository Sync ---
        ToolDefinition {
            name: "repository_sync".to_string(),
            description: Some("Fetches latest changes, updates local copy, and re-indexes a repository.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Name of the repository to sync" }
                    // Add options later like branch, force, etc. if needed
                },
                "required": ["name"]
            }),
            annotations: Some(ToolAnnotations {
                title: Some("Sync Repository".to_string()),
                read_only_hint: Some(false),
                destructive_hint: Some(false), // Modifies local state, but based on remote
                idempotent_hint: Some(false), // Syncing again might pull new changes
                open_world_hint: Some(true), // Interacts with Git remotes
            }),
        },
        // --- Query ---
        ToolDefinition {
            name: "query".to_string(),
            description: Some("Performs semantic search on an indexed repository.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "repositoryName": { "type": "string", "description": "Name of the repository to query" },
                    "queryText": { "type": "string", "description": "The natural language query text" },
                    "limit": { "type": "integer", "description": "Maximum number of results to return" },
                    "branchName": { "type": "string", "description": "Optional branch to query (defaults to active)" }
                },
                "required": ["repositoryName", "queryText", "limit"]
            }),
             annotations: Some(ToolAnnotations {
                title: Some("Query Repository".to_string()),
                read_only_hint: Some(true),
                destructive_hint: Some(false),
                idempotent_hint: Some(true),
                open_world_hint: Some(false),
            }),
        },
        // --- Repository Search File ---
        ToolDefinition {
            name: "repository_search_file".to_string(),
            description: Some("Searches for files within a repository using a glob pattern.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "repositoryName": { "type": "string", "description": "Name of the repository to search within." },
                    "pattern": { "type": "string", "description": "Glob pattern to search for (e.g., \"*.rs\")." },
                    "caseSensitive": { "type": "boolean", "description": "Perform case-sensitive matching (default: false)." }
                },
                "required": ["repositoryName", "pattern"]
            }),
             annotations: Some(ToolAnnotations {
                title: Some("Search Repository Files".to_string()),
                read_only_hint: Some(true),
                destructive_hint: Some(false),
                idempotent_hint: Some(true),
                open_world_hint: Some(false),
            }),
        },
        // --- Repository View File ---
        ToolDefinition {
            name: "repository_view_file".to_string(),
            description: Some("Views the content of a specific file within a repository, optionally within a line range.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "repositoryName": { "type": "string", "description": "Name of the repository containing the file." },
                    "filePath": { "type": "string", "description": "Relative path of the file within the repository." },
                    "startLine": { "type": "integer", "description": "Optional start line number (1-based)." },
                    "endLine": { "type": "integer", "description": "Optional end line number (1-based)." }
                },
                "required": ["repositoryName", "filePath"]
            }),
             annotations: Some(ToolAnnotations {
                title: Some("View Repository File".to_string()),
                read_only_hint: Some(true),
                destructive_hint: Some(false),
                idempotent_hint: Some(true),
                open_world_hint: Some(false),
            }),
        },
    ]
}
