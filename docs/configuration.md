# sagitta-search Configuration Reference

This document describes all configuration options available in `config.toml` for `sagitta-search`, `sagitta-cli`, and `sagitta-mcp`.

---

## Location

The main configuration file is typically located at:

- **Linux/macOS:** `~/.config/sagitta/config.toml`
- **Windows:** `%APPDATA%\sagitta\config.toml`

---

## Example config.toml

```toml
qdrant_url = "http://localhost:6334"
onnx_model_path = "/absolute/path/to/model.onnx"
onnx_tokenizer_path = "/absolute/path/to/tokenizer.json" # or directory containing tokenizer.json
vector_dimension = 384 # Moved to top-level
repositories_base_path = "/absolute/path/to/repos"
vocabulary_base_path = "/absolute/path/to/vocab"
# tenant_id = "your-tenant-uuid"  # Optional: see below

# Embedding engine configuration
[embedding]
max_sessions = 4                    # Maximum number of concurrent ONNX sessions
enable_cuda = false                 # Enable CUDA acceleration
max_sequence_length = 128           # Maximum sequence length for tokenization
session_timeout_seconds = 300       # Session timeout in seconds (0 = no timeout)
enable_session_cleanup = true       # Enable session cleanup on idle

# TLS/HTTPS settings (for sagitta-mcp)
tls_enable = false
tls_cert_path = null
tls_key_path = null

# CORS settings (for sagitta-mcp)
cors_allowed_origins = null
cors_allow_credentials = true
```

---

## Configuration Options

### `qdrant_url` (string, required)
URL for the Qdrant vector database instance. Example: `"http://localhost:6334"`

### `onnx_model_path` (string, required)
Absolute path to the ONNX embedding model file. Example: `"/path/to/model.onnx"`

### `onnx_tokenizer_path` (string, required)
Absolute path to the tokenizer file (e.g., `tokenizer.json`) or directory containing it. Example: `"/path/to/tokenizer.json"` or `"/path/to/tokenizer_dir"`

### `vector_dimension` (integer, required)
Dimension of the vectors produced by the embedding model. Must match the model's output. Example: `384` for `all-minilm-l6-v2`.

### `repositories_base_path` (string, optional)
Base directory where repositories are cloned and managed. Example: `"/path/to/repos"`

### `vocabulary_base_path` (string, optional)
Base directory for storing vocabulary files. Example: `"/path/to/vocab"`

### `tenant_id` (string, optional, advanced)
**Multi-Tenancy:**
- If set, this value will be used as the default tenant for all CLI/server operations (unless overridden by CLI argument or API key).
- Useful for single-tenant deployments or for scripting.
- In multi-user mode (MCP server), tenant_id is usually determined by the API key or authentication context.

### `repositories` (array of tables, advanced)
List of managed repositories. Normally managed by the CLI/server, not edited manually.

### `active_repository` (string, optional)
Name of the currently active repository. Set via CLI (`repo use <name>`).

### `indexing` (table, advanced)
Indexing configuration. Example:
```toml
[indexing]
max_concurrent_upserts = 8
```
- `max_concurrent_upserts` (integer): Maximum number of concurrent upsert operations.

### `performance` (table, advanced)
Performance tuning options. Example:
```toml
[performance]
batch_size = 256
collection_name_prefix = "repo_"
max_file_size_bytes = 5242880
vector_dimension = 384
```
- `batch_size` (integer, default: 256): Batch size for Qdrant upserts.
- `collection_name_prefix` (string, default: "repo_"): Prefix for Qdrant collections.
- `max_file_size_bytes` (integer, default: 5242880): Max file size to index (bytes).
- `vector_dimension` (integer, default: 384): Default vector dimension for embeddings.

### `embedding` (table, advanced)
Embedding engine configuration. Example:
```toml
[embedding]
max_sessions = 4
enable_cuda = false
max_sequence_length = 128
session_timeout_seconds = 300
enable_session_cleanup = true
```
- `max_sessions` (integer, default: 4): Maximum number of concurrent ONNX sessions for session pooling. Higher values allow more parallel embedding generation but use more GPU memory.
- `enable_cuda` (bool, default: false): Enable CUDA acceleration for embedding generation. Requires CUDA-compatible hardware and drivers.
- `max_sequence_length` (integer, default: 128): Maximum sequence length for tokenization. Longer sequences use more memory and processing time.
- `session_timeout_seconds` (integer, default: 300): Session timeout in seconds. Set to 0 for no timeout.
- `enable_session_cleanup` (bool, default: true): Enable automatic cleanup of idle sessions to free memory.

### `oauth` (table, optional, advanced)
OAuth2 configuration for MCP server. Example:
```toml
[oauth]
client_id = "..."
client_secret = "..."
auth_url = "..."
token_url = "..."
user_info_url = "..."
redirect_uri = "..."
introspection_url = null
scopes = ["openid", "profile", "email"]
```

