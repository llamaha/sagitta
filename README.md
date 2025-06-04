# `sagitta-search`

`sagitta-search` is a library for semantic code search, providing the core functionalities for indexing codebases, generating embeddings, and performing similarity searches. It is designed to be the engine behind tools like `sagitta-cli`.

This repository also contains:
- [`crates/sagitta-cli`](./crates/sagitta-cli/README.md): A command-line interface for `sagitta-search`.
- [`crates/sagitta-mcp`](./crates/sagitta-mcp/README.md): A server component (MCP) for `sagitta-search`.
- [`crates/git-manager`](./crates/git-manager/README.md): Centralized git operations with branch management and automatic resync capabilities.
- [`crates/sagitta-code`](./crates/sagitta-code/README.md): AI agent with conversation management and repository integration.
- [`crates/sagitta-embed`](./crates/sagitta-embed/README.md): High-performance, thread-safe embedding generation library with ONNX model support.
- [`crates/reasoning-engine`](./crates/reasoning-engine/README.md): Advanced reasoning and orchestration engine for AI workflows.
- [`crates/repo-mapper`](./crates/repo-mapper/README.md): Repository structure analysis and mapping utilities.

**Note:** This tool is under development and not ready for production use.

## Performance

`sagitta-search` is designed for high-performance indexing and search operations, enabling tools like `sagitta-cli` to achieve significant speed. Through careful tuning of parallel processing, GPU utilization (via ONNX Runtime), and embedding model selection, we've focused on achieving substantial speed improvements while maintaining high-quality search results. The library aims to intelligently balance resource usage based on hardware capabilities, making it efficient even on systems with limited GPU memory when used appropriately by a frontend application.

## Prerequisites

