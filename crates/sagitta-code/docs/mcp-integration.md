# Model Context Protocol (MCP) Integration

Sagitta Code now has full MCP (Model Context Protocol) integration, allowing Claude CLI to access all tools from the ToolRegistry through a standardized protocol.

## Overview

The MCP integration works by:
1. Starting an internal MCP server as a subprocess when Claude needs it
2. Exposing all registered tools through the MCP protocol
3. Generating a temporary config file that Claude CLI can use with `--mcp-config`

## How It Works

### Internal Architecture

1. **Enhanced MCP Server** (`src/mcp/enhanced_server.rs`)
   - Exposes tools from ToolRegistry via JSON-RPC protocol
   - Handles standard MCP methods: initialize, tools/list, tools/call
   - Communicates over stdin/stdout with proper stderr logging

2. **MCP Integration** (`src/llm/claude_code/mcp_integration.rs`)
   - Manages the lifecycle of the internal MCP server
   - Creates temporary config files for Claude CLI
   - Ensures proper cleanup on shutdown

3. **Claude Code Client** (`src/llm/claude_code/client.rs`)
   - Initializes MCP when created with a tool registry
   - Provides CLI arguments for Claude to use the MCP config

### Usage

#### GUI Mode
MCP is automatically initialized when using the GUI with Claude Code as the LLM provider.

#### CLI Mode
MCP is initialized when running in CLI mode with Claude Code client.

#### Direct MCP Server Mode
You can also run Sagitta Code as a standalone MCP server:

```bash
# Regular MCP server (for external tools)
sagitta-code --mcp

# Internal MCP server (for Claude integration)
sagitta-code --mcp-internal
```

## Testing

The MCP integration includes comprehensive tests:

- `tests/mcp_integration_test.rs` - Tests MCP server functionality
- `tests/mcp_claude_integration_test.rs` - Tests Claude CLI integration

## Logging

When running in MCP mode, all logs are redirected to stderr to avoid interfering with the JSON-RPC protocol on stdout. This is handled automatically by setting `RUST_LOG_TARGET=stderr`.

## Available Tools

All tools registered in the ToolRegistry are exposed through MCP, including:
- File operations (read, edit)
- Repository management (list, search, map)
- Shell execution
- Code search
- And many more...

## Benefits

1. **Standardized Protocol**: Uses the official MCP protocol supported by Claude CLI
2. **Automatic Tool Discovery**: Claude can discover and use all available tools
3. **Type Safety**: Tool parameters are validated through JSON schemas
4. **Clean Integration**: No manual tool registration needed in Claude

## Future Improvements

1. Add support for MCP resources (in addition to tools)
2. Implement tool change notifications
3. Add authentication/authorization for MCP endpoints