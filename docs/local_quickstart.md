# Local Quickstart Guide

This guide provides the minimal steps to get `vectordb-cli` running locally for code search.

## Prerequisites

- **Rust**: Install from [rustup.rs](https://rustup.rs/)
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  source "$HOME/.cargo/env"
  ```

- **Build Dependencies**:
  - **Linux**: `sudo apt-get update && sudo apt-get install build-essential git-lfs libssl-dev pkg-config`
  - **macOS**: `xcode-select --install && brew install git-lfs pkg-config`

- **Qdrant**: Run via Docker:
  ```bash
  docker run -p 6333:6333 -p 6334:6334 \
      -v $(pwd)/qdrant_storage:/qdrant/storage:z \
      qdrant/qdrant:latest
  ```

## Installation

1. **Clone the repository**:
   ```bash
   git clone https://gitlab.com/amulvany/vectordb-cli.git
   cd vectordb-cli
   ```

2. **Build (CPU-only)**:
   ```bash
   cargo build --release
   ```

   The built binary will be at `target/release/vectordb-cli`

3. **Pull the ONNX model files from Git LFS**:
   ```bash
   # Make sure Git LFS is installed
   git lfs install

   # Pull the model files
   git lfs pull
   ```

   This will download the default ONNX model (`all-minilm-l6-v2.onnx`) and tokenizer files in the `onnx/` directory.

4. **Configure the model paths**:

   Either set environment variables:
   ```bash
   export VECTORDB_ONNX_MODEL="$PWD/onnx/all-minilm-l6-v2.onnx"
   export VECTORDB_ONNX_TOKENIZER_DIR="$PWD/onnx"
   ```

   Or specify the paths when running commands:
   ```bash
   vectordb-cli --onnx-model="$PWD/onnx/all-minilm-l6-v2.onnx" --onnx-tokenizer-dir="$PWD/onnx" [commands...]
   ```

   Or create a config file at `~/.config/vectordb-cli/config.toml`:
   ```toml
   onnx_model_path = "/absolute/path/to/vectordb-cli/onnx/all-minilm-l6-v2.onnx"
   onnx_tokenizer_path = "/absolute/path/to/vectordb-cli/onnx"
   ```

5. **Optional: Add to PATH**:
   ```bash
   sudo ln -s $PWD/target/release/vectordb-cli /usr/local/bin
   ```

## Basic Usage

### Simple Indexing & Searching

1. **Index a directory**:
   ```bash
   # If you haven't set environment variables or config file:
   vectordb-cli --onnx-model="$PWD/onnx/all-minilm-l6-v2.onnx" --onnx-tokenizer-dir="$PWD/onnx" simple index /path/to/your/code
   
   # Or if you've set the environment variables or config file:
   vectordb-cli simple index /path/to/your/code
   ```

2. **Search for code**:
   ```bash
   vectordb-cli simple query "how to implement authentication middleware"
   ```

### Repository Management

1. **Add a repository**:
   ```bash
   vectordb-cli repo add --url https://github.com/username/repo.git
   ```

2. **Sync and index repository**:
   ```bash
   vectordb-cli repo use repo
   vectordb-cli repo sync
   ```

3. **Search in repository**:
   ```bash
   vectordb-cli repo query "database connection implementation"
   ```

## GPU Acceleration

For improved performance with GPU acceleration:

- **NVIDIA GPU (Linux)**: See [CUDA Setup Guide](./CUDA_SETUP.md)
- **Apple Silicon/Metal (macOS)**: See [macOS GPU Setup Guide](./MACOS_GPU_SETUP.md)

## Next Steps

- [Complete CLI Usage](../README.md#usage-cli)
- [Server Mode](./server_usage.md)
- [Library Integration](./library_quickstart.md)
- [Compilation Options](./compile_options.md) 