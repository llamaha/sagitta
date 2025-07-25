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
            description: "Lists currently configured repositories with their dependencies. Each repository includes a 'dependencies' field showing all linked dependency repositories with their pinned versions. Dependencies are version-locked to match your project's actual usage - query these dependency repositories directly for accurate implementation details instead of relying on potentially outdated web documentation. IMPORTANT: Use repository names from this list with tools that support 'repositoryName' parameter to work across repositories.".to_string(),
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
            description: "Performs semantic search on an indexed repository. The repositoryName parameter allows searching in ANY repository including dependencies! Use elementType (function, class, struct, method, interface) and lang (rust, python, javascript, go) parameters for better results. Examples: query='authentication', elementType='function', lang='rust' finds Rust auth functions. CRITICAL: Query dependency repositories (e.g., 'egui', 'tokio') directly instead of web searching - use repository_list to see available repos.".to_string(),
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
            description: "Searches for files within a repository using glob patterns. The repositoryName parameter allows searching in ANY repository including dependencies! Patterns like '*.rs' search recursively by default. Use 'src/*.rs' to limit to a specific directory. Pattern '*' is now allowed and will show first 50 files. IMPORTANT: Use this to explore dependency repositories directly.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "repositoryName": { "type": "string", "description": "Name of the repository to search within." },
                    "pattern": { "type": "string", "description": "Glob pattern to search for. Examples: '*.rs' (all Rust files recursively), 'src/*.rs' (only in src/), '**/*.md' (all markdown files recursively), '*README*' (files with README in name)" },
                    "caseSensitive": { "type": "boolean", "description": "Perform case-sensitive matching (default: false)." }
                },
                "required": ["repositoryName", "pattern"]
            }),
            is_required: false,
        },
        ToolDefinition {
            name: "ripgrep".to_string(),
            description: "Performs recursive regex search for content within repository files. The repository_name parameter allows searching in ANY repository including dependencies! Similar to ripgrep (rg) command. Searches file contents, not just filenames. CRITICAL: Use this to search code in dependencies instead of web searching.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Regular expression pattern to search for in file contents" },
                    "file_pattern": { "type": "string", "description": "Optional glob pattern to filter files (e.g., '*.rs' for Rust files)" },
                    "repository_name": { "type": "string", "description": "Optional repository name to search within. If not specified, searches the current repository" },
                    "case_sensitive": { "type": "boolean", "description": "Whether the search should be case-sensitive (default: false)" },
                    "context_lines": { "type": "integer", "description": "Number of context lines to show before and after matches (default: 2)" },
                    "max_results": { "type": "integer", "description": "Maximum number of matches to return (default: 100)" }
                },
                "required": ["pattern"]
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
            description: "Performs exact string replacements in files with diff output. The repository_name parameter allows editing files in ANY repository including dependencies!".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "The absolute path to the file to edit" },
                    "old_string": { "type": "string", "description": "The text to search for and replace" },
                    "new_string": { "type": "string", "description": "The text to replace it with" },
                    "replace_all": { "type": "boolean", "description": "Replace all occurrences (default: false)" },
                    "repository_name": { "type": "string", "description": "Optional repository name to edit file in. If not specified, uses the current/active repository" }
                },
                "required": ["file_path", "old_string", "new_string"]
            }),
            is_required: false,
        },
        ToolDefinition {
            name: "multi_edit_file".to_string(),
            description: "Performs multiple sequential edits to a single file with diff output. The repository_name parameter allows editing files in ANY repository including dependencies!".to_string(),
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
                    },
                    "repository_name": { "type": "string", "description": "Optional repository name to edit file in. If not specified, uses the current/active repository" }
                },
                "required": ["file_path", "edits"]
            }),
            is_required: false,
        },
        ToolDefinition {
            name: "shell_execute".to_string(),
            description: "Executes shell commands. Must include ONE of: tail_lines (see end of output), head_lines (see start of output), or grep_pattern (filter output). Examples: 'cargo build' with tail_lines=50, 'cargo test' with grep_pattern='error'. Working directory defaults to current repository.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "The command to execute. Must include one of: tail_lines, head_lines, or grep_pattern" },
                    "working_directory": { "type": "string", "description": "Working directory path. Only set if you need a different directory and have confirmed the path exists" },
                    "env": { "type": "object", "description": "Optional environment variables", "additionalProperties": { "type": "string" } },
                    "timeout_ms": { "type": "integer", "description": "Command timeout in milliseconds (default: 60000)" },
                    "grep_pattern": { "type": "string", "description": "Filter output to lines containing this pattern (use when searching for specific text)" },
                    "head_lines": { "type": "integer", "description": "Show first N lines of output (use when start of output is important)" },
                    "tail_lines": { "type": "integer", "description": "Show last N lines of output (recommended: 50 for most commands, 100 for verbose output)" }
                },
                "required": ["command"]
            }),
            is_required: false,
        },
        ToolDefinition {
            name: "read_file".to_string(),
            description: "Reads a specific range of lines from a file. You MUST specify both start_line and end_line (1-based line numbers). Maximum 400 lines per request. Example: to read first 100 lines use start_line=1, end_line=100. DO NOT use 'limit' or 'offset' parameters - they don't exist. The repository_name parameter allows reading files from ANY repository including dependencies!".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "Path to the file to read. Can be absolute or relative to the current repository" },
                    "start_line": { "type": "integer", "description": "REQUIRED: Line number to start reading from (1-based, inclusive). Example: 1 for first line" },
                    "end_line": { "type": "integer", "description": "REQUIRED: Line number to stop reading at (1-based, inclusive). Maximum range is 400 lines. Example: 100 to read up to line 100" },
                    "repository_name": { "type": "string", "description": "Optional repository name to read file from. If not specified, uses the current/active repository" }
                },
                "required": ["file_path", "start_line", "end_line"]
            }),
            is_required: false,
        },
        ToolDefinition {
            name: "write_file".to_string(),
            description: "Writes content to a file with optional parent directory creation. The repository_name parameter allows writing files in ANY repository including dependencies!".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "Path to the file to write. Can be absolute or relative to the current repository" },
                    "content": { "type": "string", "description": "The content to write to the file" },
                    "create_parents": { "type": "boolean", "description": "Create parent directories if they don't exist (default: true)" },
                    "repository_name": { "type": "string", "description": "Optional repository name to write file in. If not specified, uses the current/active repository" }
                },
                "required": ["file_path", "content"]
            }),
            is_required: false,
        },
        ToolDefinition {
            name: "current_working_directory".to_string(),
            description: "Returns the current working directory context. Shows the repository path if one is active, or the current system directory otherwise.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
            is_required: false,
        },
    ]
}