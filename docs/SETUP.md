# Project Setup Guide: `vectordb-core` and Tools

This guide provides instructions to set up your environment, build the `vectordb-core` library, and its associated tools (`vectordb-cli`, `vectordb-mcp`).

## Core Prerequisites

1.  **Rust**: Install from [rustup.rs](https://rustup.rs/)
    ```bash
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    source "$HOME/.cargo/env"
    ```

2.  **ONNX Runtime**: `vectordb-core` uses ONNX Runtime for its embedding models.
    *   Please follow the installation and `LD_LIBRARY_PATH` (or equivalent for your OS) setup instructions in the main [../README.md#prerequisites](../README.md#prerequisites) document.
    *   Ensure this is done before attempting to run applications that use `vectordb-core` with ONNX models (like `vectordb-cli`).

3.  **Qdrant (Vector Database)**: If you plan to use Qdrant as the vector store (default for `vectordb-cli`):
    *   Running via Docker is recommended:
        ```bash
        docker run -d --name qdrant_db -p 6333:6333 -p 6334:6334 \\
            -v $(pwd)/qdrant_storage:/qdrant/storage:z \\
            qdrant/qdrant:latest
        ```
    *   *(Note: The `-d` flag runs it in the background. Use `docker logs qdrant_db` to view logs and `docker stop qdrant_db` to stop.)*

## Building the Project

1.  **Clone the Repository**:
    ```bash
    # Replace with the actual repository URL
    git clone <your-repository-url>
    cd vectordb-core # Or your repository's root directory name
    ```

2.  **Build `vectordb-core` and Tools**:
    *   To build everything in the workspace (including `vectordb-core`, `vectordb-cli`, and `vectordb-mcp`):
        ```bash
        cargo build --release --workspace
        ```
    *   To build a specific package, e.g., `vectordb-cli`:
        ```bash
        cargo build --release --package vectordb-cli
        ```
        The binary will be at `target/release/vectordb-cli`.
    *   Similarly for `vectordb-mcp`:
        ```bash
        cargo build --release --package vectordb-mcp
        ```
        The binary will be at `target/release/vectordb-mcp`.


## Setting Up Embedding Models for `vectordb-cli`

`vectordb-cli` requires ONNX-format embedding models and their tokenizers.

1.  **Generate/Obtain Model Files**:
    *   The `scripts/setup_onnx_model.sh` script can download and prepare a default model:
        ```bash
        bash scripts/setup_onnx_model.sh
        ```
        This typically creates `onnx/model_quantized.onnx` and tokenizer files in `onnx/`.
    *   Alternatively, see the section "Using Different Embedding Models" below for converting other models.

2.  **Configure `vectordb-cli` Model Paths**:
    *   `vectordb-cli` needs to know where the ONNX model and tokenizer files are. This is detailed in the [`crates/vectordb-cli/README.md`](../crates/vectordb-cli/README.md#installation) (via environment variables, config file, or command-line arguments).

## Using GPU Acceleration with ONNX Runtime

`vectordb-core` can leverage GPU acceleration if you have a compatible ONNX Runtime build installed and correctly configured.

1.  **Install GPU-enabled ONNX Runtime**:
    *   When you install ONNX Runtime (as per [../README.md#prerequisites](../README.md#prerequisites)), choose a version that includes support for your GPU (e.g., CUDA for NVIDIA, DirectML for Windows, CoreML/Metal for macOS).
    *   Ensure all necessary drivers (e.g., NVIDIA drivers, CUDA Toolkit, cuDNN) are installed as required by that ONNX Runtime build.

2.  **Set `LD_LIBRARY_PATH` (or equivalent)**:
    *   This environment variable must point to the directory containing the ONNX Runtime shared libraries (including the GPU-specific ones).

3.  **Build `vectordb-core` with GPU features**:
    *   The `ort` crate (a dependency of `vectordb-core`) has Cargo features to enable different execution providers. For example, to enable CUDA, you might need to ensure the `cuda` feature is active for the `ort` dependency in `vectordb-core`'s `Cargo.toml`.
    *   Currently, `vectordb-core`'s `Cargo.toml` includes `ort` with `features = ["download-binaries"]` by default. If you are using a system-installed GPU-enabled ONNX Runtime, you might want to build `vectordb-core` without the `download-binaries` feature for `ort` and instead enable a specific GPU feature like `cuda` if necessary, for example:
        ```toml
        # In vectordb-core/Cargo.toml (example, may need adjustment)
        # ort = { version = "...", default-features = false, features = ["cuda"] }
        ```
    *   Then rebuild: `cargo build --release` (or for the specific package).

If `vectordb-core` is built with the appropriate features and a compatible GPU-enabled ONNX Runtime is found via `LD_LIBRARY_PATH`, it should automatically attempt to use the GPU.

**Managing GPU Memory Usage (e.g., for `vectordb-cli`)**

When indexing large repositories with GPU acceleration, you might encounter Out-of-Memory (OOM) errors. To manage this, you can limit the number of parallel threads used by Rayon (which `vectordb-core` may use internally):
```bash
# Limit Rayon to N worker threads - adjust based on your GPU memory
export RAYON_NUM_THREADS=N
vectordb-cli repo sync # Or other commands
```
Experiment to find the optimal value for your hardware.

## Using Different Embedding Models (e.g., CodeBERT for `vectordb-cli`)

You can configure `vectordb-cli` to use alternative sentence-transformer models compatible with ONNX.

### Available Model Conversion Scripts
The repository includes scripts in the `./scripts/` directory to generate ONNX models from different Sentence Transformers, for example:
- `convert_all_minilm_model.py` (general purpose model, default)
- `convert_st_code_model.py` (code-specific model)
- `codebert.py` (for `microsoft/codebert-base`)

To use them:
1.  Set up a Python virtual environment:
    ```bash
    python -m venv .venv
    source .venv/bin/activate  # On Windows: .venv\\Scripts\\activate
    pip install torch transformers onnx onnxruntime numpy tokenizers optimum
    ```
2.  Run the desired conversion script, e.g.:
    ```bash
    python scripts/codebert.py
    ```
    This will typically create a new directory (e.g., `codebert_onnx/`) containing the ONNX model and tokenizer files.
3.  Deactivate the virtual environment: `deactivate`

### Configuring `vectordb-cli` for the New Model
Update `vectordb-cli`'s configuration (environment variables, config file, or CLI args) to point to the new model's `.onnx` file and tokenizer directory. Refer to [`crates/vectordb-cli/README.md`](../crates/vectordb-cli/README.md#installation) for details.

### Model Comparison & Index Compatibility
(This section can largely remain as it was, but ensure it refers to `vectordb-cli` and the paths are general or relative to the CLI's execution).

**Important:** Different models produce embeddings of different dimensions. Qdrant indexes are tied to a specific dimension. If `vectordb-cli` detects a model dimension mismatch for an existing index, it will likely clear and recreate the index.

[The MiniLM vs CodeBERT table and discussion on index compatibility can be retained here, updated for context if necessary.]
