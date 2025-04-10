# macOS GPU Setup (Apple Silicon)

**Note:** GPU acceleration on macOS with Apple Silicon (M1/M2/M3 series chips) has **not yet been explicitly tested** for `vectordb-cli`. The following information is based on general ONNX Runtime capabilities for macOS.

Modern Macs with Apple Silicon chips utilize their integrated GPUs differently than systems with NVIDIA GPUs. They do not use CUDA or cuDNN. Instead, acceleration relies on Apple's native frameworks:

*   **Core ML:** Apple's framework for machine learning.
*   **Metal:** Apple's low-level graphics and compute API.

ONNX Runtime (which `vectordb-cli` uses) includes execution providers for both Core ML and Metal.

## Potential Setup & Considerations

1.  **ONNX Runtime Build:** For `vectordb-cli` to potentially leverage the GPU on macOS, the underlying ONNX Runtime library it uses must have been compiled with Core ML and/or Metal support enabled. This is typically handled when the `onnxruntime` crate is built.

2.  **Automatic Detection:** Ideally, ONNX Runtime should automatically detect the available hardware and select the appropriate execution provider (Core ML or Metal) if available and deemed beneficial.

3.  **Manual Configuration (If Necessary):** If automatic detection doesn't work or you want to force a specific provider, ONNX Runtime often allows specifying the desired execution provider(s) during session creation. It's unclear if `vectordb-cli` currently exposes this level of configuration. Future versions might add flags to select execution providers.

4.  **Compatibility:** There might be compatibility issues between:
    *   The specific ONNX model used for embeddings.
    *   The version of the `onnxruntime` crate.
    *   The version of macOS and its Core ML/Metal frameworks.

## Verification (Hypothetical)

If GPU acceleration *is* working, enabling debug logs might show messages related to the Core ML or Metal execution providers being registered and used. The command would be similar to the Linux/CUDA case:

```bash
# Note: The RUST_LOG variable might need adjustment based on 
# how Core ML / Metal providers log within ORT.
RUST_LOG="ort=debug" vectordb-cli index <your_data_directory>
```

Look for messages mentioning `CoreMLExecutionProvider` or `MetalExecutionProvider`.

## Current Status

As mentioned, this is largely theoretical for `vectordb-cli` at this point. Functionality and performance on Apple Silicon GPUs have not been verified. If you are a macOS user and attempt this, please report your findings! 