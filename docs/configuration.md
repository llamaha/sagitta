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

## Performance Tuning Guide

Optimizing indexing performance involves tuning several parameters based on your specific hardware (CPU cores, GPU VRAM), the embedding model in use, and the nature of your codebase. Here's a guide to the key settings:

### `internal_embed_batch_size` (integer)
- **Controls**: The number of code chunks sent to the ONNX embedding model on the GPU in a single pass for generating dense embeddings. This happens *within each parallel processing thread*.
- **Primary Constraint**: GPU VRAM. The model, input data, and output embeddings for this batch must fit in your GPU's memory.
- **Tuning**:
    - **Goal**: Maximize GPU utilization without causing Out-of-Memory (OOM) errors.
    - **Starting Point**: For a moderately sized model (e.g., `st-codesearch-distilroberta-base` ~120M parameters) on an 8GB GPU, try values between **16 and 64**.
    - If you use fewer parallel Rayon threads (see `RAYON_NUM_THREADS` below, e.g., 2-4), you can often afford a larger `internal_embed_batch_size` per thread.
    - **Method**: Monitor GPU VRAM usage. Incrementally increase this value until VRAM is almost fully utilized during embedding, or until you encounter OOM errors, then reduce it slightly.
- **Location**: `[performance]` table in `config.toml`.

### `RAYON_NUM_THREADS` (Environment Variable)
- **Controls**: The number of threads Rayon (the parallel processing library in Rust) uses for its global thread pool. For repository indexing, each Rayon worker thread can create a thread-local ONNX model instance.
- **Primary Constraint**: GPU VRAM, due to multiple ONNX model instances. Each instance consumes VRAM.
- **Tuning**:
    - **Goal**: Maximize the number of parallel workers that can concurrently perform GPU-bound embedding without exhausting VRAM.
    - **Starting Point**: For an 8GB GPU, try **2 to 6 threads**. `export RAYON_NUM_THREADS=4` is a common starting value.
    - If each model instance + its `internal_embed_batch_size` uses 1.5-2GB VRAM, you can only fit a few such instances on the GPU.
    - **Method**: Start with a moderate number (e.g., 4). If VRAM usage is too high, reduce `RAYON_NUM_THREADS`. If VRAM allows and CPU cores are available, you can try increasing it. Note that CPU cores will also be busy with I/O, parsing, and sparse vector creation.
- **How to set**: Set it as an environment variable before running your application (e.g., `export RAYON_NUM_THREADS=4`).

### `batch_size` (integer)
- **Controls**: The number of fully processed data points (chunks with their dense embeddings, sparse embeddings, and metadata) that are grouped together before being sent to the Qdrant database in a single upsert operation.
- **Primary Constraint**: Network efficiency, Qdrant's ingestion capacity, and client-side RAM (for holding points before sending). Not typically GPU VRAM limited.
- **Tuning**:
    - **Goal**: Efficiently upload data to Qdrant with minimal network overhead, without overwhelming the client or server.
    - **Starting Point**: Values between **64 and 256** are usually reasonable. Try `128`.
    - **Method**: If uploads are slow due to many small requests, or if Qdrant seems underutilized during this phase, try increasing `batch_size`. If you encounter timeouts or high client RAM usage during the "uploading to Qdrant" phase, reduce it.
- **Location**: `[performance]` table in `config.toml`.

### `max_concurrent_upserts` (integer)
- **Controls**: The maximum number of concurrent asynchronous tasks allowed for uploading batches of points to Qdrant. This is particularly relevant for the `index_repo_files` function.
- **Primary Constraint**: Network bandwidth, Qdrant's ability to handle concurrent connections and writes, and client-side resources for managing these tasks.
- **Tuning**:
    - **Goal**: Saturate the network connection to Qdrant and maximize Qdrant's write throughput without causing network errors or excessive load.
    - **Starting Point**: A value like **4 to 16** is often a good starting point. The default is 8.
    - **Method**: If network and Qdrant CPU seem underutilized during the upload phase, you can try increasing this. If you see network errors, timeouts, or Qdrant becoming unresponsive, reduce this value. This interacts with `batch_size` – many small concurrent batches behave differently than a few large ones.
- **Location**: `[indexing]` table in `config.toml`.

**General Tuning Strategy:**
1.  **Set `RAYON_NUM_THREADS`**: Start with a conservative number (e.g., 4 for 8GB GPU).
2.  **Tune `internal_embed_batch_size`**: With `RAYON_NUM_THREADS` fixed, adjust `internal_embed_batch_size` to maximize GPU VRAM usage during embedding without OOMs.
3.  **Re-adjust `RAYON_NUM_THREADS`**: If VRAM allows after tuning `internal_embed_batch_size`, you might try slightly increasing `RAYON_NUM_THREADS`.
4.  **Tune `batch_size` and `max_concurrent_upserts`**: Once embedding is optimized, tune these for the Qdrant upload phase.

Monitor system resources (GPU VRAM, GPU utilization, CPU utilization, network I/O) throughout the process.

**Interpreting CPU Utilization (in relation to `RAYON_NUM_THREADS`):**
- Monitor your application's total CPU usage (e.g., using `htop` or Task Manager).
- If `RAYON_NUM_THREADS` is set to `N`, and your CPU usage is consistently much lower than `N * 100%` (e.g., you set 6 threads but CPU usage is 200-300%), it suggests that the Rayon threads are often waiting, not CPU-bound.
- If the GPU is highly utilized or VRAM is near capacity in this scenario, it indicates the GPU is likely the bottleneck. Consider reducing `RAYON_NUM_THREADS` (e.g., to 2-4 for an 8GB GPU) to lessen VRAM pressure and GPU contention. This might then allow you to slightly increase `internal_embed_batch_size` for the remaining threads, improving their individual efficiency.

**Important Note on GPU Memory (VRAM):**
- When indexing large repositories, closely monitor your GPU VRAM usage (e.g., using `nvidia-smi` on Linux for NVIDIA GPUs).
- Aim to keep VRAM consumption safely below 90-95% of your GPU's total capacity during sustained indexing. Pushing to 100% risks Out-of-Memory (OOM) errors.
- The application will report processing errors, including OOM-related failures from the embedding engine, in a summary after indexing completes.
- If you encounter OOM errors (often manifesting as "Failed to allocate memory" or errors during ONNX node execution like `MatMul`), the most effective solutions are:
    1.  **Reduce `internal_embed_batch_size`**: This lessens the memory load per embedding operation.
    2.  **Reduce `RAYON_NUM_THREADS`**: This decreases the number of concurrent ONNX model instances trying to use VRAM.

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
