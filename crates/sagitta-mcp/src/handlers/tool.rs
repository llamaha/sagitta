use crate::mcp::{
    error_codes,
    types::{
        CallToolParams, CallToolResult, ContentBlock, ErrorObject, PingParams, QueryParams,
        RepositoryAddParams, RepositoryListParams, RepositoryRemoveParams, RepositorySyncParams, ToolAnnotations, ToolDefinition,
        RepositorySearchFileParams, RepositoryViewFileParams,
        RepositorySwitchBranchParams, RepositoryListBranchesParams,
        TodoReadParams, TodoWriteParams,
        EditFileParams, MultiEditFileParams,
        ShellExecuteParams,
        ReadFileParams, WriteFileParams,
        RepositoryGitHistoryParams,
    },
};
use crate::server::{deserialize_value, ok_some, result_to_call_result}; // Import necessary helpers
use crate::handlers::{ping::handle_ping, query::handle_query, repository::*, 
                      // repository_map::handle_repository_map, // DISABLED
                      todo_read::handle_todo_read, todo_write::handle_todo_write,
                      edit_file::handle_edit_file, multi_edit_file::handle_multi_edit_file,
                      shell_execute::handle_shell_execute,
                      read_file::handle_read_file, write_file::handle_write_file,
                      git_history::handle_repository_git_history}; // Import actual handlers

