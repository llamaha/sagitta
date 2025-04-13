# vectordb-cli

A lightweight command-line tool for fast, local code search using semantic retrieval powered by ONNX models and Qdrant.

**Note:** This repository contains both the `vectordb-cli` command-line tool and the underlying `vectordb_lib` library.

## Features

-   **Semantic Search:** Finds relevant code chunks based on meaning using ONNX models.
-   **Qdrant Backend:** Utilizes a Qdrant vector database instance for scalable storage and efficient search.
-   **Local or Remote Qdrant:** Can connect to a local Dockerized Qdrant or a remote instance.
-   **Simple Indexing:** Recursively indexes specified directories.
-   **Configurable:** Supports custom ONNX embedding models/tokenizers and Qdrant connection details via config file or environment variables.

## Supported Languages

This tool uses tree-sitter for accurate code chunking. The following languages are currently supported for AST-based chunking (falling back to whole-file chunking for others):

*   **Rust** (`.rs`)
*   **Markdown** (`.md`, `.mdx`)
*   **Go** (`.go`)
*   **JavaScript** (`.js`, `.jsx`)
*   **TypeScript** (`.ts`, `.tsx`)
*   **YAML** (`.yaml`, `.yml`)
*   **Ruby** (`.rb`)
*   **Python** (`.py`)

**Planned Languages:**

Support for the following languages is planned for future releases:

*   Java (`.java`)
*   C# (`.cs`)
*   C++ (`.cpp`, `.h`, `.hpp`)
*   C (`.c`, `.h`)
*   PHP (`.php`)
*   Swift (`.swift`)
*   Kotlin (`.kt`, `.kts`)
*   HTML (`.html`)
*   CSS (`.css`)
*   JSON (`.json`)

## Prerequisites

