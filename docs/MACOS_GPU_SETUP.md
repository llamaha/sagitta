# macOS GPU Setup (Apple Silicon)

**Note:** GPU acceleration on macOS with Apple Silicon (M1/M2/M3 series chips) has **not yet been explicitly tested** for `vectordb-cli`. The following information is based on general ONNX Runtime capabilities for macOS.

Modern Macs with Apple Silicon chips utilize their integrated GPUs differently than systems with NVIDIA GPUs. They do not use CUDA or cuDNN. Instead, acceleration relies on Apple's native frameworks:

*   **Core ML:** Apple's framework for machine learning.
*   **Metal:** Apple's low-level graphics and compute API.

ONNX Runtime (which `vectordb-cli` uses) includes execution providers for both Core ML and Metal.

## Setup and Usage

1.  **Prerequisites:** Ensure your macOS version is up-to-date. No specific driver installation is usually required beyond standard macOS updates, as Core ML support is built-in.

2.  **Enable `ort` Core ML Feature:** When building or running `vectordb-cli`, you **must** enable the `coreml` feature flag for the `ort` crate. This instructs the build process to download and link the version of the ONNX Runtime library that includes Core ML support.

    Enable the feature via the command line:
    ```bash
    # Build with Core ML support enabled
    cargo build --release --features ort/coreml

    # Run with Core ML support enabled (pass args after '--')
    cargo run --release --features ort/coreml -- index /path/to/your/code
    ```
    Adding `coreml` directly to the `features` array in `Cargo.toml` is generally not recommended unless you *always* want to build with Core ML support, as it increases build dependencies. Using the command-line flag is more flexible.

3.  **Execution Provider Configuration:** The `ort` crate should automatically attempt to use the Core ML execution provider when it's available (i.e., when built with the `ort/coreml` feature). You generally do not need to explicitly configure the `SessionBuilder` in the `vectordb-cli` code for this, although the underlying code does need to be compatible with selecting execution providers (which it should be if it supports CUDA).

    *(Optional Code Example - Illustrative)*: If manual configuration were needed (unlikely for standard use), the Rust code might look something like this (refer to `ort` crate docs for specifics):
    ```rust
    use ort::{execution_providers::CoreMLExecutionProvider, Session, SessionBuilder};

    // ...

    // Enable Core ML execution provider (flags might be needed, e.g., COREML_FLAG_ENABLE_ON_GPU)
    let providers = [CoreMLExecutionProvider::default().build()]; // Check ort docs for flags like `with_flag_enable_on_gpu`
    let session = Session::builder()?
        .with_execution_providers(providers)?
        .commit_from_file("your_model.onnx")?;

    // ... use the session ...
    ```

4.  **Compatibility & Performance:**
    *   Performance gains will vary depending on the model and the specific Apple Silicon chip.
    *   Ensure compatibility between the ONNX model, the `ort` crate version, and your macOS version.
    *   Consult the [ONNX Runtime Execution Providers documentation](https://onnxruntime.ai/docs/execution-providers/) for more details on Core ML flags and capabilities.

## Verification

To verify if the Core ML provider is being used, you can try running with debug logging for the `ort` crate:

```bash
# Example for indexing
RUST_LOG="ort=debug,vectordb_cli=info" cargo run --release --features ort/coreml -- index <your_data_directory>
```

Look for log messages mentioning `CoreMLExecutionProvider` being registered or utilized.

## Current Status

Using the `ort/coreml` feature flag is the correct way to enable potential GPU acceleration on Apple Silicon. While this mechanism *should* work, **explicit testing and performance benchmarking for `vectordb-cli` on macOS with Core ML have not yet been conducted.** If you are a macOS user and attempt this, please report your findings and performance results! 