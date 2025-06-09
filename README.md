# Sagitta

Sagitta is a semantic code search and AI development toolkit. This workspace contains multiple crates that work together to provide powerful code analysis, search, and AI-assisted development capabilities.

<!-- Do not update this file unless specifically asked to do so -->
## Crates

### Core Libraries

- **[`sagitta-search`](./crates/sagitta-search/)** - Core semantic search engine with indexing, embedding generation, and vector similarity search
- **[`sagitta-embed`](./crates/sagitta-embed/)** - High-performance embedding generation with ONNX model support and thread-safe pooling
- **[`git-manager`](./crates/git-manager/)** - Git operations with branch management and automatic resync capabilities
- **[`code-parsers`](./crates/code-parsers/)** - Language-specific code parsing and analysis utilities
- **[`repo-mapper`](./crates/repo-mapper/)** - Repository structure analysis and mapping
- **[`reasoning-engine`](./crates/reasoning-engine/)** - AI reasoning and orchestration engine
- **[`terminal-stream`](./crates/terminal-stream/)** - Terminal streaming and interaction utilities

### Applications

- **[`sagitta-cli`](./crates/sagitta-cli/)** - Command-line interface for semantic code search and repository management
- **[`sagitta-mcp`](./crates/sagitta-mcp/)** - Model Context Protocol server for IDE and tool integration
- **[`sagitta-code`](./crates/sagitta-code/)** - AI coding assistant with conversation management and repository integration

**Note:** This toolkit is under development and not ready for production use.

## Performance

`sagitta-search` is designed for high-performance indexing and search operations, enabling tools like `sagitta-cli` to achieve significant speed. Through careful tuning of parallel processing, GPU utilization (via ONNX Runtime), and embedding model selection, we've focused on achieving substantial speed improvements while maintaining high-quality search results. The library aims to intelligently balance resource usage based on hardware capabilities, making it efficient even on systems with limited GPU memory when used appropriately by a frontend application.

## Prerequisites

