# CUDA Setup for vectordb-cli

Using the GPU for embedding generation significantly speeds up the indexing process. This document outlines the steps required to configure your system to use an NVIDIA GPU with `vectordb-cli`.

## Prerequisites

1.  **NVIDIA Driver:** Ensure you have a compatible NVIDIA driver installed for your GPU. You can usually install this through your system's package manager or download it directly from NVIDIA.
2.  **CUDA Toolkit:** Install the NVIDIA CUDA Toolkit. `vectordb-cli` currently relies on components from CUDA 12.x. You can download it from the [NVIDIA CUDA Toolkit website](https://developer.nvidia.com/cuda-downloads) or install it via your package manager (e.g., `sudo apt install cuda-toolkit-12-8`). Make sure the CUDA binaries (like `nvcc`) are in your system's `PATH` and the libraries are in the `LD_LIBRARY_PATH`.

## Installing cuDNN

The ONNX Runtime (which `vectordb-cli` uses for model execution) requires the NVIDIA CUDA Deep Neural Network library (cuDNN). Specifically, it often requires version 9 (`libcudnn.so.9`).

**Installation (Debian/Ubuntu):**

First, ensure you have configured the NVIDIA CUDA package repository for `apt`. Instructions are available on the CUDA Toolkit download page.

Then, install the required cuDNN packages. Note that package names may be specific to your installed CUDA toolkit version (e.g., `-cuda-12` for CUDA 12.x):

```bash
sudo apt update
sudo apt install libcudnn9-cuda-12 libcudnn9-dev-cuda-12 # Adjust '-cuda-12' if using a different major CUDA version
```

**Verification:**

After installation, you should be able to locate the library file:

```bash
find /usr/lib /usr/local -name "libcudnn.so.9" 2>/dev/null
```

This command should output the path to the installed library, typically somewhere within `/usr/lib/` or `/usr/local/cuda-*/lib64/`.

## Environment Variables

Ensure the CUDA library path is included in your `LD_LIBRARY_PATH`. If you installed CUDA to `/usr/local/cuda-12.8`, you would typically set it like this:

```bash
export LD_LIBRARY_PATH=/usr/local/cuda-12.8/lib64:$LD_LIBRARY_PATH
```

You might want to add this line to your shell's configuration file (e.g., `~/.bashrc`, `~/.zshrc`) for persistence.

## Running with GPU

Once the driver, CUDA toolkit, and cuDNN are installed and environment variables are set, `vectordb-cli` should automatically attempt to use the CUDA execution provider. You can enable debug logging to confirm:

```bash
RUST_LOG="ort=debug" vectordb-cli index <your_data_directory>
```

Look for messages indicating the successful registration and use of the `CUDAExecutionProvider`. 