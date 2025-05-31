use crate::mcp::{
    error_codes,
    types::{
        CallToolParams, CallToolResult, ContentBlock, ErrorObject, PingParams, QueryParams,
        RepositoryAddParams, RepositoryListParams, RepositoryRemoveParams, RepositorySyncParams,
        RepositoryMapParams, ToolAnnotations, ToolDefinition,
        RepositorySearchFileParams, RepositoryViewFileParams,
        RepositorySwitchBranchParams, RepositoryListBranchesParams,
    },
};
use crate::server::{deserialize_value, ok_some, result_to_call_result}; // Import necessary helpers
use crate::handlers::{ping::handle_ping, query::handle_query, repository::*, repository_map::handle_repository_map}; // Import actual handlers

use anyhow::Result;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::instrument;
use sagitta_search::{config::AppConfig, embedding::EmbeddingHandler, qdrant_client_trait::QdrantClientTrait};

#[instrument(skip(config, qdrant_client), fields(tool_name = %params.name))]
pub async fn handle_tools_call<C: QdrantClientTrait + Send + Sync + 'static>(
    params: CallToolParams,
    config: Arc<RwLock<AppConfig>>,
    qdrant_client: Arc<C>,
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
            match handle_repository_add(add_params, config, qdrant_client, None).await {
                Ok(res) => result_to_call_result(res),
                Err(e) => Err(e),
            }
        }
        "repository_list" => {
            let list_params: RepositoryListParams = deserialize_value(arguments, tool_name)?;
             // Call imported handler
            match handle_repository_list(list_params, config, None).await {
                Ok(res) => result_to_call_result(res),
                Err(e) => Err(e),
            }
        }
        "repository_remove" => {
            let remove_params: RepositoryRemoveParams = deserialize_value(arguments, tool_name)?;
             // Call imported handler
            match handle_repository_remove(remove_params, config, qdrant_client, None).await {
                Ok(res) => result_to_call_result(res),
                Err(e) => Err(e),
            }
        }
        "repository_sync" => {
            let sync_params: RepositorySyncParams = deserialize_value(arguments, tool_name)?;
             // Call imported handler
            match handle_repository_sync(sync_params, config, qdrant_client, None).await {
                Ok(res) => result_to_call_result(res),
                Err(e) => Err(e),
            }
        }
        "query" => {
            let query_params: QueryParams = deserialize_value(arguments, tool_name)?;
             // Call imported handler
            match handle_query(query_params, config, qdrant_client, None).await {
                Ok(res) => result_to_call_result(res),
                Err(e) => Err(e),
            }
        }
        "repository_search_file" => {
            let search_params: RepositorySearchFileParams = deserialize_value(arguments, tool_name)?;
             match handle_repository_search_file(search_params, config, None).await {
                Ok(res) => result_to_call_result(res),
                Err(e) => Err(e),
            }
        }
        "repository_view_file" => {
            let view_params: RepositoryViewFileParams = deserialize_value(arguments, tool_name)?;
             match handle_repository_view_file(view_params, config, None).await {
                Ok(res) => result_to_call_result(res),
                Err(e) => Err(e),
            }
        }
        "repository_map" => {
            let map_params: RepositoryMapParams = deserialize_value(arguments, tool_name)?;
             match handle_repository_map(map_params, config, None).await {
                Ok(res) => result_to_call_result(res),
                Err(e) => Err(e),
            }
        }
        "repository_switch_branch" => {
            let switch_params: RepositorySwitchBranchParams = deserialize_value(arguments, tool_name)?;
             match handle_repository_switch_branch(switch_params, config, qdrant_client, None).await {
                Ok(res) => result_to_call_result(res),
                Err(e) => Err(e),
            }
        }
        "repository_list_branches" => {
            let list_branches_params: RepositoryListBranchesParams = deserialize_value(arguments, tool_name)?;
             match handle_repository_list_branches(list_branches_params, config, None).await {
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
            description: Some(
                "Performs semantic search on an indexed repository.\n\
\n**Best Practices for Effective Queries:**\n\
- This system uses hybrid (dense + sparse vector) search. Combine natural language and code terms for best results (e.g., `jwt middleware function`, `How is authentication handled?`).\n\
- Use the `elementType` argument (e.g., `function`, `struct`, `enum`) to restrict results to specific code elements.\n\
- For API/code navigation, use `elementType` and include function/struct names or signatures in your query.\n\
- For conceptual, documentation, or workflow/config queries, *omit* `elementType` to surface doc comments, config files, and broader context.\n\
- For maximum recall, start broad (no `elementType`), then narrow with `elementType` if needed.\n\
- For conceptual/documentation queries, omitting `elementType` or using a more targeted query may be necessary.\n\
- For code block and workflow/config queries, omitting `elementType` surfaces relevant results.\n\
".to_string()
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "repositoryName": { "type": "string", "description": "Name of the repository to query" },
                    "queryText": { "type": "string", "description": "The natural language query text" },
                    "limit": { "type": "integer", "description": "Maximum number of results to return" },
                    "branchName": { "type": "string", "description": "Optional branch to query (defaults to active)" },
                    "elementType": { "type": "string", "description": "Optional: Filter by code element type (e.g., function, struct, enum, etc.). For conceptual/documentation queries, omitting this or using a more targeted query may be necessary. For code block and workflow/config queries, omitting this surfaces relevant results." },
                    "lang": { "type": "string", "description": "Optional: Filter by programming language (e.g., \"rust\", \"python\"). **Highly recommended** for code queries to improve relevance." }
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
        // --- Repository Map ---
        ToolDefinition {
            name: "repository_map".to_string(),
            description: Some("Generates a high-level map of code structure in a repository, showing functions, structs, and their relationships. Use targeted parameters to control scope and verbosity.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "repositoryName": { "type": "string", "description": "Name of the repository to map." },
                    "verbosity": { "type": "integer", "description": "Verbosity level (0=minimal, 1=normal, 2=detailed).", "default": 1, "minimum": 0, "maximum": 2 },
                    "paths": { "type": "array", "items": { "type": "string" }, "description": "Optional: Specific paths to scan within the repository." },
                    "fileExtension": { "type": "string", "description": "Optional: Filter by file extension (e.g., 'rs', 'py', 'js')." }
                },
                "required": ["repositoryName"]
            }),
             annotations: Some(ToolAnnotations {
                title: Some("Map Repository Structure".to_string()),
                read_only_hint: Some(true),
                destructive_hint: Some(false),
                idempotent_hint: Some(true),
                open_world_hint: Some(false),
            }),
        },
        // --- Repository Switch Branch ---
        ToolDefinition {
            name: "repository_switch_branch".to_string(),
            description: Some("Switches to a different branch or Git reference in a repository with automatic resync. This operation will update the repository's active branch/ref and optionally trigger a resync of the vector database to reflect the new content. Supports branches, tags, commits, and remote references.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "repositoryName": { "type": "string", "description": "Name of the repository to switch branches in." },
                    "branchName": { "type": "string", "description": "Target branch name to switch to. Either this or targetRef must be provided, but not both." },
                    "targetRef": { "type": "string", "description": "Optional specific Git ref (tag, commit hash, branch name) to check out. If provided, this ref will be checked out instead of branchName. Supports any valid git reference including tags, commits, and remote branches." },
                    "force": { "type": "boolean", "description": "Force switch even with uncommitted changes (default: false)." },
                    "noAutoResync": { "type": "boolean", "description": "Disable automatic resync after branch switch (default: false)." }
                },
                "required": ["repositoryName"],
                "oneOf": [
                    { "required": ["branchName"] },
                    { "required": ["targetRef"] }
                ]
            }),
             annotations: Some(ToolAnnotations {
                title: Some("Switch Repository Branch/Ref".to_string()),
                read_only_hint: Some(false),
                destructive_hint: Some(false), // Changes state but doesn't delete data
                idempotent_hint: Some(true), // Switching to the same branch/ref is safe
                open_world_hint: Some(false),
            }),
        },
        // --- Repository List Branches ---
        ToolDefinition {
            name: "repository_list_branches".to_string(),
            description: Some("Lists all available branches in a repository, showing the current active branch.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "repositoryName": { "type": "string", "description": "Name of the repository to list branches for." }
                },
                "required": ["repositoryName"]
            }),
             annotations: Some(ToolAnnotations {
                title: Some("List Repository Branches".to_string()),
                read_only_hint: Some(true),
                destructive_hint: Some(false),
                idempotent_hint: Some(true),
                open_world_hint: Some(false),
            }),
        },
    ]
}
