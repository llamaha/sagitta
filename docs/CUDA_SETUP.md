# Linux CUDA Setup for `vectordb-cli`

This document outlines how to build and run `vectordb-cli` with CUDA GPU acceleration on Linux.

## Prerequisites

1.  **NVIDIA GPU:** A CUDA-compatible NVIDIA GPU.
2.  **NVIDIA Driver:** Install the appropriate proprietary NVIDIA driver for your Linux distribution and GPU model.
3.  **CUDA Toolkit:** Install the NVIDIA CUDA Toolkit. The version required depends on the ONNX Runtime build used by the `ort` crate. Check the [`ort` crate documentation](https://crates.io/crates/ort) or ONNX Runtime documentation for compatibility details. Often, installing it system-wide via your distribution's package manager or NVIDIA's official installers is sufficient.
4.  **Rust & Build Tools:** Ensure you have Rust installed via `rustup` and the necessary C build tools (`build-essential` on Debian/Ubuntu).

## Building with CUDA Support

To enable CUDA support, build the project using the `ort/cuda` feature flag:

```bash
git clone https://gitlab.com/amulvany/vectordb-cli.git
cd vectordb-cli
git lfs pull # Ensure model files are downloaded

cargo build --release --features ort/cuda
```

## How it Works (Build Script & RPATH)

`vectordb-cli` uses a build script (`build.rs`) to simplify the setup for using shared ONNX Runtime libraries, especially with CUDA.

1.  **Library Download:** The `ort` crate (when built with `--features ort/cuda`) downloads the appropriate CUDA-enabled ONNX Runtime libraries (e.g., `libonnxruntime.so`, `libonnxruntime_providers_shared.so`, `libonnxruntime_providers_cuda.so`) into a cache directory (usually `~/.cache/ort.pyke.io/`).
2.  **Library Copying:** The `build.rs` script locates this cache directory and copies **all** the necessary `.so` files from the cache into your project's build output directory (`target/release/lib/`).
3.  **RPATH Setting:** The script then sets the `RPATH` on the final `vectordb-cli` executable to `$ORIGIN/lib`. `$ORIGIN` is a special linker token that means "the directory containing the executable".

**Result:** When you run `./target/release/vectordb-cli`, the dynamic linker looks for required libraries first in the directory specified by the RPATH. Since the RPATH points to the `lib` directory right next to the executable, and `build.rs` copied all the necessary ONNX Runtime libraries there, the executable finds them automatically.

**You generally do *not* need to manually set the `LD_LIBRARY_PATH` environment variable.** The build script handles the library placement and linking.

## Running

After building with the `ort/cuda` feature, simply run the `vectordb-cli` executable as usual. If CUDA was initialized successfully by ONNX Runtime, it will automatically use the GPU for embedding generation during `index` and `query` operations.

```bash
# Assuming the binary is in target/release
./target/release/vectordb-cli index /path/to/code --onnx-model ./onnx/model.onnx --onnx-tokenizer-dir ./onnx/tokenizer/

./target/release/vectordb-cli query "my search query" --onnx-model ./onnx/model.onnx --onnx-tokenizer-dir ./onnx/tokenizer/
```

## Troubleshooting

-   **GPU Not Used:** If you run the tool and monitor `nvidia-smi` but see no GPU activity:
    -   **Check Build Feature:** Ensure you built with `--release --features ort/cuda`.
    -   **Check Build Logs:** Look for warnings or errors during the `cargo build` process, especially messages from `build.rs` about finding or copying libraries.
    -   **Check Runtime Logs:** Run the command with `RUST_LOG="ort=debug"` prepended (e.g., `RUST_LOG="ort=debug" ./target/release/vectordb-cli ...`) and look for errors from `ort::execution_providers` related to CUDA initialization or loading libraries.
    -   **CUDA Environment:** Verify your NVIDIA driver and CUDA toolkit installation are correct and compatible with the ONNX Runtime version being used.
-   **Library Not Found Errors:** If you encounter errors like `cannot open shared object file`, ensure the `build.rs` script ran successfully and copied the libraries to `target/release/lib`. Check the build logs for confirmation. 