1.  **Rust**: Install from [rustup.rs](https://rustup.rs/).
    ```bash
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    source "$HOME/.cargo/env"
    ```

2.  **ONNX Runtime**: `sagitta-embed` uses ONNX Runtime for its embedding models.

    **Note:** The crates currently use `ort = "2.0.0-rc.9"` with the `download-binaries` feature enabled by default, so manual ONNX Runtime installation is typically not required. The ONNX Runtime binaries will be automatically downloaded during the build process.
    
    **Manual Installation (Optional):** For specific optimizations or custom builds, you can manually install ONNX Runtime:
    
    **Download:** Get the pre-built binaries for your OS/Architecture from the official **[ONNX Runtime v1.20.0 Release](https://github.com/microsoft/onnxruntime/releases/tag/v1.20.0)**. Find the appropriate archive for your system (e.g., `onnxruntime-linux-x64-gpu-1.20.0.tgz`) under the assets menu.
    
    **Extract:** Decompress the downloaded archive to a suitable location (e.g., `~/onnxruntime/` or `/opt/onnxruntime/`).
    ```bash
    # Example for Linux
    tar -xzf onnxruntime-linux-x64-1.20.0.tgz -C ~/onnxruntime/
    # This creates a directory like ~/onnxruntime/onnxruntime-linux-x64-1.20.0/
    ```
    **Configure Library Path:** Set `LD_LIBRARY_PATH` to point to the `lib` directory:
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
git clone https://gitlab.com/amulvany/sagitta.git
cd sagitta 
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
        **For CPU usage:** Add the `--quantized` flag for better CPU performance:
        ```bash
        python scripts/convert_all_minilm_model.py --quantized
        ```
        This script typically downloads the model and saves the ONNX model and tokenizer files into an `onnx/` directory (or similar, check the script output).
    *   To generate other models (like BGE small), use the corresponding script (e.g., `convert_bge_small_model.py`). See section 6 for more details.

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

`sagitta-embed` can leverage GPU acceleration if you have a compatible ONNX Runtime build installed and correctly configured.

*   **Install GPU-enabled ONNX Runtime**: Follow the instructions in Prerequisites, ensuring you select a version with GPU support (currently, CUDA on Linux is the primary tested configuration) and install any necessary drivers (NVIDIA drivers, CUDA Toolkit, cuDNN).
*   **Set Library Path**: Ensure `LD_LIBRARY_PATH` (or equivalent like `PATH` on Windows) points to the directory containing the GPU-enabled ONNX Runtime libraries.
*   **Build with GPU features**: Build the workspace with CUDA support:
    ```bash
    cargo build --release --workspace --features cuda
    ```
*   **Manage GPU Memory**: By default this tool may be bottlenecked by your available GPU memory. You can control GPU memory usage through the configuration file settings in the `[embedding]` section:
    - `max_sessions`: Controls how many model instances run in parallel (directly affects GPU memory usage)
    - `embedding_batch_size`: Controls batch size per model instance (affects VRAM per model)

### 5a. Using CPU-Only Mode

For CPU-only usage, especially on systems without dedicated GPUs:

*   **Use Quantized Models**: When generating models with the conversion scripts, use the `--quantized` flag for better CPU performance:
    ```bash
    python scripts/convert_all_minilm_model.py --quantized
    python scripts/convert_bge_small_model.py --quantized
    ```
    Quantized models are significantly faster on CPU with minimal quality loss.

*   **Build without CUDA**: Use the standard build command:
    ```bash
    cargo build --release --workspace
    ```

*   **Adjust Configuration**: In your `config.toml`, consider lower values for CPU usage:
    ```toml
    [embedding]
    max_sessions = 2              # Fewer parallel sessions for CPU
    embedding_batch_size = 32     # Smaller batches for CPU
    ```

### 5b. Execution Provider Support

`sagitta-embed` currently supports the following execution providers:

- **CPU** - Standard CPU execution (always available)
- **CUDA** - GPU acceleration for NVIDIA graphics cards (when built with `--features cuda`)

The embedding engine automatically selects the best available provider based on build features and hardware availability. CUDA will be used automatically if:
1. The application was built with `--features cuda`
2. Compatible NVIDIA hardware and drivers are available
3. ONNX Runtime CUDA libraries are properly installed

**Future Provider Support**: Additional execution providers (DirectML, CoreML, TensorRT, etc.) are planned but not yet implemented. The current focus is on reliable CPU and CUDA support.

### 6. Using Different Embedding Models

`sagitta-embed` supports using alternative sentence-transformer models compatible with ONNX.

*   **Available Model Conversion Scripts**: The `./scripts/` directory includes Python scripts to generate ONNX models from different Sentence Transformer models available on the Hugging Face Hub:
    - `convert_all_minilm_model.py` - Converts `sentence-transformers/all-MiniLM-L6-v2` (384 dimensions)
    - `convert_bge_small_model.py` - Converts BGE small model
    
*   **Model Performance Comparison**:
    - **BGE Small**: Generally outperforms MiniLM in search quality and accuracy
    - **MiniLM**: Faster and uses less VRAM, good for frequent indexing of large repositories or systems with limited GPU memory
    - **Recommendation**: Use BGE for best quality, MiniLM for speed and memory efficiency
    
*   **Running Conversion Scripts**:
    1.  Set up a Python virtual environment and install dependencies:
        ```bash
        python -m venv .venv
        source .venv/bin/activate  # On Windows: .venv\Scripts\activate
        pip install torch transformers onnx onnxruntime numpy tokenizers optimum
        ```
    2.  Run the desired conversion script (e.g., `python scripts/convert_all_minilm_model.py`). This typically creates a new directory (e.g., `onnx/`) with the model files.
        
        **For CPU usage, add the `--quantized` flag:**
        ```bash
        python scripts/convert_all_minilm_model.py --quantized
        python scripts/convert_bge_small_model.py --quantized
        ```
    3.  Deactivate: `deactivate`.
*   **Configure Model Paths**: Update the central configuration (see section 4) to point to the new model's `.onnx` file and tokenizer directory. Tools may also allow overrides via environment variables or arguments.
*   **Index Compatibility**: Different models produce embeddings of different dimensions. Qdrant indexes are tied to a specific dimension. If the core library (used by tools like `sagitta-cli`) detects a model dimension mismatch for an existing index, it will likely need to clear and recreate the index.

## Model Conversion Scripts

The following scripts in the `./scripts` directory help you download and convert popular Hugging Face models to ONNX format for use with sagitta-embed:

| Script Name                   | Model Name / HF Repo                  | Embedding Dimension | Description                                      |
|------------------------------ |---------------------------------------|--------------------|--------------------------------------------------|
| convert_all_minilm_model.py   | sentence-transformers/all-MiniLM-L6-v2| 384                | Fast, small, general-purpose semantic model. Good for frequent indexing or limited VRAM. |
| convert_bge_small_model.py    | BAAI/bge-small-en-v1.5               | 384                | Higher quality model that outperforms MiniLM. Recommended for best search accuracy. |

- Each script will output an ONNX model and tokenizer directory.
- **For CPU usage:** Add the `--quantized` flag to any script for optimized CPU performance.
- Update your `config.toml` to point to the generated files and set the correct `performance.vector_dimension` if needed.
- You can add your own scripts for other models as needed.

## License

This project is licensed under the MIT License - see the [LICENSE-MIT](./LICENSE-MIT) file for details.
