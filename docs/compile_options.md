# VectorDB-CLI Compilation Options

This document explains all available compilation options for the vectordb-cli project, detailing feature flags and how to compile for different scenarios.

## Available Feature Flags

The vectordb-cli project supports the following feature flags:

| Feature Flag | Description | Default |
|--------------|-------------|---------|
| `onnx` | Enables ONNX embedding model support | Yes |
| `cuda` | Enables NVIDIA CUDA GPU acceleration (Linux) | No |
| `ort/cuda` | Alternative way to enable CUDA acceleration | No |
| `ort/coreml` | Enables Apple Core ML acceleration (macOS) | No |
| `ort/metal` | Enables Apple Metal acceleration (macOS) | No |

## Basic Compilation

To build the basic CLI without any special features:

```bash
cargo build --release
```

This builds the standard vectordb-cli with ONNX support but without GPU acceleration.

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

### GPU Acceleration

```bash
# Linux with NVIDIA GPU
cargo build --release --features ort/cuda

# macOS with Core ML
cargo build --release --features ort/coreml

# macOS with Metal
cargo build --release --features ort/metal
```

## Testing with Different Feature Flags

Testing can also be done with specific feature combinations:

```bash
# Run tests without server features (faster, fewer dependencies)
cargo test
```

## Feature Dependencies

Features have the following dependencies:

- `onnx` (default): Depends on the ort crate with download-binaries feature
- GPU acceleration: Depends on respective GPU toolkit installations

## Performance Considerations

- GPU acceleration can significantly speed up embedding generation, especially for large codebases
- When running on resource-constrained systems, the standard build without GPU acceleration is recommended

## Checking Compiled Features

To verify which features are enabled in your build:

```bash
# Run the CLI with --version flag
./target/release/vectordb-cli --version
```

## Troubleshooting

If you encounter issues with specific features:

- For GPU acceleration, ensure proper drivers and toolkits are installed
- If a feature doesn't appear to be working, try rebuilding with `cargo clean` first

See also:
- [CUDA Setup Guide](./CUDA_SETUP.md) for detailed NVIDIA GPU setup instructions
- [macOS GPU Setup](./MACOS_GPU_SETUP.md) for Apple Silicon and Metal setup