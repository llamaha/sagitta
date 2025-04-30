use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use crate::mcp::types::ErrorObject;

// Add submodule for types
pub mod types;

/// Represents a JSON-RPC request.
#[derive(Serialize, Deserialize, Debug)]
pub struct Request {
    pub jsonrpc: String,
    pub method: String,
    pub params: Value, // Using Value for flexibility initially
    pub id: Option<Value>, // Request ID (number or string)
}

/// Represents a JSON-RPC response.
#[derive(Serialize, Deserialize, Debug)]
pub struct Response {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorObject>,
    pub id: Option<Value>, // Should match the request ID
}

/// Represents a JSON-RPC error object.

// --- Specific Method Params/Results (Example: Ping) ---

/// Parameters for the "ping" method (empty).
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct PingParams {}

/// Result for the "ping" method.
#[derive(Serialize, Deserialize, Debug)]
pub struct PingResult {
    pub message: String,
}

/// Parameters for the "repositoryAdd" method.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryAddParams {
    pub url: Option<String>,
    pub local_path: Option<PathBuf>,
    pub name: Option<String>,
    pub branch: Option<String>,
    pub ssh_key: Option<PathBuf>,
    pub ssh_passphrase: Option<String>,
}

/// Result for the "repositoryAdd" method.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryAddResult {
    pub name: String,
    pub url: String,
    pub local_path: PathBuf,
    pub default_branch: String,
    pub active_branch: Option<String>, // Included from RepositoryConfig
}

// Repository List
#[derive(Serialize, Deserialize, Debug, Default)] // Default allows empty params {}
pub struct RepositoryListParams {
    // No parameters needed
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryInfo {
    pub name: String,
    pub url: String,
    pub local_path: PathBuf,
    pub active_branch: Option<String>,
    // Add other useful fields from RepositoryConfig if needed
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RepositoryListResult {
    pub repositories: Vec<RepositoryInfo>,
}

// --- Repository Sync ---

/// Parameters for the "repositorySync" method.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySyncParams {
    pub name: String, // The name of the repository to sync
    // Optional: Add branch name if you want to sync a specific branch other than the active one
    // pub branch: Option<String>,
    // Optional: Add sync options (e.g., force pull, prune)
    // pub force: Option<bool>,
    // pub prune: Option<bool>,
}

/// Result for the "repositorySync" method.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySyncResult {
    pub name: String,
    pub status: String, // e.g., "Synced", "Up-to-date", "Error: <message>"
    pub commit_hash: Option<String>, // The commit hash after syncing
    // Potentially add info about files changed/indexed
}

// --- Repository Remove ---

/// Parameters for the "repositoryRemove" method.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryRemoveParams {
    pub name: String, // The name of the repository to remove
    // Optional: Flag to control if local files should be deleted
    // pub delete_local_files: Option<bool>,
}

/// Result for the "repositoryRemove" method.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryRemoveResult {
    pub name: String,
    pub status: String, // e.g., "Removed", "Data Removed", "Error: <message>"
}

// --- Query ---

/// Parameters for the "query" method.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QueryParams {
    pub query_text: String,
    pub repository_name: String, // Specify which repository to query
    pub branch_name: Option<String>, // Optionally specify a branch (defaults to active)
    pub limit: Option<usize>, // Optional limit for number of results
}

/// Represents a single search result item.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultItem {
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub score: f32,
    pub content: String, // The actual content chunk
    // Optional: Add language, element_type etc. if needed from payload
}

/// Result for the "query" method.
#[derive(Serialize, Deserialize, Debug)]
pub struct QueryResult {
    pub results: Vec<SearchResultItem>,
}

impl Response {
    /// Helper to create a success response.
    pub fn success(result: Value, id: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    /// Helper to create an error response.
    pub fn error(error: ErrorObject, id: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(error),
            id,
        }
    }
}

// Standard JSON-RPC error codes
pub mod error_codes {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL_ERROR: i64 = -32603;
    pub const SERVER_ERROR_START: i64 = -32000;
    pub const SERVER_ERROR_END: i64 = -32099;

    // Custom application-specific errors
    pub const CORE_LOGIC_ERROR: i64 = -32000; // Generic core error
    // pub const REPOSITORY_ADD_FAILED: i64 = -32001; // Replaced by more specific codes below

    // Repository Management Errors
    pub const REPO_ALREADY_EXISTS: i64 = -32010;
    pub const REPO_NOT_FOUND: i64 = -32011;
    pub const GIT_OPERATION_FAILED: i64 = -32012;
    pub const QDRANT_OPERATION_FAILED: i64 = -32013;
    pub const CONFIG_SAVE_FAILED: i64 = -32014;
    pub const CONFIG_LOAD_FAILED: i64 = -32015;
    pub const EMBEDDING_ERROR: i64 = -32016;
    pub const URL_DETERMINATION_FAILED: i64 = -32017;
    pub const NAME_DERIVATION_FAILED: i64 = -32018;
    pub const BRANCH_DETECTION_FAILED: i64 = -32019;

    // Query Errors
    pub const INVALID_QUERY_PARAMS: i64 = -32020;
    pub const QUERY_EXECUTION_FAILED: i64 = -32021;
}