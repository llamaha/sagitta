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
# Embedding engine configuration
[embedding]
# Session management is now automatic
max_sequence_length = 128           # Maximum sequence length for tokenization
session_timeout_seconds = 300       # Session timeout in seconds (0 = no timeout)
enable_session_cleanup = true       # Enable session cleanup on idle
embedding_batch_size = 128
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
# Session management is now automatic
max_sequence_length = 128
session_timeout_seconds = 300
enable_session_cleanup = true
embedding_batch_size = 128
```
- Session management is now automatic, optimizing GPU memory usage based on available resources.
- `max_sequence_length` (integer, default: 128): Maximum sequence length for tokenization. Longer sequences use more memory and processing time.
- `session_timeout_seconds` (integer, default: 300): Session timeout in seconds. Set to 0 for no timeout.
- `enable_session_cleanup` (bool, default: true): Enable automatic cleanup of idle sessions to free memory.
- `embedding_batch_size` (integer, default: 128): Number of texts processed together by a single model instance. Higher values improve throughput per model but use more VRAM per model.

**Performance Tuning:**
- **Session Management**: Automatic optimization of parallelism and GPU memory usage based on available resources.
- **`embedding_batch_size`**: Controls throughput per model instance and VRAM per model. Increase for better throughput (uses more VRAM per model). Decrease to reduce memory usage per model.
- **Interaction**: A single large operation (like `repo add`) will automatically manage model instances in parallel, each processing `embedding_batch_size` texts at once.

**CUDA Support**: CUDA acceleration is automatically enabled if the application was compiled with CUDA support and compatible hardware is available. No configuration needed.


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
- Automatic session pooling for parallel embedding generation
- `max_sequence_length`: Controls tokenization limits
- `session_timeout_seconds` and `enable_session_cleanup`: Control session lifecycle
- `embedding_batch_size`: Controls batch size for individual model instances

For detailed embedding performance tuning, refer to the `sagitta-embed` documentation.

---


---

## Updating Configuration

- Edit `config.toml` directly, or use `sagitta-cli` commands where available.
- After editing, restart any running servers or re-run CLI commands to pick up changes.

---

## See Also
- [README.md](../README.md) for setup and usage instructions.
---

*If you add new configuration options, please update this file!*

 

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
