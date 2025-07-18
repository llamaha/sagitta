use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::collections::HashMap;

/// Top-level type for MCP requests
#[derive(Serialize, Deserialize, Debug)]
pub struct MCPRequest<T> {
    pub jsonrpc: String,
    pub method: String,
    pub params: T,
    pub id: u64,
}

impl<T> MCPRequest<T> {
    pub fn new(method: &str, params: T) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: 1, // Default ID
        }
    }
}

/// Top-level type for MCP responses
#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum MCPResponse<T> {
    Result { result: T },
    Error { error: ErrorObject },
}

/// Represents a JSON-RPC error object
#[derive(Serialize, Deserialize, Debug, Clone)] // Clone needed for error object
pub struct ErrorObject {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Response for ping method
#[derive(Serialize, Deserialize, Debug)]
pub struct PingResponse {
    pub status: String,
}

/// Configuration for repository add request
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct RepoConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub languages: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_key_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_passphrase: Option<String>,
}

/// Parameters for adding a repository
#[derive(Serialize, Deserialize, Debug)]
pub struct AddRepositoryRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub remote: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default)]
    pub config: RepoConfig,
}

/// Response for repository add
#[derive(Serialize, Deserialize, Debug)]
pub struct RepositoryAddResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Repository information
#[derive(Serialize, Deserialize, Debug)]
pub struct RepositoryInfo {
    pub name: String,
    pub remote: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,
    /// List of dependencies for this repository
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub dependencies: Vec<DependencyInfo>,
}

/// Response for repository list
#[derive(Serialize, Deserialize, Debug)]
pub struct RepositoryListResponse {
    pub repositories: Vec<RepositoryInfo>,
}

/// Response for repository sync
#[derive(Serialize, Deserialize, Debug)]
pub struct RepositorySyncResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_id: Option<String>,
    #[serde(default)]
    pub files_added: i32,
    #[serde(default)]
    pub files_updated: i32,
    #[serde(default)]
    pub files_removed: i32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Request {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<Value>, // Use Option<serde_json::Value> for optional params
    pub id: Option<Value>, // ID can be number, string or null
}

#[derive(Serialize, Debug)]
pub struct Response {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorObject>,
    pub id: Option<Value>,
}

