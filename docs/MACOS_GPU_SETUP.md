# macOS GPU Setup for `vectordb-cli` (Core ML)

While traditional NVIDIA CUDA is generally not available on modern macOS, ONNX Runtime supports Apple's Core ML framework for hardware acceleration on Apple Silicon (M1/M2/M3+) and potentially some Intel Macs with compatible AMD GPUs.

This document outlines how to build and run `vectordb-cli` using the Core ML execution provider on macOS.

## Prerequisites

1.  **macOS:** A recent version of macOS.
2.  **Compatible Hardware:** Apple Silicon (M1/M2/M3+) is recommended for best performance. Some Intel Macs with AMD GPUs might also support Core ML acceleration.
3.  **Rust & Build Tools:** Ensure you have Rust installed via `rustup` and the Xcode Command Line Tools installed (`xcode-select --install`).

## Building with Core ML Support

To enable Core ML support, you need to build the project with the `ort/coreml` feature flag:

```bash
git clone https://gitlab.com/amulvany/vectordb-cli.git
cd vectordb-cli
git lfs pull # Ensure model files are downloaded

# Build specifically enabling the Core ML feature for the ort crate
cargo build --release --no-default-features --features ort/coreml

# Alternatively, if you want both default features (like onnx) AND coreml:
cargo build --release --features onnx,ort/coreml 
```

**Note:** You might need to experiment with the combination of features (`onnx`, `ort/coreml`) depending on your exact needs and if other `ort` features are used.

## How it Works (Build Script & RPATH)

`vectordb-cli` uses a build script (`build.rs`) to simplify the setup for using shared ONNX Runtime libraries.

1.  **Library Download:** The `ort` crate (when built with `--features ort/coreml`) downloads the appropriate ONNX Runtime libraries for macOS, potentially including specific components for Core ML, into a cache directory (usually `~/.cache/ort.pyke.io/`).
2.  **Library Copying:** The `build.rs` script locates this cache directory and copies **all** the necessary `.dylib` files from the cache into your project's build output directory (`target/release/lib/`).
3.  **RPATH Setting:** The script then sets the `RPATH` on the final `vectordb-cli` executable to `@executable_path/lib`. This tells the executable to look for required libraries in the `lib` directory next to itself.

**Result:** When you run `./target/release/vectordb-cli`, the dynamic linker looks for required libraries first in the directory specified by the RPATH. Since the RPATH points to the `lib` directory right next to the executable, and `build.rs` copied all the necessary ONNX Runtime `.dylib` libraries there, the executable finds them automatically.

**You generally do *not* need to manually set the `DYLD_LIBRARY_PATH` environment variable.** The build script handles the library placement and linking.

## Running

After building with the `ort/coreml` feature, you need to explicitly tell `vectordb-cli` (or rather, the underlying `ort` library) to *use* the Core ML provider. **Currently, `vectordb-cli` does not have a command-line flag or configuration option to select execution providers like Core ML.**

**To enable Core ML, you would need to modify the source code** in `src/vectordb/provider/onnx.rs` where the ONNX Runtime environment is initialized:

```rust
// Inside src/vectordb/provider/onnx.rs -> OnnxEmbeddingProvider::new()

use ort::execution_providers::{/* ..., */ CoreMLExecutionProvider, /* ... */};

// ... other code ...

// Initialize Environment using ort::init()
let coreml_provider = CoreMLExecutionProvider::default() // Or configure specific flags
    .with_flag(ort::execution_providers::CoreMLFlags::COREML_ENABLE_ON_SUBGRAPH)
    .build();

// Specify CoreML when initializing the environment
ort::init()
    .with_name("vectordb-onnx")
    .with_execution_providers([coreml_provider]) // Request CoreML
    .commit()?;

// Session creation remains the same, it uses the global environment
let session = Session::builder()?
    .with_optimization_level(GraphOptimizationLevel::Level1)?
    .commit_from_file(model_path)?;

// ... rest of the function ...
```

After making this code change, rebuild the project with the `ort/coreml` feature enabled.

When you run the commands, ONNX Runtime will attempt to use the Core ML framework for acceleration.

```bash
# Rebuild after code changes
cargo build --release --features onnx,ort/coreml # Or whichever features you need

# Run as usual
./target/release/vectordb-cli index /path/to/code ...
./target/release/vectordb-cli query "search query" ...
```

## Troubleshooting

-   **Build Errors:** Ensure Xcode Command Line Tools are installed.
-   **Runtime Errors / No Speedup:**
    -   Verify you built with the `ort/coreml` feature.
    -   Confirm you modified the code in `src/vectordb/provider/onnx.rs` to request the `CoreMLExecutionProvider`.
    -   Run with `RUST_LOG="ort=debug"` to see logs from ONNX Runtime about provider registration and potential fallbacks.
    -   Ensure your macOS version and hardware support Core ML acceleration for the model operations. 