use anyhow::Result;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::instrument;
use sagitta_search::{config::AppConfig, qdrant_client_trait::QdrantClientTrait};

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
        // DISABLED: Repository mapper tool - consumes too many tokens
        // "repository_map" => {
        //     let map_params: RepositoryMapParams = deserialize_value(arguments, tool_name)?;
        //      match handle_repository_map(map_params, config).await {
        //         Ok(res) => result_to_call_result(res),
        //         Err(e) => Err(e),
        //     }
        // }
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
        "todo_read" => {
            let todo_read_params: TodoReadParams = deserialize_value(arguments, tool_name)?;
            match handle_todo_read(todo_read_params, config, qdrant_client, None).await {
                Ok(res) => result_to_call_result(res),
                Err(e) => Err(e),
            }
        }
        "todo_write" => {
            let todo_write_params: TodoWriteParams = deserialize_value(arguments, tool_name)?;
            match handle_todo_write(todo_write_params, config, qdrant_client, None).await {
                Ok(res) => result_to_call_result(res),
                Err(e) => Err(e),
            }
        }
        "edit_file" => {
            let edit_params: EditFileParams = deserialize_value(arguments, tool_name)?;
            match handle_edit_file(edit_params, config, qdrant_client, None).await {
                Ok(res) => result_to_call_result(res),
                Err(e) => Err(e),
            }
        }
        "multi_edit_file" => {
            let multi_edit_params: MultiEditFileParams = deserialize_value(arguments, tool_name)?;
            match handle_multi_edit_file(multi_edit_params, config, qdrant_client, None).await {
                Ok(res) => result_to_call_result(res),
                Err(e) => Err(e),
            }
        }
        "shell_execute" => {
            let shell_params: ShellExecuteParams = deserialize_value(arguments, tool_name)?;
            match handle_shell_execute(shell_params, config, qdrant_client, None).await {
                Ok(res) => result_to_call_result(res),
                Err(e) => Err(e),
            }
        }
        "read_file" => {
            let read_params: ReadFileParams = deserialize_value(arguments, tool_name)?;
            match handle_read_file(read_params, config, qdrant_client, None).await {
                Ok(res) => result_to_call_result(res),
                Err(e) => Err(e),
            }
        }
        "write_file" => {
            let write_params: WriteFileParams = deserialize_value(arguments, tool_name)?;
            match handle_write_file(write_params, config, qdrant_client, None).await {
                Ok(res) => result_to_call_result(res),
                Err(e) => Err(e),
            }
        }
        "repository_git_history" => {
            let history_params: RepositoryGitHistoryParams = deserialize_value(arguments, tool_name)?;
            match handle_repository_git_history(history_params, config, qdrant_client, None).await {
                Ok(res) => result_to_call_result(res),
                Err(e) => Err(e),
            }
        }
        _ => Err(ErrorObject {
            code: error_codes::METHOD_NOT_FOUND,
            message: format!("Tool '{tool_name}' not found or not callable via tools/call"),
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
        // DISABLED: Repository mapper tool - consumes too many tokens
        // // --- Repository Map ---
        // ToolDefinition {
        //     name: "repository_map".to_string(),
        //     description: Some("Generates a high-level map of code structure in a repository, showing functions, structs, and their relationships. Use targeted parameters to control scope and verbosity.".to_string()),
        //     input_schema: json!({
        //         "type": "object",
        //         "properties": {
        //             "repositoryName": { "type": "string", "description": "Name of the repository to map." },
        //             "verbosity": { "type": "integer", "description": "Verbosity level (0=minimal, 1=normal, 2=detailed).", "default": 1, "minimum": 0, "maximum": 2 },
        //             "paths": { "type": "array", "items": { "type": "string" }, "description": "Optional: Specific paths to scan within the repository." },
        //             "fileExtension": { "type": "string", "description": "Optional: Filter by file extension (e.g., 'rs', 'py', 'js')." }
        //         },
        //         "required": ["repositoryName"]
        //     }),
        //      annotations: Some(ToolAnnotations {
        //         title: Some("Map Repository Structure".to_string()),
        //         read_only_hint: Some(true),
        //         destructive_hint: Some(false),
        //         idempotent_hint: Some(true),
        //         open_world_hint: Some(false),
        //     }),
        // },
        // --- Repository Switch Branch ---
        ToolDefinition {
            name: "repository_switch_branch".to_string(),
            description: Some("Switches to a different branch or Git reference in a repository with automatic resync.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "repositoryName": { "type": "string", "description": "Name of the repository to switch branches in." },
                    "branchName": { "type": "string", "description": "Target branch name to switch to. Either this or targetRef must be provided, but not both." },
                    "targetRef": { "type": "string", "description": "Optional specific Git ref (tag, commit hash, branch name) to check out. If provided, this ref will be checked out instead of branchName. Supports any valid git reference including tags, commits, and remote branches." },
                    "force": { "type": "boolean", "description": "Force switch even with uncommitted changes (default: false)." },
                    "noAutoResync": { "type": "boolean", "description": "Disable automatic resync after branch switch (default: false)." }
                },
                "required": ["repositoryName"]
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
            description: Some("Lists branches and references in a repository with optional filtering. Use filters to avoid overwhelming output in large repositories.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "repositoryName": { "type": "string", "description": "Name of the repository to list branches for." },
                    "filter": { "type": "string", "description": "Optional: Filter pattern for branch/ref names (e.g., 'feature*', 'v1.*', 'main'). Supports glob patterns." },
                    "includeRemote": { "type": "boolean", "description": "Include remote branches (default: true)." },
                    "includeTags": { "type": "boolean", "description": "Include tags (default: true)." },
                    "limit": { "type": "integer", "description": "Maximum number of results to return (default: 50, max: 200)." }
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
        
        // --- Todo Read ---
        ToolDefinition {
            name: "todo_read".to_string(),
            description: Some("Reads the current todo list with detailed status information.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
            annotations: Some(ToolAnnotations {
                title: Some("Read Todo List".to_string()),
                read_only_hint: Some(true),
                destructive_hint: Some(false),
                idempotent_hint: Some(true),
                open_world_hint: Some(false),
            }),
        },
        
        // --- Todo Write ---
        ToolDefinition {
            name: "todo_write".to_string(),
            description: Some("Updates the todo list with structured todo items.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "todos": {
                        "type": "array",
                        "description": "Complete list of todos to write (replaces existing todos)",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string", "description": "Unique identifier for the todo" },
                                "content": { "type": "string", "description": "The content/description of the todo" },
                                "status": { 
                                    "type": "string", 
                                    "enum": ["pending", "in_progress", "completed"],
                                    "description": "Current status of the todo" 
                                },
                                "priority": { 
                                    "type": "string", 
                                    "enum": ["low", "medium", "high"],
                                    "description": "Priority level" 
                                },
                                "created_at": { "type": "string", "description": "Optional RFC3339 timestamp when created" },
                                "updated_at": { "type": "string", "description": "Optional RFC3339 timestamp when last updated" }
                            },
                            "required": ["id", "content", "status", "priority"]
                        }
                    }
                },
                "required": ["todos"]
            }),
            annotations: Some(ToolAnnotations {
                title: Some("Write Todo List".to_string()),
                read_only_hint: Some(false),
                destructive_hint: Some(false),
                idempotent_hint: Some(false),
                open_world_hint: Some(false),
            }),
        },
        
        // --- Edit File ---
        ToolDefinition {
            name: "edit_file".to_string(),
            description: Some("Performs exact string replacements in files with diff output.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "The absolute path to the file to edit" },
                    "old_string": { "type": "string", "description": "The text to search for and replace" },
                    "new_string": { "type": "string", "description": "The text to replace it with" },
                    "replace_all": { "type": "boolean", "description": "Replace all occurrences (default: false)" }
                },
                "required": ["file_path", "old_string", "new_string"]
            }),
            annotations: Some(ToolAnnotations {
                title: Some("Edit File".to_string()),
                read_only_hint: Some(false),
                destructive_hint: Some(false),
                idempotent_hint: Some(false),
                open_world_hint: Some(false),
            }),
        },
        
        // --- Multi Edit File ---
        ToolDefinition {
            name: "multi_edit_file".to_string(),
            description: Some("Performs multiple sequential edits to a single file with diff output.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "The absolute path to the file to edit" },
                    "edits": {
                        "type": "array",
                        "description": "Array of edit operations to perform sequentially",
                        "items": {
                            "type": "object",
                            "properties": {
                                "old_string": { "type": "string", "description": "The text to search for and replace" },
                                "new_string": { "type": "string", "description": "The text to replace it with" },
                                "replace_all": { "type": "boolean", "description": "Replace all occurrences (default: false)" }
                            },
                            "required": ["old_string", "new_string"]
                        }
                    }
                },
                "required": ["file_path", "edits"]
            }),
            annotations: Some(ToolAnnotations {
                title: Some("Multi Edit File".to_string()),
                read_only_hint: Some(false),
                destructive_hint: Some(false),
                idempotent_hint: Some(false),
                open_world_hint: Some(false),
            }),
        },
        
        // --- Shell Execute ---
        ToolDefinition {
            name: "shell_execute".to_string(),
            description: Some("Executes shell commands with cross-platform support (Windows/Linux/macOS).".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "The command to execute" },
                    "working_directory": { "type": "string", "description": "Optional working directory (defaults to current directory)" },
                    "timeout_ms": { "type": "integer", "description": "Optional timeout in milliseconds (default: 30000ms)" },
                    "env": { 
                        "type": "object",
                        "description": "Optional environment variables",
                        "additionalProperties": { "type": "string" }
                    }
                },
                "required": ["command"]
            }),
            annotations: Some(ToolAnnotations {
                title: Some("Execute Shell Command".to_string()),
                read_only_hint: Some(false),
                destructive_hint: Some(true),
                idempotent_hint: Some(false),
                open_world_hint: Some(true),
            }),
        },
        
        // --- Read File ---
        ToolDefinition {
            name: "read_file".to_string(),
            description: Some("Reads file contents with optional line range support.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "The absolute path to the file to read" },
                    "start_line": { "type": "integer", "description": "Optional start line (1-based, inclusive)" },
                    "end_line": { "type": "integer", "description": "Optional end line (1-based, inclusive)" }
                },
                "required": ["file_path"]
            }),
            annotations: Some(ToolAnnotations {
                title: Some("Read File".to_string()),
                read_only_hint: Some(true),
                destructive_hint: Some(false),
                idempotent_hint: Some(true),
                open_world_hint: Some(false),
            }),
        },
        
        // --- Write File ---
        ToolDefinition {
            name: "write_file".to_string(),
            description: Some("Writes content to a file with optional parent directory creation.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "The absolute path to the file to write" },
                    "content": { "type": "string", "description": "The content to write to the file" },
                    "create_parents": { "type": "boolean", "description": "Create parent directories if they don't exist (default: true)" }
                },
                "required": ["file_path", "content"]
            }),
            annotations: Some(ToolAnnotations {
                title: Some("Write File".to_string()),
                read_only_hint: Some(false),
                destructive_hint: Some(false),
                idempotent_hint: Some(false),
                open_world_hint: Some(false),
            }),
        },
        
        // --- Repository Git History ---
        ToolDefinition {
            name: "repository_git_history".to_string(),
            description: Some("Retrieves git commit history for a repository with filtering options.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "repositoryName": { "type": "string", "description": "Name of the repository to get history for." },
                    "maxCommits": { "type": "integer", "description": "Maximum number of commits to retrieve (default: 100, max: 1000)." },
                    "branchName": { "type": "string", "description": "Optional branch name (defaults to current branch)." },
                    "since": { "type": "string", "description": "Optional start date filter (RFC3339 format, e.g., '2024-01-01T00:00:00Z')." },
                    "until": { "type": "string", "description": "Optional end date filter (RFC3339 format)." },
                    "author": { "type": "string", "description": "Optional author name/email filter (partial match)." },
                    "path": { "type": "string", "description": "Optional path filter (show commits affecting specific paths)." }
                },
                "required": ["repositoryName"]
            }),
            annotations: Some(ToolAnnotations {
                title: Some("Get Git History".to_string()),
                read_only_hint: Some(true),
                destructive_hint: Some(false),
                idempotent_hint: Some(true),
                open_world_hint: Some(false),
            }),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_switch_branch_tool_is_exposed() {
        let tools = get_tool_definitions();
        
        // Find the switch branch tool
        let switch_branch_tool = tools.iter()
            .find(|tool| tool.name == "repository_switch_branch")
            .expect("repository_switch_branch tool should be defined");
        
        // Verify the tool has proper description
        assert!(switch_branch_tool.description.is_some());
        assert!(switch_branch_tool.description.as_ref().unwrap().contains("branch"));
        
        // Verify the tool has proper input schema
        let schema = &switch_branch_tool.input_schema;
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["repositoryName"].is_object());
        assert!(schema["properties"]["branchName"].is_object());
        assert!(schema["properties"]["targetRef"].is_object());
        
        // Verify required fields
        assert!(schema["required"].as_array().unwrap().contains(&serde_json::Value::String("repositoryName".to_string())));
        
        // Verify annotations
        assert!(switch_branch_tool.annotations.is_some());
        let annotations = switch_branch_tool.annotations.as_ref().unwrap();
        assert_eq!(annotations.read_only_hint, Some(false));
        assert_eq!(annotations.destructive_hint, Some(false));
        assert_eq!(annotations.idempotent_hint, Some(true));
    }

    #[test]
    fn test_debug_branch_tool_json_size() {
        let tools = get_tool_definitions();
        
        // Find the problematic tools
        let switch_branch_tool = tools.iter()
            .find(|tool| tool.name == "repository_switch_branch")
            .expect("repository_switch_branch tool should be defined");
            
        let list_branches_tool = tools.iter()
            .find(|tool| tool.name == "repository_list_branches")
            .expect("repository_list_branches tool should be defined");
        
        // Serialize to JSON to see actual size and content
        let switch_json = serde_json::to_string_pretty(switch_branch_tool).unwrap();
        let list_json = serde_json::to_string_pretty(list_branches_tool).unwrap();
        
        println!("=== SWITCH BRANCH TOOL JSON ({} chars) ===", switch_json.len());
        println!("{}", switch_json);
        println!("\n=== LIST BRANCHES TOOL JSON ({} chars) ===", list_json.len());
        println!("{}", list_json);
        
        // Check for specific problematic patterns
        if switch_json.contains("oneOf") {
            println!("\n⚠️  SWITCH BRANCH TOOL CONTAINS 'oneOf' SCHEMA CONSTRAINT");
        }
        
        if switch_json.len() > 2000 {
            println!("\n⚠️  SWITCH BRANCH TOOL JSON IS VERY LARGE: {} characters", switch_json.len());
        }
        
        // Check description lengths
        let switch_desc = switch_branch_tool.description.as_ref().unwrap();
        let list_desc = list_branches_tool.description.as_ref().unwrap();
        
        println!("\n=== DESCRIPTION LENGTHS ===");
        println!("Switch branch: {} characters", switch_desc.len());
        println!("List branches: {} characters", list_desc.len());
        
        if switch_desc.len() > 200 {
            println!("⚠️  SWITCH BRANCH DESCRIPTION IS VERY LONG");
        }
    }

    #[test]
    fn test_all_expected_tools_are_exposed() {
        let tools = get_tool_definitions();
        let tool_names: Vec<&String> = tools.iter().map(|t| &t.name).collect();
        
        // Verify all expected tools are present
        let expected_tools = vec![
            "ping",
            "repository_add",
            "repository_list",
            "repository_remove",
            "repository_sync",
            "query",
            "repository_search_file",
            "repository_view_file",
            // "repository_map", // DISABLED - consumes too many tokens
            "repository_switch_branch",
            "repository_list_branches",
            "repository_git_history",
            "todo_read",
            "todo_write",
            "edit_file",
            "multi_edit_file",
            "shell_execute",
            "read_file",
            "write_file",
        ];
        
        for expected_tool in expected_tools {
            assert!(
                tool_names.contains(&&expected_tool.to_string()),
                "Tool '{}' should be exposed in tools list",
                expected_tool
            );
        }
    }
}