### `tls_enable` (bool, default: false)
Enable TLS/HTTPS for MCP server.

### `tls_cert_path` (string, optional)
Path to TLS certificate file.

### `tls_key_path` (string, optional)
Path to TLS private key file.

### `cors_allowed_origins` (array of strings, optional)
List of allowed origins for CORS (MCP server).

### `cors_allow_credentials` (bool, default: true)
Allow credentials in CORS requests.

---

## Performance Tuning Guide

The embedding generation is now handled by the `sagitta-embed` crate which has its own internal optimizations. Key performance settings are:

### `batch_size` (integer)
- **Controls**: The number of fully processed data points (chunks with their dense embeddings, sparse embeddings, and metadata) that are grouped together before being sent to the Qdrant database in a single upsert operation.
- **Primary Constraint**: Network efficiency, Qdrant's ingestion capacity, and client-side RAM (for holding points before sending).
- **Tuning**:
    - **Goal**: Efficiently upload data to Qdrant with minimal network overhead, without overwhelming the client or server.
    - **Starting Point**: Values between **64 and 256** are usually reasonable. Try `128`.
    - **Method**: If uploads are slow due to many small requests, or if Qdrant seems underutilized during this phase, try increasing `batch_size`. If you encounter timeouts or high client RAM usage during the "uploading to Qdrant" phase, reduce it.
- **Location**: `[performance]` table in `config.toml`.

### `max_concurrent_upserts` (integer)
- **Controls**: The maximum number of concurrent asynchronous tasks allowed for uploading batches of points to Qdrant.
- **Primary Constraint**: Network bandwidth, Qdrant's ability to handle concurrent connections and writes, and client-side resources for managing these tasks.
- **Tuning**:
    - **Goal**: Saturate the network connection to Qdrant and maximize Qdrant's write throughput without causing network errors or excessive load.
    - **Starting Point**: A value like **4 to 16** is often a good starting point. The default is 8.
    - **Method**: If network and Qdrant CPU seem underutilized during the upload phase, you can try increasing this. If you see network errors, timeouts, or Qdrant becoming unresponsive, reduce this value.
- **Location**: `[indexing]` table in `config.toml`.

### Embedding Engine Settings
The embedding engine configuration in the `[embedding]` section is handled by the `sagitta-embed` crate:
- `max_sessions`: Controls session pooling for parallel embedding generation
- `enable_cuda`: Enables GPU acceleration if available
- `max_sequence_length`: Controls tokenization limits
- `session_timeout_seconds` and `enable_session_cleanup`: Control session lifecycle

For detailed embedding performance tuning, refer to the `sagitta-embed` documentation.

---

## Multi-User / Multi-Tenancy Mode

- By default, the CLI and server operate in single-user mode unless tenant_id is set or required by the operation.
- In MCP server deployments, tenant_id is determined by the API key or authentication context.
- To enable multi-user mode explicitly, set `tenant_id` to `null` or omit it, and use API keys for tenant isolation.
- For scripting or single-tenant use, you may set `tenant_id` globally in config.toml.

---

## Updating Configuration

- Edit `config.toml` directly, or use `sagitta-cli` commands where available.
- After editing, restart any running servers or re-run CLI commands to pick up changes.

---

## See Also
- [README.md](../README.md) for setup and usage instructions.
---

*If you add new configuration options, please update this file!*

## tenant_id

- **Type:** `String` (optional)
- **Purpose:** If set, this value is used as the default tenant ID for all CLI and server operations. It enables multi-tenancy and is required for most repository operations.
- **How to set:**
  - Run `sagitta-cli init` to generate a new UUID and write it to your config.
  - Or, manually add a line like `tenant_id = "your-uuid-here"` to your `config.toml`.
- **Multi-user mode:**
  - If you want to use sagitta-search in a multi-tenant environment, each user or automation should have a unique `tenant_id`.
  - If you want to run in single-user mode, just use the generated `tenant_id`.

**Note:** If neither `--tenant-id` nor `tenant_id` in config is set, most CLI commands will error out. See the [README](../README.md) for more details and usage examples. 

# Performance Configuration

This section controls performance-related settings:

## Options:

- **`batch_size`** (integer, default: 256)
  - Batch size for Qdrant upserts
  - Larger values may improve throughput but use more memory

- **`collection_name_prefix`** (string, default: "repo_")
  - Prefix for collection names in Qdrant
  - Useful for namespacing when sharing a Qdrant instance

- **`max_file_size_bytes`** (integer, default: 5242880)
  - Maximum file size in bytes that will be processed
  - Files larger than this will be skipped during indexing

- **`vector_dimension`** (integer, default: 384)
  - Default vector dimension for embeddings
  - Must match the dimension of your embedding model