impl Response {
    pub fn success(result: Value, id: Option<Value>) -> Self {
        // Ensure response ID is never null, default to 0 if needed
        let response_id = match id {
            Some(Value::Null) | None => Some(Value::Number(0.into())),
            Some(other) => Some(other),
        };
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id: response_id,
        }
    }

    pub fn error(error: ErrorObject, id: Option<Value>) -> Self {
        // Ensure response ID is never null, default to 0 if needed
        let response_id = match id {
            Some(Value::Null) | None => Some(Value::Number(0.into())),
            Some(other) => Some(other),
        };
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(error),
            id: response_id,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct ClientInfo {
    // Define fields based on MCP spec or observed client usage
    // Placeholder fields:
    pub name: Option<String>,
    pub version: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
#[derive(Default)]
pub struct InitializeParams {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    #[serde(rename = "clientInfo")]
    pub client_info: Option<ClientInfo>,
    // Add other fields as needed based on the MCP spec or observed client requests
    #[serde(default)] // Ensure missing capabilities field doesn't cause deserialization error
    pub capabilities: Option<Value> // Accept arbitrary capabilities for now
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: String,
    pub server_info: ServerInfo,
    pub capabilities: ServerCapabilities,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    pub tools: HashMap<String, ToolDefinition>, 
    // Add other capabilities like prompts, resources if supported later
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolAnnotations {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only_hint: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destructive_hint: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotent_hint: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_world_hint: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: serde_json::Value, // Using Value for flexibility, can refine with schemars later
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<ToolAnnotations>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct PingParams {}

#[derive(Serialize, Deserialize, Debug)]
pub struct PingResult {
    pub message: String,
}

/// Parameters for the `repository/add` method.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct RepositoryAddParams {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_passphrase: Option<String>,
    /// Optional specific Git ref (tag, commit hash, branch name) to check out initially.
    /// If provided, this ref will be checked out instead of the default branch after cloning.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_ref: Option<String>,
}

/// Result of the `repository/add` method.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryAddResult {
    pub name: String,
    pub url: String,
    pub local_path: PathBuf,
    pub default_branch: String,
    /// The branch/ref the repository is currently checked out to. 
    /// Will be the `target_ref` if one was provided during add, otherwise the default branch.
    pub active_branch: Option<String>, // Can be None initially
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct RepositoryListParams {}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryListResult {
    pub repositories: Vec<RepositoryInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySyncParams {
    /// The name of the repository to sync.
    pub name: String,
    /// Force re-indexing even if commit hasn't changed.
    pub force: Option<bool>,
    /// Optional list of file extensions to filter indexing by.
    pub extensions: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySyncResult {
    // Result might just be a message string or more structured info
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryRemoveParams {
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryRemoveResult {
    pub name: String,
    pub status: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct QueryParams {
    #[serde(rename = "repositoryName")]
    pub repository_name: String,
    #[serde(rename = "queryText")]
    pub query_text: String,
    #[serde(default = "default_limit")]
    pub limit: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "branchName")]
    pub branch_name: Option<String>,
    /// RECOMMENDED: Filter by code element type for more precise results (e.g., "function", "struct", "class", "method", "enum", "interface", "trait", "type")
    /// Using this parameter significantly improves search precision when looking for specific code constructs.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "elementType")]
    pub element_type: Option<String>,
    /// RECOMMENDED: Filter by programming language for better precision (e.g., "rust", "python", "javascript", "go", "java")
    /// This is especially valuable in multi-language repositories to reduce cross-language noise.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "lang")] // Match CLI argument
    pub lang: Option<String>,
    /// Optional: Include code content in results (default: false). Use only for smaller repositories due to output size limits.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "showCode")]
    pub show_code: Option<bool>,
}

fn default_limit() -> u64 {
    10 // Default limit for query results
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub results: Vec<SearchResultItem>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultItem {
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub score: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    pub element_type: String,
    pub language: String,
    /// Rich code context information for better understanding
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_info: Option<CodeContextInfo>,
}

/// Rich code context information for search results
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CodeContextInfo {
    /// Function or method signature if applicable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// Parent class or module name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_name: Option<String>,
    /// Brief description extracted from comments
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Key identifiers in this code chunk
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub identifiers: Vec<String>,
    /// Functions/methods that this code calls (outgoing calls)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub outgoing_calls: Vec<String>,
    /// Functions/methods that call this code (incoming calls)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub incoming_calls: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct ListToolsParams {
    // No parameters for tools/list currently
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ListToolsResult {
    pub tools: Vec<ToolDefinition>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct InitializedNotificationParams {
    // No parameters expected for initialized notification
}

// --- Tool Call Structs ---

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct CallToolParams {
    pub name: String, // Name of the tool to call
    pub arguments: Value, // Tool arguments as a JSON value
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ContentBlock {
    // Currently only supporting text content
    #[serde(rename = "type")]
    pub block_type: String, // Should be "text" for now
    pub text: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CallToolResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    pub content: Vec<ContentBlock>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySearchFileParams {
    /// Glob pattern to search for files (e.g., "*.rs", "src/**/*.toml").
    pub pattern: String,
    /// Optional: Specify the repository name to search in (overrides active repo concept if used).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository_name: Option<String>,
    /// Optional: Perform case-sensitive matching (defaults to false).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub case_sensitive: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySearchFileResult {
    /// List of relative file paths matching the pattern.
    pub matching_files: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryViewFileParams {
    /// Relative path to the file within the repository.
    pub file_path: String, // Using String for JSON compatibility
    /// Optional: Specify the repository name (overrides active repo concept if used).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository_name: Option<String>,
    /// Optional: Start line number (1-based, inclusive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_line: Option<usize>,
    /// Optional: End line number (1-based, inclusive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryViewFileResult {
    /// The content of the requested file range.
    pub content: String,
    /// The repository the file belongs to.
    pub repository_name: String,
    /// The relative path provided in the request.
    pub relative_path: String,
    /// The resolved absolute path on the server.
    pub absolute_path: String, 
    /// The start line used (if any).
    pub start_line: Option<usize>,
    /// The end line used (if any).
    pub end_line: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryMapParams {
    /// Name of the repository to map.
    pub repository_name: String,
    /// Verbosity level (0=minimal, 1=normal, 2=detailed).
    #[serde(default = "default_verbosity")]
    pub verbosity: u8,
    /// Optional: Specific paths to scan within the repository.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paths: Option<Vec<String>>,
    /// Optional: Filter by file extension (e.g., "rs", "py", "js").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_extension: Option<String>,
}

fn default_verbosity() -> u8 {
    1 // Default verbosity level
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryMapResult {
    /// The generated repository map as formatted text.
    pub map_content: String,
    /// Summary statistics about the mapping.
    pub summary: RepositoryMapSummary,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryMapSummary {
    /// Total number of files scanned.
    pub files_scanned: usize,
    /// Total number of code elements found.
    pub elements_found: usize,
    /// Breakdown by file type.
    pub file_types: std::collections::HashMap<String, usize>,
    /// Breakdown by element type.
    pub element_types: std::collections::HashMap<String, usize>,
}

// Branch operation types for git-manager integration

/// Parameters for branch switching
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySwitchBranchParams {
    /// Name of the repository
    pub repository_name: String,
    /// Target branch name (optional if target_ref is provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch_name: Option<String>,
    /// Optional specific Git ref (tag, commit hash, branch name) to check out.
    /// If provided, this ref will be checked out instead of the branch name.
    /// This supports any valid git reference including tags, commits, and remote branches.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_ref: Option<String>,
    /// Force switch even with uncommitted changes
    #[serde(default)]
    pub force: bool,
    /// Disable automatic resync
    #[serde(default)]
    pub no_auto_resync: bool,
}

/// Result of branch switching
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySwitchBranchResult {
    /// Previous branch name
    pub previous_branch: String,
    /// New branch name
    pub new_branch: String,
    /// Whether sync was performed
    pub sync_performed: bool,
    /// Number of files changed
    pub files_changed: usize,
    /// Sync details if performed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_details: Option<SyncDetails>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SyncDetails {
    pub files_added: usize,
    pub files_updated: usize,
    pub files_removed: usize,
}

/// Parameters for listing repository branches
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryListBranchesParams {
    /// Name of the repository
    pub repository_name: String,
    /// Optional filter pattern for branch/ref names (supports glob patterns)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    /// Include remote branches (default: true)
    #[serde(default = "default_include_remote")]
    pub include_remote: bool,
    /// Include tags (default: true)
    #[serde(default = "default_include_tags")]
    pub include_tags: bool,
    /// Maximum number of results to return (default: 50, max: 200)
    #[serde(default = "default_branch_limit")]
    pub limit: usize,
}

fn default_include_remote() -> bool {
    true
}

fn default_include_tags() -> bool {
    true
}

fn default_branch_limit() -> usize {
    50
}

/// Result of listing branches
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryListBranchesResult {
    /// List of branch names
    pub branches: Vec<String>,
    /// Current active branch
    pub current_branch: String,
}

// Todo management types

/// A single todo item
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TodoItem {
    /// Unique identifier for the todo
    pub id: String,
    /// The content/description of the todo
    pub content: String,
    /// Current status of the todo
    pub status: TodoStatus,
    /// Priority level
    pub priority: TodoPriority,
    /// Optional timestamp when created
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Optional timestamp when last updated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// Todo status enum
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

/// Todo priority enum
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TodoPriority {
    Low,
    Medium,
    High,
}

/// Parameters for todo_read - empty since it takes no parameters
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct TodoReadParams {}

/// Result of todo_read
#[derive(Serialize, Deserialize, Debug)]
pub struct TodoReadResult {
    /// List of all todos
    pub todos: Vec<TodoItem>,
    /// Summary of todo statuses
    pub summary: String,
}

/// Parameters for todo_write
#[derive(Serialize, Deserialize, Debug)]
pub struct TodoWriteParams {
    /// The complete list of todos to write (replaces existing todos)
    pub todos: Vec<TodoItem>,
}

/// Result of todo_write
#[derive(Serialize, Deserialize, Debug)]
pub struct TodoWriteResult {
    /// The updated list of todos
    pub todos: Vec<TodoItem>,
    /// Summary of the update
    pub summary: String,
}

// File editing types

/// Parameters for edit_file
#[derive(Serialize, Deserialize, Debug)]
pub struct EditFileParams {
    /// The absolute path to the file to edit
    pub file_path: String,
    /// The text to search for and replace
    pub old_string: String,
    /// The text to replace it with
    pub new_string: String,
    /// Replace all occurrences (default: false)
    #[serde(default)]
    pub replace_all: bool,
}

/// Result of edit_file
#[derive(Serialize, Deserialize, Debug)]
pub struct EditFileResult {
    /// The file path that was edited
    pub file_path: String,
    /// The original content (limited to relevant context)
    pub old_content: String,
    /// The new content (limited to relevant context)
    pub new_content: String,
    /// Unified diff showing the changes
    pub diff: String,
    /// Summary of changes made
    pub changes_summary: String,
}

/// A single edit operation for multi_edit_file
#[derive(Serialize, Deserialize, Debug)]
pub struct EditOperation {
    /// The text to search for and replace
    pub old_string: String,
    /// The text to replace it with
    pub new_string: String,
    /// Replace all occurrences (default: false)
    #[serde(default)]
    pub replace_all: bool,
}

/// Parameters for multi_edit_file
#[derive(Serialize, Deserialize, Debug)]
pub struct MultiEditFileParams {
    /// The absolute path to the file to edit
    pub file_path: String,
    /// Array of edit operations to perform sequentially
    pub edits: Vec<EditOperation>,
}

/// Result of multi_edit_file
#[derive(Serialize, Deserialize, Debug)]
pub struct MultiEditFileResult {
    /// The file path that was edited
    pub file_path: String,
    /// The original content before any edits
    pub original_content: String,
    /// The final content after all edits
    pub final_content: String,
    /// Unified diff showing all changes
    pub diff: String,
    /// Number of edits applied successfully
    pub edits_applied: usize,
    /// Summary of all changes made
    pub changes_summary: String,
}

// Shell execution types

/// Parameters for shell_execute
#[derive(Serialize, Deserialize, Debug)]
pub struct ShellExecuteParams {
    /// The command to execute
    pub command: String,
    /// Optional working directory (defaults to current directory)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
    /// Optional timeout in milliseconds (default: 30000ms)
    #[serde(default = "default_shell_timeout")]
    pub timeout_ms: u64,
    /// Optional environment variables
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
}

fn default_shell_timeout() -> u64 {
    30000 // 30 seconds default
}

/// Result of shell_execute
#[derive(Serialize, Deserialize, Debug)]
pub struct ShellExecuteResult {
    /// The command that was executed
    pub command: String,
    /// Exit code of the process
    pub exit_code: i32,
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Whether the command timed out
    pub timed_out: bool,
}

// File operation types

/// Parameters for read_file
#[derive(Serialize, Deserialize, Debug)]
pub struct ReadFileParams {
    /// The absolute path to the file to read
    pub file_path: String,
    /// Optional start line (1-based, inclusive)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_line: Option<usize>,
    /// Optional end line (1-based, inclusive)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<usize>,
}

/// Result of read_file
#[derive(Serialize, Deserialize, Debug)]
pub struct ReadFileResult {
    /// The file path that was read
    pub file_path: String,
    /// The content of the file (or requested portion)
    pub content: String,
    /// Total number of lines in the file
    pub line_count: usize,
    /// File size in bytes
    pub file_size: u64,
    /// Start line that was read (if specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_line: Option<usize>,
    /// End line that was read (if specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<usize>,
}

/// Parameters for write_file
#[derive(Serialize, Deserialize, Debug)]
pub struct WriteFileParams {
    /// The absolute path to the file to write
    pub file_path: String,
    /// The content to write to the file
    pub content: String,
    /// Create parent directories if they don't exist (default: true)
    #[serde(default = "default_create_parents")]
    pub create_parents: bool,
}

fn default_create_parents() -> bool {
    true
}

/// Result of write_file
#[derive(Serialize, Deserialize, Debug)]
pub struct WriteFileResult {
    /// The file path that was written
    pub file_path: String,
    /// The content that was written (truncated if too large)
    pub content: String,
    /// Number of bytes written
    pub bytes_written: u64,
    /// Whether this was a new file creation
    pub created: bool,
}

// Git history types

/// Parameters for repository_git_history
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryGitHistoryParams {
    /// Name of the repository to get history for
    pub repository_name: String,
    /// Maximum number of commits to retrieve (default: 100, max: 1000)
    #[serde(default = "default_max_commits")]
    pub max_commits: u64,
    /// Optional branch name (defaults to current branch)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch_name: Option<String>,
    /// Optional start date filter (RFC3339 format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    /// Optional end date filter (RFC3339 format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<String>,
    /// Optional author name/email filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Optional path filter (show commits affecting specific paths)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl Default for RepositoryGitHistoryParams {
    fn default() -> Self {
        Self {
            repository_name: String::new(),
            max_commits: default_max_commits(),
            branch_name: None,
            since: None,
            until: None,
            author: None,
            path: None,
        }
    }
}

fn default_max_commits() -> u64 {
    100
}

/// A single commit in the git history
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GitCommit {
    /// Full commit hash
    pub id: String,
    /// Short commit hash (first 7 characters)
    pub short_id: String,
    /// Commit message
    pub message: String,
    /// Author name
    pub author: String,
    /// Author email
    pub email: String,
    /// Commit timestamp (RFC3339 format)
    pub timestamp: String,
    /// Parent commit hashes
    pub parents: Vec<String>,
    /// Branch/tag references pointing to this commit
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub refs: Vec<String>,
}

/// Result of repository_git_history
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryGitHistoryResult {
    /// List of commits in reverse chronological order
    pub commits: Vec<GitCommit>,
    /// Current branch name
    pub current_branch: String,
    /// Total number of commits returned
    pub total_commits: u64,
    /// Whether the history was truncated due to max_commits
    pub truncated: bool,
}

// Dependency management types

/// Parameters for adding/removing repository dependencies
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryDependencyParams {
    /// The main repository to add/remove dependency from
    pub repository_name: String,
    /// The dependency repository name
    pub dependency_name: String,
    /// Optional specific ref (branch/tag/commit) for the dependency
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_ref: Option<String>,
    /// Optional description of why this dependency is needed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
}

/// Result of dependency operations
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryDependencyResult {
    /// Whether the operation succeeded
    pub success: bool,
    /// Status message
    pub message: String,
}

/// Parameters for listing repository dependencies
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryListDependenciesParams {
    /// The repository to list dependencies for
    pub repository_name: String,
}

/// Information about a single dependency
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DependencyInfo {
    /// Name of the dependency repository
    pub repository_name: String,
    /// Target ref specified for this dependency
    pub target_ref: Option<String>,
    /// Purpose/description of the dependency
    pub purpose: Option<String>,
    /// Whether the dependency repository is available in the system
    pub is_available: bool,
    /// Local path of the dependency repository (if available)
    pub local_path: Option<String>,
    /// Current ref of the dependency repository (if available)
    pub current_ref: Option<String>,
}

/// Result of listing dependencies
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryListDependenciesResult {
    /// The repository name
    pub repository_name: String,
    /// List of dependencies
    pub dependencies: Vec<DependencyInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, from_value, to_value};

    #[test]
    fn test_mcp_request_new() {
        let params = PingParams {};
        let request: MCPRequest<PingParams> = MCPRequest::new("ping", params);
        
        assert_eq!(request.jsonrpc, "2.0");
        assert_eq!(request.method, "ping");
        assert_eq!(request.id, 1);
    }

    #[test]
    fn test_mcp_response_success() {
        let value = json!({"status": "ok"});
        let response = Response::success(value.clone(), Some(json!(123)));
        
        assert_eq!(response.jsonrpc, "2.0");
        assert_eq!(response.result, Some(value));
        assert!(response.error.is_none());
        assert_eq!(response.id, Some(json!(123)));
    }

    #[test]
    fn test_mcp_response_error() {
        let error = ErrorObject {
            code: -32600,
            message: "Invalid Request".to_string(),
            data: None,
        };
        let response = Response::error(error.clone(), Some(json!(456)));
        
        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert_eq!(response.error.as_ref().unwrap().code, -32600);
        assert_eq!(response.error.as_ref().unwrap().message, "Invalid Request");
        assert_eq!(response.id, Some(json!(456)));
    }

    #[test]
    fn test_response_null_id_handling() {
        // Test that null ID gets converted to 0
        let response = Response::success(json!({"test": "value"}), Some(Value::Null));
        assert_eq!(response.id, Some(json!(0)));

        let response = Response::success(json!({"test": "value"}), None);
        assert_eq!(response.id, Some(json!(0)));
    }

    #[test]
    fn test_default_functions() {
        assert_eq!(default_limit(), 10);
        assert_eq!(default_verbosity(), 1);
        assert_eq!(default_shell_timeout(), 30000);
        assert_eq!(default_create_parents(), true);
        assert_eq!(default_include_remote(), true);
        assert_eq!(default_include_tags(), true);
        assert_eq!(default_branch_limit(), 50);
        assert_eq!(default_max_commits(), 100);
    }

    #[test]
    fn test_todo_status_serialization() {
        assert_eq!(serde_json::to_string(&TodoStatus::Pending).unwrap(), "\"pending\"");
        assert_eq!(serde_json::to_string(&TodoStatus::InProgress).unwrap(), "\"in_progress\"");
        assert_eq!(serde_json::to_string(&TodoStatus::Completed).unwrap(), "\"completed\"");
        
        let status: TodoStatus = serde_json::from_str("\"pending\"").unwrap();
        assert_eq!(status, TodoStatus::Pending);
    }

    #[test]
    fn test_todo_priority_serialization() {
        assert_eq!(serde_json::to_string(&TodoPriority::Low).unwrap(), "\"low\"");
        assert_eq!(serde_json::to_string(&TodoPriority::Medium).unwrap(), "\"medium\"");
        assert_eq!(serde_json::to_string(&TodoPriority::High).unwrap(), "\"high\"");
        
        let priority: TodoPriority = serde_json::from_str("\"high\"").unwrap();
        assert_eq!(priority, TodoPriority::High);
    }

    #[test]
    fn test_initialize_params_deserialization() {
        let json = json!({
            "protocolVersion": "1.0",
            "clientInfo": {
                "name": "test-client",
                "version": "0.1.0"
            }
        });
        
        let params: InitializeParams = from_value(json).unwrap();
        assert_eq!(params.protocol_version, "1.0");
        assert_eq!(params.client_info.as_ref().unwrap().name, Some("test-client".to_string()));
        assert_eq!(params.client_info.as_ref().unwrap().version, Some("0.1.0".to_string()));
    }

    #[test]
    fn test_initialize_params_missing_fields() {
        // Test that missing optional fields don't cause errors
        let json = json!({
            "protocolVersion": "1.0"
        });
        
        let params: InitializeParams = from_value(json).unwrap();
        assert_eq!(params.protocol_version, "1.0");
        assert!(params.client_info.is_none());
        assert!(params.capabilities.is_none());
    }

    #[test]
    fn test_query_params_defaults() {
        let json = json!({
            "repositoryName": "test-repo",
            "queryText": "search query"
        });
        
        let params: QueryParams = from_value(json).unwrap();
        assert_eq!(params.repository_name, "test-repo");
        assert_eq!(params.query_text, "search query");
        assert_eq!(params.limit, 10); // Should use default
        assert!(params.branch_name.is_none());
        assert!(params.element_type.is_none());
        assert!(params.lang.is_none());
        assert!(params.show_code.is_none());
    }

    #[test]
    fn test_content_block_serialization() {
        let block = ContentBlock {
            block_type: "text".to_string(),
            text: "Hello, world!".to_string(),
        };
        
        let json = to_value(&block).unwrap();
        assert_eq!(json["type"], "text");
        assert_eq!(json["text"], "Hello, world!");
    }

    #[test]
    fn test_git_commit_serialization() {
        let commit = GitCommit {
            id: "abcdef1234567890".to_string(),
            short_id: "abcdef1".to_string(),
            message: "Test commit".to_string(),
            author: "Test Author".to_string(),
            email: "test@example.com".to_string(),
            timestamp: "2024-01-01T12:00:00Z".to_string(),
            parents: vec!["parent1".to_string()],
            refs: vec![],
        };
        
        let json = to_value(&commit).unwrap();
        assert_eq!(json["id"], "abcdef1234567890");
        assert_eq!(json["shortId"], "abcdef1");
        assert_eq!(json["message"], "Test commit");
        assert!(!json.as_object().unwrap().contains_key("refs")); // Should be skipped when empty
    }

    #[test]
    fn test_repository_add_params_optional_fields() {
        let json = json!({
            "name": "test-repo"
        });
        
        let params: RepositoryAddParams = from_value(json).unwrap();
        assert_eq!(params.name, "test-repo");
        assert!(params.url.is_none());
        assert!(params.local_path.is_none());
        assert!(params.branch.is_none());
        assert!(params.ssh_key.is_none());
        assert!(params.ssh_passphrase.is_none());
        assert!(params.target_ref.is_none());
    }

    #[test]
    fn test_shell_execute_params_timeout_default() {
        let json = json!({
            "command": "echo hello"
        });
        
        let params: ShellExecuteParams = from_value(json).unwrap();
        assert_eq!(params.command, "echo hello");
        assert_eq!(params.timeout_ms, 30000);
        assert!(params.working_directory.is_none());
        assert!(params.env.is_none());
    }

    #[test]
    fn test_repository_map_params_defaults() {
        let json = json!({
            "repositoryName": "test-repo"
        });
        
        let params: RepositoryMapParams = from_value(json).unwrap();
        assert_eq!(params.repository_name, "test-repo");
        assert_eq!(params.verbosity, 1);
        assert!(params.paths.is_none());
        assert!(params.file_extension.is_none());
    }

    #[test]
    fn test_edit_operation_default_replace_all() {
        let json = json!({
            "old_string": "foo",
            "new_string": "bar"
        });
        
        let op: EditOperation = from_value(json).unwrap();
        assert_eq!(op.old_string, "foo");
        assert_eq!(op.new_string, "bar");
        assert!(!op.replace_all); // Should default to false
    }

    #[test]
    fn test_switch_branch_params_force_default() {
        let json = json!({
            "repositoryName": "test-repo",
            "branchName": "feature-branch"
        });
        
        let params: RepositorySwitchBranchParams = from_value(json).unwrap();
        assert_eq!(params.repository_name, "test-repo");
        assert_eq!(params.branch_name, Some("feature-branch".to_string()));
        assert!(!params.force); // Should default to false
        assert!(!params.no_auto_resync); // Should default to false
    }

    #[test]
    fn test_dependency_info_serialization() {
        let info = DependencyInfo {
            repository_name: "lib-repo".to_string(),
            target_ref: Some("v1.0.0".to_string()),
            purpose: Some("Utility functions".to_string()),
            is_available: true,
            local_path: Some("/path/to/lib".to_string()),
            current_ref: Some("v1.0.0".to_string()),
        };
        
        let json = to_value(&info).unwrap();
        assert_eq!(json["repositoryName"], "lib-repo");
        assert_eq!(json["targetRef"], "v1.0.0");
        assert_eq!(json["purpose"], "Utility functions");
        assert_eq!(json["isAvailable"], true);
    }

    #[test]
    fn test_tool_annotations_all_fields() {
        let annotations = ToolAnnotations {
            title: Some("Test Tool".to_string()),
            read_only_hint: Some(true),
            destructive_hint: Some(false),
            idempotent_hint: Some(true),
            open_world_hint: Some(false),
        };
        
        let json = to_value(&annotations).unwrap();
        assert_eq!(json["title"], "Test Tool");
        assert_eq!(json["readOnlyHint"], true);
        assert_eq!(json["destructiveHint"], false);
        assert_eq!(json["idempotentHint"], true);
        assert_eq!(json["openWorldHint"], false);
    }

    #[test]
    fn test_sync_details_serialization() {
        let details = SyncDetails {
            files_added: 10,
            files_updated: 5,
            files_removed: 2,
        };
        
        let json = to_value(&details).unwrap();
        assert_eq!(json["filesAdded"], 10);
        assert_eq!(json["filesUpdated"], 5);
        assert_eq!(json["filesRemoved"], 2);
    }

    #[test]
    fn test_error_object_with_data() {
        let error = ErrorObject {
            code: -32700,
            message: "Parse error".to_string(),
            data: Some(json!({"line": 42, "column": 10})),
        };
        
        let json = to_value(&error).unwrap();
        assert_eq!(json["code"], -32700);
        assert_eq!(json["message"], "Parse error");
        assert_eq!(json["data"]["line"], 42);
        assert_eq!(json["data"]["column"], 10);
    }

    #[test]
    fn test_repository_git_history_params_default() {
        let params = RepositoryGitHistoryParams::default();
        assert_eq!(params.repository_name, "");
        assert_eq!(params.max_commits, 100);
        assert!(params.branch_name.is_none());
        assert!(params.since.is_none());
        assert!(params.until.is_none());
        assert!(params.author.is_none());
        assert!(params.path.is_none());
    }
} 