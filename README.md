# vectordb-cli

A lightweight command-line tool for fast, local search across your codebases and text files using both semantic (vector) and lexical (keyword) retrieval.

## Features

-   **Hybrid Search:** Combines deep semantic understanding (via ONNX models) with efficient BM25 lexical matching for relevant results.
-   **Local First:** Indexes and searches files directly on your machine. No data leaves your system.
-   **Simple Indexing:** Recursively indexes specified directories.
-   **Configurable:** Supports custom ONNX embedding models and tokenizers.
-   **Cross-Platform:** Runs on Linux and macOS.

## Prerequisites

-   **Rust:** Required for building the project. Install from [rustup.rs](https://rustup.rs/).
    ```bash
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    ```
-   **(Optional) Git LFS:** Needed only if you intend to use the default embedding model provided in the repository via Git LFS.
    ```bash
    # Debian/Ubuntu: sudo apt-get install git-lfs
    # macOS: brew install git-lfs
    git lfs install 
    ```

## Installation

1.  **Clone the Repository:**
    ```bash
    # If using the default model, ensure LFS is installed first
    # git lfs install 
    git clone https://gitlab.com/amulvany/vectordb-cli.git 
    cd vectordb-cli
    # If using the default model, pull the LFS files
    # git lfs pull 
    ```

2.  **Build:**
    ```bash
    cargo build --release
    ```

3.  **Install Binary:** Copy the compiled binary to a location in your `PATH`.
    ```bash
    # Example:
    cp target/release/vectordb-cli ~/.local/bin/ 
    ```

## ONNX Model Setup (Required)

`vectordb-cli` requires an ONNX embedding model and its corresponding tokenizer file for semantic search.

**Default:** If you cloned the repository using `git lfs pull`, the default model (`all-minilm-l12-v2.onnx`) and tokenizer (`minilm_tokenizer.json`) should be present in an `./onnx/` subdirectory relative to where you run the tool.

**Manual Configuration:** If the default files aren't found, you **must** specify their paths using **either** environment variables **or** command-line arguments during indexing:

*   **Environment Variables:**
    ```bash
    export VECTORDB_ONNX_MODEL="/path/to/your/model.onnx"
    export VECTORDB_ONNX_TOKENIZER="/path/to/your/tokenizer.json"
    ```
    (Add these to your `~/.bashrc`, `~/.zshrc`, etc.)

*   **Command-Line Arguments (during `index`):**
    ```bash
    vectordb-cli index ./your/code --onnx-model /path/to/model.onnx --onnx-tokenizer /path/to/tokenizer.json
    ```

Failure to provide a valid model and tokenizer will result in an error.

## GPU Acceleration (CUDA)

To enable GPU acceleration using CUDA, follow these steps:

1.  **Install CUDA:** Ensure you have a compatible NVIDIA driver and the CUDA Toolkit installed. You can verify your installation using `nvidia-smi` and `nvcc --version`.
2.  **Enable `ort` CUDA Feature:** When building or running your project, enable the `cuda` feature for the `ort` crate. **This tells the `ort` build process to download and use the GPU-enabled version of the ONNX Runtime library.**
    *   **Option 1 (Recommended):** Add the feature to your `Cargo.toml`:
        ```toml
        [dependencies]
        ort = { version = "2.0.0-rc.9", features = ["cuda"] }
        ```
    *   **Option 2:** Enable the feature via the command line:
        ```bash
        cargo build --features ort/cuda
        cargo run --features ort/cuda
        ```
3.  **Configure Session:** In your Rust code, configure the `SessionBuilder` to use the CUDA execution provider:
    ```rust
    use ort::{execution_providers::CUDAExecutionProvider, Session, SessionBuilder};

    // ...

    let providers = [CUDAExecutionProvider::default().build()];
    let session = Session::builder()?
        .with_execution_providers(providers)?
        .commit_from_file("your_model.onnx")?; // Replace with your model path

    // ... use the session ...
    ```

    You can customize the `CUDAExecutionProvider` further if needed (e.g., selecting a specific GPU device). Refer to the `ort` documentation for details.

## Usage

### 1. Indexing Files

Create a search index for a directory. By default, the tool only indexes files with common source code and text extensions (e.g., `.rs`, `.go`, `.py`, `.js`, `.ts`, `.md`, `.txt`, etc. - see `VectorDB::get_supported_file_types()` in the code for the full default list). Use the `--file-types` flag to specify your own list or override the defaults. The content of these allowed files is then processed as text by the semantic embedding model.

Run this command from the root of the `vectordb-cli` directory or ensure the `onnx/` subdirectory (or configured paths) are accessible.

```bash
# Index a directory using default file types (assuming default ./onnx/ model)
vectordb-cli index /path/to/your/code

# Index only specific file types
vectordb-cli index /path/to/your/code --file-types rs,md,py

# Index using specific model paths
vectordb-cli index /path/to/your/code \
  --onnx-model /custom/model.onnx \
  --onnx-tokenizer /custom/tokenizer.json

# Use more threads for potentially faster indexing
vectordb-cli index /path/to/your/code -j 8 
```

### 2. Searching the Index

Query the index using natural language or keywords.

```bash
# Hybrid search (semantic + lexical) - default
vectordb-cli query "how does the authentication logic work?"

# Limit the number of results (default is 20)
vectordb-cli query "error handling in the API" --limit 5

# Perform only vector (semantic) search
vectordb-cli query "database schema migration" --vector-only

# Adjust hybrid search weights (defaults are vector=0.6, bm25=0.4)
# Increase BM25 for more keyword focus:
vectordb-cli query "struct DatabaseConfig" --vector-weight 0.3 --bm25-weight 0.7
# Increase vector for more semantic focus:
vectordb-cli query "async task processing" --vector-weight 0.8 --bm25-weight 0.2
```

### 3. Other Commands

```bash
# Show database statistics
vectordb-cli stats

# Clear the entire search index and database
vectordb-cli clear 
```

## Database Location

The search index and database are stored locally within your user's data directory:

-   **Linux:** `~/.local/share/vectordb-cli/`
-   **macOS:** `~/Library/Application Support/vectordb-cli/`

The primary database file is typically named `vectordb.json` within this directory. To backup, simply copy this directory or the `vectordb.json` file.

## License

MIT 