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
    /// Optional Tenant ID for this repository operation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
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
    /// Optional: Filter by code element type (e.g., "function", "struct", "enum", etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "elementType")]
    pub element_type: Option<String>,
    /// Optional: Filter by specific programming language (e.g., "rust", "python", "go"). This significantly improves relevance for code-related queries.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "lang")] // Match CLI argument
    pub lang: Option<String>,
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
    pub content: String,
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