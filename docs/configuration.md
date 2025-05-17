# vectordb-core Configuration Reference

This document describes all configuration options available in `config.toml` for `vectordb-core`, `vectordb-cli`, and `vectordb-mcp`.

---

## Location

The main configuration file is typically located at:

- **Linux/macOS:** `~/.config/vectordb/config.toml`
- **Windows:** `%APPDATA%\vectordb\config.toml`

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

# TLS/HTTPS settings (for vectordb-mcp)
tls_enable = false
tls_cert_path = null
tls_key_path = null

# CORS settings (for vectordb-mcp)
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
internal_embed_batch_size = 128
collection_name_prefix = "repo_"
max_file_size_bytes = 5242880
```
- `batch_size` (integer): Batch size for indexing.
- `internal_embed_batch_size` (integer): Batch size for embedding.
- `collection_name_prefix` (string): Prefix for Qdrant collections.
- `max_file_size_bytes` (integer): Max file size to index (bytes).

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

## Multi-User / Multi-Tenancy Mode

- By default, the CLI and server operate in single-user mode unless tenant_id is set or required by the operation.
- In MCP server deployments, tenant_id is determined by the API key or authentication context.
- To enable multi-user mode explicitly, set `tenant_id` to `null` or omit it, and use API keys for tenant isolation.
- For scripting or single-tenant use, you may set `tenant_id` globally in config.toml.

---

## Updating Configuration

- Edit `config.toml` directly, or use `vectordb-cli` commands where available.
- After editing, restart any running servers or re-run CLI commands to pick up changes.

---

## See Also
- [README.md](../README.md) for setup and usage instructions.
- [multi_tenancy_api_keys.md](./multi_tenancy_api_keys.md) for multi-tenancy and API key details.

---

*If you add new configuration options, please update this file!*

## tenant_id

- **Type:** `String` (optional)
- **Purpose:** If set, this value is used as the default tenant ID for all CLI and server operations. It enables multi-tenancy and is required for most repository operations.
- **How to set:**
  - Run `vectordb-cli init` to generate a new UUID and write it to your config.
  - Or, manually add a line like `tenant_id = "your-uuid-here"` to your `config.toml`.
- **Multi-user mode:**
  - If you want to use vectordb-core in a multi-tenant environment, each user or automation should have a unique `tenant_id`.
  - If you want to run in single-user mode, just use the generated `tenant_id`.

**Note:** If neither `--tenant-id` nor `tenant_id` in config is set, most CLI commands will error out. See the [README](../README.md) for more details and usage examples. 