1.  **Rust**: Install from [rustup.rs](https://rustup.rs/).
    ```bash
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    source "$HOME/.cargo/env"
    ```

2.  **ONNX Runtime**: `sagitta-search` uses ONNX Runtime for its embedding models.

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
git clone https://gitlab.com/amulvany/sagitta-search.git
cd sagitta-search 
```

### 2. Build the Tools

The recommended way to build is to compile the entire workspace, which includes `sagitta-search`, `sagitta-cli`, and `sagitta-mcp`:

**For GPU acceleration (recommended for production):**
```bash
cargo build --release --workspace --features cuda
```

**For CPU-only builds (development/testing):**
```bash
cargo build --release --workspace
```

The resulting binaries will be located in the `target/release/` directory (e.g., `target/release/sagitta-cli`, `target/release/sagitta-mcp`).

### 3. Set Up Embedding Models

Tools using `sagitta-search` (like `sagitta-cli`) require ONNX-format embedding models and their tokenizers.

*   **Generate/Obtain Model Files**: The `scripts/` directory contains Python helper scripts to convert models from the Hugging Face Hub to the required ONNX format:
    *   To generate the default model (`all-MiniLM-L6-v2`), use `convert_all_minilm_model.py`. First, set up a Python environment (see section 6 below), then run:
        ```bash
        python scripts/convert_all_minilm_model.py
        ```
        This script typically downloads the model and saves the ONNX model and tokenizer files into an `onnx/` directory (or similar, check the script output).
    *   To generate other models (like a code-specific one), use the corresponding script (e.g., `convert_st_code_model.py`). See section 6 for more details.

*   **Configure Model Paths**: The paths to the ONNX model (`.onnx` file) and tokenizer (`tokenizer.json` directory) need to be specified. This is typically done via the central configuration file (see section 4), although tools like `sagitta-cli` may also allow overriding via environment variables or command-line arguments (refer to specific tool documentation).

### 4. Configuration File

All Sagitta tools (`sagitta-cli`, `sagitta-mcp`, and `sagitta-code`) use a unified configuration system:

#### Configuration Files

**Core Configuration:** `~/.config/sagitta/config.toml`
- Contains shared settings for Qdrant, ONNX models, repositories, and performance tuning
- Used by all Sagitta tools for core functionality
- You can initialize a default configuration using `sagitta-cli init` (see the `sagitta-cli` README)

**Tool-Specific Configurations:**
- `sagitta-code`: `~/.config/sagitta/sagitta_code_config.json` for Gemini API, UI preferences, and conversation management

#### Data Storage

Following XDG Base Directory conventions:
- **Configuration**: `~/.config/sagitta/` (settings and preferences)
- **Data**: `~/.local/share/sagitta/` (conversations, logs, repositories)
- **Cache**: `~/.cache/sagitta/` (temporary data, future use)

This unified approach ensures:
- **Single namespace**: All Sagitta-related files are under `sagitta/` directories
- **Proper separation**: Configuration vs. data vs. cache following XDG standards
- **Easy backup**: All important data is in predictable locations
- **Tool consistency**: All tools share core settings while maintaining their specific configurations

#### Migration

If you have existing configurations from previous versions, they will be automatically migrated to the new unified structure on first run.

**See [docs/configuration.md](./docs/configuration.md) for a complete reference of all configuration options and performance tuning guidance.**

### 4a. Example: Setting ONNX Model and Tokenizer Paths in config.toml

To avoid errors like:

    Error: ONNX model path or tokenizer path not specified. Please provide them via CLI arguments (--onnx-model-path, --onnx-tokenizer-dir) or ensure they are set in the configuration file.

Add the following lines to your `~/.config/sagitta/config.toml` (adjust the paths as needed):

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

`sagitta-search` can leverage GPU acceleration if you have a compatible ONNX Runtime build installed and correctly configured.

*   **Install GPU-enabled ONNX Runtime**: Follow the instructions in Prerequisites, ensuring you select a version with GPU support (currently, CUDA on Linux is the primary tested configuration) and install any necessary drivers (NVIDIA drivers, CUDA Toolkit, cuDNN).
*   **Set Library Path**: Ensure `LD_LIBRARY_PATH` (or equivalent like `PATH` on Windows) points to the directory containing the GPU-enabled ONNX Runtime libraries.
*   **(Optional) Build `sagitta-search` with GPU features**: The `ort` crate dependency in `Cargo.toml` has features (like `cuda`). If you encounter issues with the default `download-binaries` feature conflicting with your system install, you might consider modifying `Cargo.toml` to use a specific feature (e.g., `ort = { ..., default-features = false, features = ["cuda"] }`) and rebuilding the workspace (`cargo build --release --workspace --features ort/cuda`).
*   **Manage GPU Memory**: By default this tool is bottlenecked by your available GPU memory.  You might hit GPU Out-of-Memory errors depending on the number of parallel threads that are loading the model into GPU memory. Limit parallel threads using Rayon:
    ```bash
    # Adjust N based on your GPU memory
    export RAYON_NUM_THREADS=N 
    sagitta-cli repo sync # Or other tool commands

### 5a. Execution Provider Support

`sagitta-search` supports **all execution providers** available in ONNX Runtime through its advanced execution provider auto-selection system. This includes hardware acceleration for:

- **NVIDIA CUDA** - GPU acceleration for NVIDIA graphics cards
- **NVIDIA TensorRT** - Optimized inference for NVIDIA GPUs  
- **Microsoft DirectML** - GPU acceleration on Windows
- **Apple CoreML** - Optimized inference on Apple devices
- **AMD ROCm** - GPU acceleration for AMD graphics cards
- **Intel OpenVINO** - Optimized inference for Intel hardware
- **Qualcomm QNN** - Mobile/edge device acceleration
- **And many more** - See the complete list below

#### Automatic Provider Selection

The embedding engine automatically detects available hardware and selects the best execution provider:

```toml
# In your config.toml - the engine will auto-select the best available provider
[performance]
enable_provider_auto_selection = true
enable_hardware_detection = true
```

#### Manual Provider Configuration  

You can also manually specify provider preferences with automatic fallback:

```toml
[performance]
execution_providers = ["cuda", "cpu"]  # Try CUDA first, fallback to CPU
```

#### Complete Provider Information

For the **authoritative and up-to-date list** of all supported execution providers, their requirements, configuration options, and platform availability, see:

**📖 [ONNX Runtime Execution Providers Documentation](https://ort.pyke.io/perf/execution-providers)**

This official documentation provides:
- Complete provider list with platform support
- Hardware requirements and driver dependencies  
- Performance optimization tips
- Configuration examples for each provider
- Troubleshooting guidance

*Note: Provider availability depends on your ONNX Runtime build and system configuration. The sagitta-embed engine will automatically handle provider detection and fallback.*

### 6. Using Different Embedding Models

`sagitta-search` supports using alternative sentence-transformer models compatible with ONNX.

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
*   **Index Compatibility**: Different models produce embeddings of different dimensions. Qdrant indexes are tied to a specific dimension. If the core library (used by tools like `sagitta-cli`) detects a model dimension mismatch for an existing index, it will likely need to clear and recreate the index.

## Model Conversion Scripts

The following scripts in the `./scripts` directory help you download and convert popular Hugging Face models to ONNX format for use with sagitta-search:

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