-   **Rust:** Required for building the project. Install from [rustup.rs](https://rustup.rs/).
    ```bash
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    ```
-   **Build Tools:** Rust often requires a C linker and build tools.
    -   **Linux (Debian/Ubuntu):**
        ```bash
        sudo apt-get update && sudo apt-get install build-essential
        ```
    -   **macOS:** Install the Xcode Command Line Tools. If you don't have Xcode installed, running the following command in your terminal will prompt you to install them:
        ```bash
        xcode-select --install
        ```
-   **Qdrant:** A Qdrant instance (v1.7.0 or later recommended) must be running and accessible. See [Qdrant Setup](#qdrant-setup).
-   **ONNX Model Files:** An ONNX embedding model and its corresponding tokenizer files are required. See [Embedding Models](#embedding-models).

## Qdrant Setup

`vectordb-cli` requires a running Qdrant instance.

**Option 1: Docker (Recommended for Local Use)**

```bash
docker run -p 6333:6333 -p 6334:6334 \
    -v $(pwd)/qdrant_storage:/qdrant/storage:z \
    qdrant/qdrant:latest
```

This starts Qdrant with the default gRPC port (6333) and HTTP/REST port (6334) mapped to your host. Data will be persisted in the `qdrant_storage` directory in your current working directory.

**Option 2: Qdrant Cloud or Other Deployment**

Follow the instructions for your chosen deployment method. You will need the **URL** (including `http://` or `https://` and the port, typically 6333 for gRPC) and potentially an **API Key** if required by your setup.

## Installation

1.  **Clone the Repository:**
    ```bash
    git clone https://gitlab.com/amulvany/vectordb-cli.git
    cd vectordb-cli
    ```

2.  **Prepare ONNX Model & Tokenizer:**
    Download or obtain your desired ONNX embedding model (`.onnx` file) and its tokenizer configuration (`tokenizer.json` and potentially other files like `vocab.txt`, `merges.txt`, etc., usually in a single directory). Place them in a known location. The repository includes `onnx/all-minilm-l12-v2.onnx` and `onnx/minilm_tokenizer.json` as an example (download via `git lfs pull` if needed).
    
    **Note:** The tool dynamically detects the embedding dimension from the provided `.onnx` model. Any dimension model (e.g., 384, 768) can be used.

3.  **Build:**
    *   **Standard (CPU):**
        ```bash
        cargo build --release
        ```
    *   **With CUDA GPU Support (Linux):** Ensure you have NVIDIA drivers and the CUDA toolkit installed. Then build with:
        ```bash
        cargo build --release --features ort/cuda
        ```

4.  **Understanding the Build Process (Linux/macOS):**
    *   The project uses a build script (`build.rs`) to simplify setup.
    *   During the build, this script automatically finds the necessary ONNX Runtime libraries (downloaded by the `ort` crate to `~/.cache/ort.pyke.io/`) including provider-specific libraries (like CUDA `.so` files or macOS `.dylib` files).
    *   It copies these libraries into the final build output directory (`target/release/lib/`).
    *   It sets the necessary RPATH (`$ORIGIN/lib` on Linux, `@executable_path/lib` on macOS) on the `vectordb-cli` executable.
    *   This means you typically **do not** need to manually set `LD_LIBRARY_PATH` (Linux) or `DYLD_LIBRARY_PATH` (macOS).

5.  **Install Binary (Optional):** Copy the compiled binary to a location in your `PATH`.
    ```bash
    # Example for Linux/macOS
    cp target/release/vectordb-cli ~/.local/bin/ 
    ```

## Configuration

`vectordb-cli` uses a hierarchical configuration system:

1.  **Command-line Arguments:** Highest priority (e.g., `--onnx-model`, `--onnx-tokenizer-dir`).
2.  **Environment Variables:** Second priority.
3.  **Configuration File (`config.toml`):** Lowest priority.

### Environment Variables

-   `QDRANT_URL`: URL of the Qdrant gRPC endpoint (e.g., `http://localhost:6334`). Defaults to `http://localhost:6334` if not set.
-   `VECTORDB_ONNX_MODEL`: Full path to the `.onnx` model file.
-   `VECTORDB_ONNX_TOKENIZER_DIR`: Full path to the directory containing the `tokenizer.json` file.

### Configuration File (`config.toml`)

The tool looks for a `config.toml` file in the XDG configuration directory:

*   **Linux/macOS:** `~/.config/vectordb-cli/config.toml`

**Example `config.toml`:**

```toml
# URL for the Qdrant gRPC endpoint
qdrant_url = "http://localhost:6333"

# --- Optional: ONNX Model Configuration ---
# These are only needed if not provided via args or env vars.

# Path to the ONNX model file
# onnx_model_path = "/path/to/your/model.onnx"

# Path to the directory containing tokenizer.json
# Note: Key name is `onnx_tokenizer_path`
onnx_tokenizer_path = "/path/to/your/tokenizer_directory"

# --- Optional: Qdrant API Key ---
# api_key = "your_qdrant_api_key"
```

**Note:** You *must* provide the ONNX model and tokenizer paths via one of these methods (arguments, environment variables, or config file) for commands like `index` and `query` to work.

## Usage

The tool interacts with a Qdrant collection named `vectordb-code-search` by default.

### `index`

Indexes files from one or more directories into the Qdrant collection.

```bash
# Index a single directory (using config/env for Qdrant/ONNX)
vectordb-cli index /path/to/your/code

# Index multiple directories
vectordb-cli index /path/to/repoA /path/to/repoB

# Index specific file types (e.g., Rust and Markdown)
vectordb-cli index /path/to/project -t rs md

# Override ONNX paths via arguments
vectordb-cli index /path/to/project \
  --onnx-model /custom/model.onnx \
  --onnx-tokenizer-dir /custom/tokenizer

# Set chunking parameters
vectordb-cli index /path/to/project --chunk-max-length 1024 --chunk-overlap 128
```

**Arguments:**
- `dirs`: (Required) One or more directory paths to index.
- `-t, --type`: Optional file extensions to include (without dots, e.g., `rs py md`).
- `--chunk-max-length`: Max lines per text chunk (default: 512).
- `--chunk-overlap`: Lines of overlap between chunks (default: 64).
- `--onnx-model`: (Global) Override path to ONNX model file.
- `--onnx-tokenizer-dir`: (Global) Override path to ONNX tokenizer directory.

### `query`

Performs a semantic search query against the indexed data.

```bash
# Basic query (using config/env for Qdrant/ONNX)
vectordb-cli query "database connection logic"

# Limit results
vectordb-cli query "error handling" -l 5

# Filter results by file type
vectordb-cli query "user authentication schema" -t sql yaml

# Adjust context lines shown in results
vectordb-cli query "async function example" --context 5

# Override ONNX paths via arguments
vectordb-cli query "search term" \
  --onnx-model /custom/model.onnx \
  --onnx-tokenizer-dir /custom/tokenizer
```

**Arguments:**
- `query`: (Required) The natural language search query.
- `-l, --limit`: Max number of results (default: 10).
- `-t, --type`: Optional file extensions to filter results (without dots).
- `--context`: Number of context lines before/after match (default: 2).
- `--onnx-model`: (Global) Override path to ONNX model file.
- `--onnx-tokenizer-dir`: (Global) Override path to ONNX tokenizer directory.

### `stats`

Displays statistics about the Qdrant collection (`vectordb-code-search`).

```bash
# Show stats (using config/env for Qdrant)
vectordb-cli stats
```

(No specific arguments)

### `list`

Lists the unique root directories that have been indexed into the collection.

```bash
# List indexed directories (using config/env for Qdrant)
vectordb-cli list
```

(No specific arguments)

### `clear`

Removes data from the Qdrant collection.

```bash
# Clear ALL data from the collection (requires confirmation)
vectordb-cli clear --all

# Clear all data, skipping confirmation
vectordb-cli clear --all -y

# Clear data associated with a specific indexed directory (requires confirmation)
vectordb-cli clear --directory /path/to/indexed/repoA

# Clear directory data, skipping confirmation
vectordb-cli clear --directory /path/to/indexed/repoA -y
```

**Arguments:**
- `--all`: Flag to remove all data by deleting the collection.
- `--directory <PATH>`: Remove indexed points originating from the specified directory.
- `-y, --yes`: Skip the confirmation prompt.

**Note:** You must provide either `--all` or `--directory`.

## Development

(Include instructions for setting up the dev environment, running tests, etc.)

```bash
# Run tests
cargo test

# Run clippy
cargo clippy --all-targets -- -D warnings

# Format code
cargo fmt
```

## Contributing

(Contribution guidelines)

## License

MIT License

## Language Support

The CLI uses `tree-sitter` for Abstract Syntax Tree (AST) parsing to extract meaningful code chunks (like functions, classes, structs) for indexing. This leads to more contextually relevant search results compared to simple line-based splitting.

Here is the current status of language support:

| Language   | Status          | Supported Elements                             |
| :--------- | :-------------- | :--------------------------------------------- |
| Rust       | ✅ Supported    | functions, structs, enums, impls, traits, mods, macros, use, extern crates, type aliases, unions, statics, consts |
| Ruby       | ✅ Supported    | modules, classes, methods, singleton_methods   |
| Go         | ✅ Supported    | functions, methods, types (struct/interface), consts, vars |
| JavaScript | ⏳ Planned      | (TBD)                                          |
| TypeScript | ⏳ Planned      | (TBD)                                          |
| Markdown   | ⏳ Planned      | (TBD - maybe sections, code blocks?)           |
| YAML       | ⏳ Planned      | (TBD - maybe top-level keys?)                  |
| Other      | ✅ Supported    | Line-based chunks (`fallback_chunk`)           |

Files with unsupported extensions will automatically use the line-based fallback mechanism. 