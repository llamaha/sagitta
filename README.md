# `vectordb-core`

`vectordb-core` is a library for semantic code search, providing the core functionalities for indexing codebases, generating embeddings, and performing similarity searches. It is designed to be the engine behind tools like `vectordb-cli`.

This repository also contains:
- [`crates/vectordb-cli`](./crates/vectordb-cli/README.md): A command-line interface for `vectordb-core`.
- [`crates/vectordb-mcp`](./crates/vectordb-mcp/README.md): A server component (MCP) for `vectordb-core`.

## Performance

`vectordb-core` is designed for high-performance indexing and search operations, enabling tools like `vectordb-cli` to achieve significant speed. Through careful tuning of parallel processing, GPU utilization (via ONNX Runtime), and embedding model selection, we've focused on achieving substantial speed improvements while maintaining high-quality search results. The library aims to intelligently balance resource usage based on hardware capabilities, making it efficient even on systems with limited GPU memory when used appropriately by a frontend application.

## Prerequisites

To use `vectordb-core` with ONNX-based embedding models, you need to have the ONNX Runtime installed and accessible to the library.

### 1. Install ONNX Runtime

You can download the ONNX Runtime from the official website: [https://onnxruntime.ai/docs/install/](https://onnxruntime.ai/docs/install/)

Please follow the instructions specific to your operating system and preferred installation method (e.g., pre-built binaries, build from source).

### 2. Configure `LD_LIBRARY_PATH` (Linux/macOS)

Once installed, you need to ensure that the system can find the ONNX Runtime shared libraries. You can do this by setting the `LD_LIBRARY_PATH` (on Linux) or `DYLD_LIBRARY_PATH` (on macOS) environment variable.

For example, if you installed ONNX Runtime to `/opt/onnxruntime`:

**Linux:**
```bash
export LD_LIBRARY_PATH=/opt/onnxruntime/lib:$LD_LIBRARY_PATH
```

**macOS:**
```bash
export DYLD_LIBRARY_PATH=/opt/onnxruntime/lib:$DYLD_LIBRARY_PATH
```

You may want to add this line to your shell's configuration file (e.g., `~/.bashrc`, `~/.zshrc`) to make the setting permanent.

**Windows:**
Ensure the path to the ONNX Runtime DLLs (e.g., `onnxruntime.dll`) is included in your system's `PATH` environment variable.

## Getting Started

See the [Setup Guide](./docs/SETUP.md) for instructions on building, running, and generating the ONNX model and tokenizer files required for vectordb-cli. The ONNX model and tokenizer are now generated locally using scripts/setup_onnx_model.sh and are not stored in the repository.

## License

This project is licensed under the MIT License - see the [LICENSE-MIT](./LICENSE-MIT) file for details.
