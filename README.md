# `vectordb-core`

`vectordb-core` is a library for semantic code search, providing the core functionalities for indexing codebases, generating embeddings, and performing similarity searches. It is designed to be the engine behind tools like `vectordb-cli`.

This repository also contains:
- [`crates/vectordb-cli`](./crates/vectordb-cli/README.md): A command-line interface for `vectordb-core`.
- [`crates/vectordb-mcp`](./crates/vectordb-mcp/README.md): A server component (MCP) for `vectordb-core`.

**Note:** This tool is under development and not ready for production use.

## Performance

`vectordb-core` is designed for high-performance indexing and search operations, enabling tools like `vectordb-cli` to achieve significant speed. Through careful tuning of parallel processing, GPU utilization (via ONNX Runtime), and embedding model selection, we've focused on achieving substantial speed improvements while maintaining high-quality search results. The library aims to intelligently balance resource usage based on hardware capabilities, making it efficient even on systems with limited GPU memory when used appropriately by a frontend application.

## Prerequisites

1.  **Rust**: Install from [rustup.rs](https://rustup.rs/).
    ```bash
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    source "$HOME/.cargo/env"
    ```

2.  **ONNX Runtime**: `vectordb-core` uses ONNX Runtime for its embedding models.

    **Download:** Get the pre-built binaries for your OS/Architecture (**GPU version is required for practical use; CPU is only for development or debugging**) from the official **[ONNX Runtime v1.20.0 Release](https://github.com/microsoft/onnxruntime/releases/tag/v1.20.0)**. Find the appropriate archive for your system (e.g., `onnxruntime-linux-x64-gpu-1.20.0.tgz`) under the assets menu. **Do not use the CPU-only version for production or large codebases.**
    **Extract:** Decompress the downloaded archive to a suitable location (e.g., `~/onnxruntime/` or `/opt/onnxruntime/`).
    ```bash
    # Example for Linux
    tar -xzf onnxruntime-linux-x64-1.20.0.tgz -C ~/onnxruntime/
    # This creates a directory like ~/onnxruntime/onnxruntime-linux-x64-1.20.0/
    ```
    **Configure Library Path:** You *must* tell your system where to find these libraries using an environment variable. Find the `lib` subdirectory inside the folder you just extracted.
        **Linux:** Set `LD_LIBRARY_PATH` to point to this `lib` directory. 
        ```bash
        # Example (adjust path and add to ~/.bashrc or ~/.zshrc for persistence):
        export LD_LIBRARY_PATH=~/onnxruntime/onnxruntime-linux-x64-1.20.0/lib:$LD_LIBRARY_PATH
        ```

3.  **Qdrant (Vector Database)**: Start the Qdrant vector store. Running via Docker is recommended:
    ```bash
    docker run -d --name qdrant_db -p 6333:6333 -p 6334:6334 \
        -v $(pwd)/qdrant_storage:/qdrant/storage:z \
        qdrant/qdrant:latest
    ```
    *(Note: The `-d` flag runs it in the background. Use `docker logs qdrant_db` to view logs and `docker stop qdrant_db` to stop.)*

## Setup and Usage

### 1. Clone the Repository
```bash
# Replace with the actual repository URL
git clone https://gitlab.com/amulvany/vectordb-core.git
cd vectordb-core 
```

### 2. Build the Tools

The recommended way to build is to compile the entire workspace, which includes `vectordb-core`, `vectordb-cli`, and `vectordb-mcp`:
```bash
cargo build --release --workspace --features ort/cuda
```
The resulting binaries will be located in the `target/release/` directory (e.g., `target/release/vectordb-cli`, `target/release/vectordb-mcp`).

### 3. Set Up Embedding Models

Tools using `vectordb-core` (like `vectordb-cli`) require ONNX-format embedding models and their tokenizers.

*   **Generate/Obtain Model Files**: The `scripts/` directory contains Python helper scripts to convert models from the Hugging Face Hub to the required ONNX format:
    *   To generate the default model (`all-MiniLM-L6-v2`), use `convert_all_minilm_model.py`. First, set up a Python environment (see section 6 below), then run:
        ```bash
        python scripts/convert_all_minilm_model.py
        ```
        This script typically downloads the model and saves the ONNX model and tokenizer files into an `onnx/` directory (or similar, check the script output).
    *   To generate other models (like a code-specific one), use the corresponding script (e.g., `convert_st_code_model.py`). See section 6 for more details.

*   **Configure Model Paths**: The paths to the ONNX model (`.onnx` file) and tokenizer (`tokenizer.json` directory) need to be specified. This is typically done via the central configuration file (see section 4), although tools like `vectordb-cli` may also allow overriding via environment variables or command-line arguments (refer to specific tool documentation).

### 4. Configuration File

Both `vectordb-cli` and `vectordb-mcp` load settings (like Qdrant URL, repository paths, model paths if not overridden by environment variables or arguments) from a central configuration file. This file is typically located at:

`~/.config/vectordb/config.toml`

You can initialize a default configuration using `vectordb-cli init` (see the `vectordb-cli` README).

**See [docs/configuration.md](./docs/configuration.md) for a full list and documentation of all configuration options.**

For specific guidance on optimizing indexing performance, refer to the [Performance Tuning Guide](./docs/configuration.md#performance-tuning-guide) section in the configuration documentation.

### 4a. Example: Setting ONNX Model and Tokenizer Paths in config.toml

To avoid errors like:

    Error: ONNX model path or tokenizer path not specified. Please provide them via CLI arguments (--onnx-model-path, --onnx-tokenizer-dir) or ensure they are set in the configuration file.

Add the following lines to your `~/.config/vectordb/config.toml` (adjust the paths as needed):

```toml
onnx_model_path = "/absolute/path/to/model.onnx"
onnx_tokenizer_path = "/absolute/path/to/tokenizer.json" # or directory containing tokenizer.json
```

- `onnx_model_path` should point to your ONNX model file (e.g., `model.onnx`).
- `onnx_tokenizer_path` should point to your tokenizer file or directory (e.g., `tokenizer.json` or a directory containing it).

You can also override these via CLI arguments:
- `--onnx-model-path /path/to/model.onnx`
- `--onnx-tokenizer-dir /path/to/tokenizer.json`

### 5. Using GPU Acceleration (Optional but highly recommended)

`vectordb-core` can leverage GPU acceleration if you have a compatible ONNX Runtime build installed and correctly configured.

*   **Install GPU-enabled ONNX Runtime**: Follow the instructions in Prerequisites, ensuring you select a version with GPU support (currently, CUDA on Linux is the primary tested configuration) and install any necessary drivers (NVIDIA drivers, CUDA Toolkit, cuDNN).
*   **Set Library Path**: Ensure `LD_LIBRARY_PATH` (or equivalent like `PATH` on Windows) points to the directory containing the GPU-enabled ONNX Runtime libraries.
*   **(Optional) Build `vectordb-core` with GPU features**: The `ort` crate dependency in `Cargo.toml` has features (like `cuda`). If you encounter issues with the default `download-binaries` feature conflicting with your system install, you might consider modifying `Cargo.toml` to use a specific feature (e.g., `ort = { ..., default-features = false, features = ["cuda"] }`) and rebuilding the workspace (`cargo build --release --workspace --features ort/cuda`).
*   **Manage GPU Memory**: By default this tool is bottlenecked by your available GPU memory.  You might hit GPU Out-of-Memory errors depending on the number of parallel threads that are loading the model into GPU memory. Limit parallel threads using Rayon:
    ```bash
    # Adjust N based on your GPU memory
    export RAYON_NUM_THREADS=N 
    vectordb-cli repo sync # Or other tool commands

### 6. Using Different Embedding Models

`vectordb-core` supports using alternative sentence-transformer models compatible with ONNX.

*   **Available Model Conversion Scripts**: The `./scripts/` directory includes Python scripts (`convert_all_minilm_model.py`, `convert_st_code_model.py`) to generate ONNX models from different Sentence Transformer models available on the Hugging Face Hub.
*   **Running Conversion Scripts**:
    1.  Set up a Python virtual environment and install dependencies:
        ```bash
        python -m venv .venv
        source .venv/bin/activate  # On Windows: .venv\Scripts\activate
        pip install torch transformers onnx onnxruntime numpy tokenizers optimum
        ```
    2.  Run the desired conversion script (e.g., `python scripts/convert_st_code_model.py`). This typically creates a new directory (e.g., `st_code_onnx/`) with the model files.
    3.  Deactivate: `deactivate`.
*   **Configure Model Paths**: Update the central configuration (see section 4) to point to the new model's `.onnx` file and tokenizer directory. Tools may also allow overrides via environment variables or arguments.
*   **Index Compatibility**: Different models produce embeddings of different dimensions. Qdrant indexes are tied to a specific dimension. If the core library (used by tools like `vectordb-cli`) detects a model dimension mismatch for an existing index, it will likely need to clear and recreate the index.

## Model Conversion Scripts

The following scripts in the `./scripts` directory help you download and convert popular Hugging Face models to ONNX format for use with vectordb-core:

| Script Name                   | Model Name / HF Repo                  | Embedding Dimension | Description                                      |
|------------------------------ |---------------------------------------|--------------------|--------------------------------------------------|
| convert_all_minilm_model.py   | sentence-transformers/all-MiniLM-L6-v2| 384                | Fast, small, general-purpose semantic model      |
| convert_st_code_model.py      | (customize in script)                 | varies (e.g. 768)  | For code-specific models, e.g. code-search-net   |
| convert_e5_large_v2_model.py  | intfloat/e5-large-v2                  | 1024               | State-of-the-art, high-quality retrieval model   |

- Each script will output an ONNX model and tokenizer directory.
- Update your `config.toml` to point to the generated files and set the correct `performance.vector_dimension` if needed.
- You can add your own scripts for other models as needed.

## License

This project is licensed under the MIT License - see the [LICENSE-MIT](./LICENSE-MIT) file for details.
