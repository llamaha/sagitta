/// Model Context Protocol (MCP) server implementation for sagitta-code
/// 
/// This is a simplified MCP server that operates only in stdio mode.
/// Multi-tenancy, OAuth, CORS, and TLS features have been removed.

pub mod types;
pub mod server;

pub use server::McpServer;
pub use types::*;