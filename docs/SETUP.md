# VectorDB CLI Setup Guide

This guide provides instructions to build, install, and configure `vectordb-cli`.

## Prerequisites

- **Rust**: Install from [rustup.rs](https://rustup.rs/)
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  source "$HOME/.cargo/env"
  ```

- **Build Dependencies**:
  - **Linux**: `sudo apt-get update && sudo apt-get install build-essential git-lfs libssl-dev pkg-config`
  - **macOS**: `xcode-select --install && brew install git-lfs pkg-config`

- **Qdrant (Vector Database)**: Run via Docker is recommended:
  ```bash
  docker run -d --name qdrant_db -p 6333:6333 -p 6334:6334 \
      -v $(pwd)/qdrant_storage:/qdrant/storage:z \
      qdrant/qdrant:latest
  ```
  *(Note: The `-d` flag runs it in the background. Use `docker logs qdrant_db` to view logs and `docker stop qdrant_db` to stop.)*

## Installation

1.  **Clone the repository**:
    ```bash
    # Replace with the actual repository URL if different
    git clone https://gitlab.com/amulvany/vectordb-cli.git
    cd vectordb-cli
    ```

2.  **Pull ONNX Model Files**: The default embedding model files are stored using Git LFS.
    ```bash
    # Ensure Git LFS is installed (see Build Dependencies above)
    git lfs install --system # Optional: install system-wide if not already
    git lfs pull
    ```
    This downloads the necessary model and tokenizer files into the `onnx/` directory.

3.  **Build (CPU-only)**:
    ```bash
    cargo build --release
    ```
    The built binary will be at `target/release/vectordb-cli`.

4.  **Configure Model Paths**: `vectordb-cli` needs to know where the ONNX model and tokenizer are located. Choose **one** method:

    *   **Environment Variables**: Set these in your shell profile (`~/.bashrc`, `~/.zshrc`, etc.)
        ```bash
        export VECTORDB_ONNX_MODEL="$PWD/onnx/all-minilm-l6-v2.onnx" # Adjust if using a different model
        export VECTORDB_ONNX_TOKENIZER_DIR="$PWD/onnx" # Adjust if using a different model/location
        # Remember to source your profile or restart your shell
        ```

    *   **Configuration File**: Create `~/.config/vectordb-cli/config.toml` with the following content (use absolute paths):
        ```toml
        # Example: ~/.config/vectordb-cli/config.toml
        onnx_model_path = "/absolute/path/to/vectordb-cli/onnx/all-minilm-l6-v2.onnx"
        onnx_tokenizer_path = "/absolute/path/to/vectordb-cli/onnx"
        ```

    *   **Command-Line Arguments**: Specify the paths directly when running commands (this overrides environment variables and config files):
        ```bash
        ./target/release/vectordb-cli --onnx-model="$PWD/onnx/all-minilm-l6-v2.onnx" --onnx-tokenizer-dir="$PWD/onnx" [command...]
        ```

5.  **Optional: Add to PATH**:
    ```bash
    # Make sure the binary path is correct
    sudo ln -s $PWD/target/release/vectordb-cli /usr/local/bin/vectordb-cli
    # Verify with: which vectordb-cli
    ```

## Basic Usage (CLI)

Assuming you have configured model paths via environment variables or config file:

1.  **Index a directory**:
    ```bash
    vectordb-cli index /path/to/your/code
    ```

2.  **Search for code**:
    ```bash
    vectordb-cli query "your search query about the code"
    ```

3.  **Explore other commands**:
    ```bash
    vectordb-cli --help
    vectordb-cli index --help
    vectordb-cli query --help
    # etc.
    ```

## Compilation Options and GPU Acceleration

The default build (`cargo build --release`) uses the CPU for embeddings. For significantly faster performance on large codebases, you can compile with GPU support using Cargo feature flags.

The build script (`build.rs`) handles downloading the necessary ONNX Runtime libraries (including GPU-specific ones) and configuring the linker (`RPATH` / `@executable_path`) so that the `vectordb-cli` binary can find these libraries at runtime without needing `LD_LIBRARY_PATH` or `DYLD_LIBRARY_PATH` to be set manually.

### Available Feature Flags

| Feature Flag | Description                        | Platform | Default | Tested? |
|--------------|------------------------------------|----------|---------|---------|
| `onnx`       | CPU-based ONNX embedding support   | All      | Yes     | Yes (Linux, macOS) |
| `ort/cuda`   | NVIDIA CUDA GPU acceleration       | Linux    | No      | Yes     |
| `ort/coreml` | Apple Core ML acceleration         | macOS    | No      | No      |
| `ort/metal`  | Apple Metal acceleration           | macOS    | No      | No      |

**Note:** Testing has primarily focused on Linux (CPU and CUDA). While ONNX Runtime supports other execution providers (like CoreML and Metal on macOS), they have not been extensively tested with `vectordb-cli`. Users attempting to use these may need to consult the [ONNX Runtime Execution Providers documentation](https://onnxruntime.ai/docs/execution-providers/) and potentially modify the source code as indicated below.

### Building with CUDA (Linux / NVIDIA)

**Prerequisites:**

1.  **NVIDIA GPU:** A CUDA-compatible NVIDIA GPU.
2.  **NVIDIA Driver:** Install the appropriate proprietary NVIDIA driver for your Linux distribution and GPU model. Ensure it's compatible with the required CUDA Toolkit version.
3.  **CUDA Toolkit:** Install the NVIDIA CUDA Toolkit. The specific version required depends on the ONNX Runtime build used by the `ort` crate. Check the [`ort` crate documentation](https://crates.io/crates/ort) or [ONNX Runtime documentation](https://onnxruntime.ai/docs/build/eps.html#cuda) for compatibility. Often, installing it system-wide via your distribution's package manager or NVIDIA's official installers is sufficient.
4.  **cuDNN:** Install the NVIDIA CUDA Deep Neural Network library (cuDNN). Version 9 or later is recommended for compatibility with recent ONNX Runtime versions. Ensure the installed cuDNN version matches the CUDA Toolkit version.
5.  **Build Tools:** Ensure Rust (`rustup`) and C build tools (`build-essential` on Debian/Ubuntu, `base-devel` on Arch, etc.) are installed.

**Build Command:**

```bash
# Ensure prerequisites are met
cargo build --release --features ort/cuda
```

**Running:**

No special flags are needed at runtime. The CUDA provider will be used automatically if the build included `ort/cuda` and a compatible GPU/driver/toolkit is detected.

**Troubleshooting:**

-   **GPU Not Used:** Check build logs for `build.rs` messages. Run with `RUST_LOG="ort=debug" ./target/release/vectordb-cli ...` and look for CUDA initialization logs/errors. Verify driver/toolkit compatibility.
-   **Library Errors:** Ensure `build.rs` copied libraries to `target/release/lib/`. Check build logs.

### Building with Core ML (macOS / Apple Silicon & AMD)

**Prerequisites:**

1.  **macOS:** Recent version.
2.  **Hardware:** Apple Silicon (M1/M2/M3+) or compatible Intel Mac with AMD GPU.
3.  **Xcode Tools:** `xcode-select --install`.

**Build Command:**

```bash
cargo build --release --no-default-features --features ort/coreml
# Or to include default features as well:
cargo build --release --features onnx,ort/coreml 
```

**Running:**

**Important:** Currently, enabling Core ML at runtime requires a **source code modification** before building. You need to explicitly request the `CoreMLExecutionProvider` when initializing `ort` in `crates/vectordb-core/src/embed/provider/onnx.rs` (or wherever the `ort::init()` call resides after the refactor).

```rust
// Example modification in the relevant Rust file:
use ort::execution_providers::{CoreMLExecutionProvider /*, ... */};

// ...
let coreml_provider = CoreMLExecutionProvider::default()
    .with_flag(ort::execution_providers::CoreMLFlags::COREML_ENABLE_ON_SUBGRAPH) // Example flag
    .build();

ort::init()
    .with_name("vectordb-onnx")
    .with_execution_providers([coreml_provider]) // Request CoreML!
    .commit()?;
// ...
```

After modifying the code, rebuild using the `--features ort/coreml` flag.

**Troubleshooting:**

-   **Build Errors:** Ensure Xcode tools are installed.
-   **Runtime Issues:** Verify the code modification was made and the correct build features were used. Check `RUST_LOG="ort=debug"` output.

### Building with Metal (macOS)

**Prerequisites:**

1.  **macOS:** Recent version.
2.  **Hardware:** Apple Silicon or compatible Intel Mac with Metal-supporting GPU.
3.  **Xcode Tools:** `xcode-select --install`.

**Build Command:**

```bash
cargo build --release --features ort/metal
```

**Running:**

Similar to Core ML, enabling Metal might require explicitly requesting the `MetalExecutionProvider` via code modification during `ort::init()` in the library's source code before building. Consult the `ort` crate documentation for specifics.

**Troubleshooting:**

-   Verify build features and necessary code modifications.
-   Check `RUST_LOG="ort=debug"` output.

## Using Different Embedding Models (e.g., CodeBERT)

You can configure `vectordb-cli` to use alternative sentence-transformer models compatible with ONNX, instead of the default `all-MiniLM-L6-v2`. CodeBERT (`microsoft/codebert-base`) is one such example, specifically trained on code.

### Generating the CodeBERT ONNX Model

1.  **Install Python Dependencies:**
    ```bash
    pip install transformers torch onnx tokenizers optimum onnxruntime
    ```

2.  **Run Generation Script:**
    ```bash
    # Ensure the script exists at this path relative to the project root
    python scripts/codebert.py
    ```
    This script downloads `microsoft/codebert-base`, converts it to ONNX, and saves the model (`codebert_model.onnx`) and tokenizer files (`tokenizer.json`, etc.) into a new `codebert_onnx/` directory.

### Configuring `vectordb-cli` for CodeBERT

Once generated, you **must** tell `vectordb-cli` to use the CodeBERT files instead of the default MiniLM ones. Use **one** of the configuration methods described in the [Installation](#installation) section:

*   **Environment Variables:**
    ```bash
    export VECTORDB_ONNX_MODEL="/path/to/your/vectordb-cli/codebert_onnx/codebert_model.onnx"
    export VECTORDB_ONNX_TOKENIZER_DIR="/path/to/your/vectordb-cli/codebert_onnx"
    ```

*   **Configuration File (`~/.config/vectordb-cli/config.toml`):**
    ```toml
    onnx_model_path = "/absolute/path/to/vectordb-cli/codebert_onnx/codebert_model.onnx"
    onnx_tokenizer_path = "/absolute/path/to/vectordb-cli/codebert_onnx"
    ```

*   **Command-Line Arguments:**
    ```bash
    vectordb-cli --onnx-model="./codebert_onnx/codebert_model.onnx" --onnx-tokenizer-dir="./codebert_onnx" [command...]
    ```

### MiniLM vs. CodeBERT Comparison

| Feature             | Default (all-MiniLM-L6-v2)               | CodeBERT (microsoft/codebert-base)           |
| ------------------- | ---------------------------------------- | -------------------------------------------- |
| **Primary Use**     | General semantic search                  | Semantic search focused on source code     |
| **Speed**           | Faster                                   | Slower                                       |
| **Accuracy (General)**| Good all-rounder                         | Potentially less accurate on non-code text |
| **Accuracy (Code)** | Decent                                   | Potentially higher for supported languages |
| **Language Focus**  | Broad (trained on diverse web text)      | Specific (Python, Java, JS, PHP, Ruby, Go) |
| **Dimension**       | 384                                      | 768                                          |
| **Index Size**      | Smaller                                  | Larger (due to higher dimension)           |
| **Memory Usage**    | Lower                                    | Higher                                       |
| **Setup**           | Included (via Git LFS)                   | Requires generation script (`scripts/codebert.py`) |

**Recommendation:** Start with the default MiniLM model. If you primarily work with the languages CodeBERT supports and find MiniLM's code-specific results lacking, try generating and using CodeBERT. Note that the performance difference within this tool's hybrid search may vary.

### Switching Models and Index Compatibility

**Important:** Different models often produce embeddings of different dimensions (e.g., MiniLM=384, CodeBERT=768). The vector index stored by Qdrant is **tied to a specific dimension**.

-   When you run `vectordb-cli index` using a model with a different dimension than the one used to create the existing index for a given codebase/collection, the tool **should automatically detect the mismatch**.
-   It will likely **clear the existing incompatible embeddings and vector index** in Qdrant and create a new index compatible with the new model dimensions before proceeding with indexing.
-   To be safe, you can manually run `vectordb-cli clear --collection <collection_name>` (or similar command, check `--help`) before indexing with a new model to ensure a clean state for that specific index.

Failure to provide valid model and tokenizer paths for the *configured* model will result in errors. 