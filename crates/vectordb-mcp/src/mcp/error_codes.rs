// Standard JSON-RPC Error Codes
pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;

// Custom Application-Specific Errors (-32000 to -32050 for MCP)

pub const CORE_LOGIC_ERROR: i64 = -32000;         // Generic core error

// Repository Management Errors (Original MCP assignments where possible)
pub const REPO_ALREADY_EXISTS: i64 = -32010;
pub const REPO_NOT_FOUND: i64 = -32011;
pub const GIT_OPERATION_FAILED: i64 = -32012;
pub const QDRANT_OPERATION_FAILED: i64 = -32013; // Was -32008 in the partial file, but -32013 in old mcp.rs which server.rs expects
pub const CONFIG_SAVE_FAILED: i64 = -32014;
pub const CONFIG_LOAD_FAILED: i64 = -32015;     // New, but logical to have
pub const EMBEDDING_ERROR: i64 = -32016;          // Was -32009 in partial, -32016 in old mcp.rs
pub const URL_DETERMINATION_FAILED: i64 = -32017; // Was -32010 in partial, -32017 in old mcp.rs
pub const NAME_DERIVATION_FAILED: i64 = -32018;
pub const BRANCH_DETECTION_FAILED: i64 = -32019;

// Query Errors
pub const INVALID_QUERY_PARAMS: i64 = -32020;    // Was also TOOL_NOT_FOUND's old number
pub const QUERY_EXECUTION_FAILED: i64 = -32021;

// File System Errors
pub const FILE_NOT_FOUND: i64 = -32022;

// Other MCP Errors
pub const TIMEOUT_ERROR: i64 = -32025;            // Renumbered from -32011

// Tool-related Errors
pub const TOOL_NOT_FOUND: i64 = -32026;           // Renumbered from -32020

// Authentication and Authorization Errors
pub const ACCESS_DENIED: i64 = -32030;           // Kept number 