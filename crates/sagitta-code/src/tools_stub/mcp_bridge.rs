// Bridge between MCP tool definitions and internal ToolDefinition format

use crate::llm::client::ToolDefinition;
use serde_json::json;

/// Get MCP tool definitions and convert them to internal format
pub fn get_mcp_tool_definitions() -> Vec<ToolDefinition> {
    // These definitions mirror what sagitta-mcp provides
    vec![
        ToolDefinition {
            name: "ping".to_string(),
            description: "Checks if the server is responsive.".to_string(),
            parameters: json!({ "type": "object", "properties": {} }),
            is_required: false,
        },
        ToolDefinition {
            name: "repository_add".to_string(),
            description: "Clones and adds a new Git repository for indexing.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Unique name for the repository" },
                    "url": { "type": "string", "description": "Remote Git URL (e.g., HTTPS or SSH)" },
                    "local_path": { "type": "string", "description": "Optional path to a pre-existing local repository" },
                    "branch": { "type": "string", "description": "Optional specific branch to checkout" },
                    "ssh_key": { "type": "string", "description": "Optional path to SSH private key for cloning" },
                    "ssh_passphrase": { "type": "string", "description": "Optional passphrase for the SSH key" }
                },
                "required": ["name"]
            }),
            is_required: false,
        },
        ToolDefinition {
            name: "repository_list".to_string(),
            description: "Lists currently configured repositories.".to_string(),
            parameters: json!({ "type": "object", "properties": {} }),
            is_required: false,
        },
        ToolDefinition {
            name: "repository_sync".to_string(),
            description: "Fetches latest changes, updates local copy, and re-indexes a repository.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Name of the repository to sync" }
                },
                "required": ["name"]
            }),
            is_required: false,
        },
        ToolDefinition {
            name: "semantic_code_search".to_string(),
            description: "Performs semantic search on an indexed repository using hybrid (dense + sparse vector) technology.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "repositoryName": { "type": "string", "description": "Name of the repository to query" },
                    "queryText": { "type": "string", "description": "The natural language query text" },
                    "limit": { "type": "integer", "description": "Maximum number of results to return" },
                    "elementType": { 
                        "type": "string", 
                        "description": "Filter by code element type (e.g., function, struct, class, method, enum, interface)"
                    },
                    "lang": { 
                        "type": "string", 
                        "description": "Filter by programming language (e.g., rust, python, javascript, go, java)"
                    },
                    "branchName": { "type": "string", "description": "Optional branch to query (defaults to active)" }
                },
                "required": ["repositoryName", "queryText", "limit"]
            }),
            is_required: false,
        },
        ToolDefinition {
            name: "search_file".to_string(),
            description: "Searches for files within a repository using a glob pattern.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "repositoryName": { "type": "string", "description": "Name of the repository to search within." },
                    "pattern": { "type": "string", "description": "Glob pattern to search for (e.g., \"*.rs\")." },
                    "caseSensitive": { "type": "boolean", "description": "Perform case-sensitive matching (default: false)." }
                },
                "required": ["repositoryName", "pattern"]
            }),
            is_required: false,
        },
        ToolDefinition {
            name: "repository_switch_branch".to_string(),
            description: "Switches to a different branch or Git reference in a repository with automatic resync.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "repositoryName": { "type": "string", "description": "Name of the repository to switch branches in." },
                    "branchName": { "type": "string", "description": "Target branch name to switch to." },
                    "targetRef": { "type": "string", "description": "Optional specific Git ref (tag, commit hash) to check out." },
                    "force": { "type": "boolean", "description": "Force switch even with uncommitted changes (default: false)." },
                    "noAutoResync": { "type": "boolean", "description": "Disable automatic resync after branch switch (default: false)." }
                },
                "required": ["repositoryName"]
            }),
            is_required: false,
        },
        ToolDefinition {
            name: "repository_list_branches".to_string(),
            description: "Lists branches and references in a repository with optional filtering.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "repositoryName": { "type": "string", "description": "Name of the repository to list branches for." },
                    "filter": { "type": "string", "description": "Optional: Filter pattern for branch/ref names." },
                    "includeRemote": { "type": "boolean", "description": "Include remote branches (default: true)." },
                    "includeTags": { "type": "boolean", "description": "Include tags (default: true)." },
                    "limit": { "type": "integer", "description": "Maximum number of results to return (default: 50, max: 200)." }
                },
                "required": ["repositoryName"]
            }),
            is_required: false,
        },
        ToolDefinition {
            name: "todo_read".to_string(),
            description: "Reads the current todo list with detailed status information.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
            is_required: false,
        },
        ToolDefinition {
            name: "todo_write".to_string(),
            description: "Updates the todo list with structured todo items.".to_string(),
            parameters: json!({
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
                                "status": { "type": "string", "enum": ["pending", "in_progress", "completed"], "description": "Current status of the todo" },
                                "priority": { "type": "string", "enum": ["low", "medium", "high"], "description": "Priority level" },
                                "created_at": { "type": "string", "description": "Optional RFC3339 timestamp when created" },
                                "updated_at": { "type": "string", "description": "Optional RFC3339 timestamp when last updated" }
                            },
                            "required": ["id", "content", "status", "priority"]
                        }
                    }
                },
                "required": ["todos"]
            }),
            is_required: false,
        },
        ToolDefinition {
            name: "edit_file".to_string(),
            description: "Performs exact string replacements in files with diff output.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "The absolute path to the file to edit" },
                    "old_string": { "type": "string", "description": "The text to search for and replace" },
                    "new_string": { "type": "string", "description": "The text to replace it with" },
                    "replace_all": { "type": "boolean", "description": "Replace all occurrences (default: false)" }
                },
                "required": ["file_path", "old_string", "new_string"]
            }),
            is_required: false,
        },
        ToolDefinition {
            name: "multi_edit_file".to_string(),
            description: "Performs multiple sequential edits to a single file with diff output.".to_string(),
            parameters: json!({
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
            is_required: false,
        },
        ToolDefinition {
            name: "shell_execute".to_string(),
            description: "Executes shell commands with cross-platform support (Windows/Linux/macOS).".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "The command to execute" },
                    "working_directory": { "type": "string", "description": "Optional working directory (defaults to current directory)" },
                    "env": { "type": "object", "description": "Optional environment variables", "additionalProperties": { "type": "string" } },
                    "timeout_ms": { "type": "integer", "description": "Optional timeout in milliseconds (default: 30000ms)" }
                },
                "required": ["command"]
            }),
            is_required: false,
        },
        ToolDefinition {
            name: "read_file".to_string(),
            description: "Reads content from a file with optional line range support.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "The absolute path to the file to read" },
                    "offset": { "type": "integer", "description": "Optional line number to start reading from" },
                    "limit": { "type": "integer", "description": "Optional number of lines to read" }
                },
                "required": ["file_path"]
            }),
            is_required: false,
        },
        ToolDefinition {
            name: "write_file".to_string(),
            description: "Writes content to a file with optional parent directory creation.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "The absolute path to the file to write" },
                    "content": { "type": "string", "description": "The content to write to the file" },
                    "create_parents": { "type": "boolean", "description": "Create parent directories if they don't exist (default: true)" }
                },
                "required": ["file_path", "content"]
            }),
            is_required: false,
        },
    ]
}