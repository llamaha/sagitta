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
    git clone https://gitlab.com/amulvany/vectordb-cli.git
    cd vectordb-cli
    ```

2.  **Download Default Model (Optional):** The default model (`all-minilm-l6-v2`) is included via Git LFS. If you want to use it and haven't already configured LFS:
    ```bash
    # Install Git LFS (if not already installed)
    # Debian/Ubuntu: sudo apt-get install git-lfs
    # macOS: brew install git-lfs
    # Then, install LFS hooks for your user:
    git lfs install
    # Pull the LFS files (downloads the model/tokenizer)
    git lfs pull
    ```
    The default model (`onnx/all-minilm-l6-v2.onnx`) and its tokenizer (`onnx/tokenizer.json`) will be downloaded into the `onnx/` directory.

3.  **Build:**
    ```bash
    cargo build --release
    ```

4.  **Install Binary:** Copy the compiled binary to a location in your `PATH`.
    ```bash
    # Example:
    cp target/release/vectordb-cli ~/.local/bin/
    ```

For GPU acceleration details, see [CUDA Setup](docs/CUDA_SETUP.md) and [macOS GPU Setup](docs/MACOS_GPU_SETUP.md).

## Embedding Models

`vectordb-cli` uses ONNX embedding models for semantic search. You can use the default model or provide your own.

### Default Model (all-MiniLM-L6-v2)

-   **Dimension:** 384
-   **Description:** A fast and effective model suitable for general semantic search.
-   **Setup:** If you followed step 2 of the Installation (using `git lfs pull`), the model and tokenizer are already downloaded in the `onnx/` directory.
-   **Usage:** When using the default model, `vectordb-cli` will automatically find it if you run the tool from the repository root, or if the `onnx/` directory is present in the current working directory. If it cannot find the files, you can specify the paths explicitly (see below).

### Using CodeBERT (or other models)

You can use other sentence-transformer models compatible with ONNX, such as CodeBERT, which is specifically trained on code.

1.  **Generate ONNX Model & Tokenizer:**
    -   Run the provided Python script:
        ```bash
        # Ensure you have Python and necessary libraries (transformers, torch, onnx, tokenizers)
        # pip install transformers torch onnx tokenizers
        python scripts/codebert.py
        ```
    -   This will download the `microsoft/codebert-base` model, convert it to ONNX format, and save it along with its tokenizer files into the `codebert_onnx/` directory.
    -   The script will output instructions on how to use these files with `vectordb-cli`.

2.  **Configure `vectordb-cli`:** You **must** tell `vectordb-cli` where to find the CodeBERT model and tokenizer using **either** environment variables **or** command-line arguments:

    *   **Environment Variables:** (Set these in your shell or `.bashrc`/`.zshrc`)
        ```bash
        export VECTORDB_ONNX_MODEL="/path/to/your/vectordb-cli/codebert_onnx/codebert_model.onnx"
        export VECTORDB_ONNX_TOKENIZER="/path/to/your/vectordb-cli/codebert_onnx/tokenizer"
        ```
        Then run `vectordb-cli index ...` normally.

    *   **Command-Line Arguments (during `index`):**
        ```bash
        vectordb-cli index ./your/code \
          --onnx-model ./codebert_onnx/codebert_model.onnx \
          --onnx-tokenizer ./codebert_onnx/tokenizer
        ```

### Switching Models

**Important:** Different models usually produce embeddings of different dimensions (e.g., MiniLM=384, CodeBERT=768). The vector index (`hnsw_index.json`) is tied to a specific dimension.

-   When you run `vectordb-cli index` using a model with a different dimension than the one used to create the existing index, the tool will automatically detect the mismatch.
-   It will **clear the existing incompatible embeddings** from the database and **create a new vector index** compatible with the new model.
-   Alternatively, you can manually run `vectordb-cli clear` before indexing with a different model to ensure a clean state.

## Usage

### 1. Indexing Files

Create a search index for a directory. You must configure an ONNX model first (see [Embedding Models](#embedding-models)).

```bash
# Index using the default MiniLM model (if present in ./onnx/)
vectordb-cli index /path/to/your/code

# Index using CodeBERT via environment variables (assuming they are set)
vectordb-cli index /path/to/your/code

# Index using CodeBERT via command-line flags
vectordb-cli index /path/to/your/code \
  --onnx-model ./codebert_onnx/codebert_model.onnx \
  --onnx-tokenizer ./codebert_onnx/tokenizer

# Index only specific file types (works with any configured model)
vectordb-cli index /path/to/your/code --file-types rs,md,py

# Use more threads for potentially faster indexing
vectordb-cli index /path/to/your/code -j 8
```