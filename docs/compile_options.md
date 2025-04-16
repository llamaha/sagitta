# VectorDB-CLI Compilation Options

This document explains all available compilation options for the vectordb-cli project, detailing feature flags and how to compile for different scenarios.

## Available Feature Flags

The vectordb-cli project supports the following feature flags:

| Feature Flag | Description | Default |
|--------------|-------------|---------|
| `onnx` | Enables ONNX embedding model support | Yes |
| `server` | Enables gRPC server functionality | No |
| `cuda` | Enables NVIDIA CUDA GPU acceleration (Linux) | No |
| `ort/cuda` | Alternative way to enable CUDA acceleration | No |
| `ort/coreml` | Enables Apple Core ML acceleration (macOS) | No |
| `ort/metal` | Enables Apple Metal acceleration (macOS) | No |

## Basic Compilation

To build the basic CLI without any special features:

```bash
cargo build --release
```

This builds the standard vectordb-cli with ONNX support but without server functionality or GPU acceleration.

## Compilation with Server Support

To build with gRPC server functionality:

```bash
cargo build --release --features server
```

This enables the gRPC server that allows other applications to connect to vectordb-cli over the network.

## Compilation with GPU Acceleration

### NVIDIA CUDA (Linux)

```bash
# Option 1
cargo build --release --features ort/cuda

# Option 2 - equivalent
cargo build --release --features cuda
```

These options enable NVIDIA CUDA acceleration for faster embedding generation. Requires CUDA toolkit and compatible GPU drivers.

### Apple Core ML (macOS)

```bash
cargo build --release --features ort/coreml
```

This enables Apple Core ML acceleration on Apple Silicon or compatible Intel Macs.

### Apple Metal (macOS)

```bash
cargo build --release --features ort/metal
```

This enables Apple Metal acceleration for embedding generation on compatible Macs.

## Combined Features

You can combine multiple features as needed:

### Server with GPU Acceleration

```bash
# Linux with NVIDIA GPU
cargo build --release --features ort/cuda,server

# macOS with Core ML
cargo build --release --features ort/coreml,server

# macOS with Metal
cargo build --release --features ort/metal,server
```

## Testing with Different Feature Flags

Testing can also be done with specific feature combinations:

```bash
# Run tests without server features (faster, fewer dependencies)
cargo test

# Run tests including server functionality
cargo test --features server

# Run only ignored tests (many server tests are ignored as they require a running server)
cargo test --features server -- --ignored
```

## Feature Dependencies

Features have the following dependencies:

- `server`: Depends on tonic, prost, tonic-reflection, tower, and vectordb-proto
- `onnx` (default): Depends on the ort crate with download-binaries feature
- GPU acceleration: Depends on respective GPU toolkit installations

## Library vs. CLI

The server functionality is only required if you want to use vectordb-cli as a service accessible over the network. For regular CLI usage, the standard build is sufficient.

## Performance Considerations

- GPU acceleration can significantly speed up embedding generation, especially for large codebases
- Server mode has minimal performance impact but enables network access to the functionality
- When running on resource-constrained systems, the standard build without GPU acceleration is recommended

## Checking Compiled Features

To verify which features are enabled in your build:

```bash
# Run the CLI with --version flag
./target/release/vectordb-cli --version

# For server functionality
./target/release/vectordb-cli server start
# If compiled without the server feature, this will show an error
```

## Troubleshooting

If you encounter issues with specific features:

- For GPU acceleration, ensure proper drivers and toolkits are installed
- For server mode, check that gRPC dependencies are properly resolved
- If a feature doesn't appear to be working, try rebuilding with `cargo clean` first

See also:
- [CUDA Setup Guide](./CUDA_SETUP.md) for detailed NVIDIA GPU setup instructions
- [macOS GPU Setup](./MACOS_GPU_SETUP.md) for Apple Silicon and Metal setup
- [Server Usage Documentation](./server_usage.md) for using the server functionality 