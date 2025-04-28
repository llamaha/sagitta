# Using `vectordb-core` Programmatically

This guide provides a quickstart for using the `vectordb-core` library within your own Rust projects.

**Note:** As mentioned in the main project [README](../../README.md), `vectordb-core` is currently tightly coupled with `vectordb-cli`. This guide assumes you are using it within that context or potentially in a separate project that mimics the necessary setup (like ONNX Runtime availability).

## Prerequisites

*   **Rust:** Ensure you have a working Rust development environment ([rustup.rs](https://rustup.rs)).
*   **ONNX Runtime:** You need the ONNX Runtime shared libraries (`.so`/`.dylib`/`.dll`) installed or available where your application runs. The library requires these at runtime to load and execute the embedding models. See the [Runtime Dependency: ONNX Runtime](#runtime-dependency-onnx-runtime) section below for details.
*   **Qdrant:** A running Qdrant instance is needed if you intend to use the indexing or querying features that interact with the vector store.

## Adding the Dependency

If `vectordb-core` were published separately, you would add it like this (adjust version and features):

```toml
[dependencies]
# vectordb-core = { version = "0.1.0", features = ["onnx"] } # Example if published
anyhow = "1.0" # For error handling in the example
```

When developing locally within the `vectordb-cli` workspace, you can use a path dependency in your test crate or example binary:

```toml
[dependencies]
vectordb-core = { path = "../vectordb-core", features = ["onnx"] }
anyhow = "1.0"
```

**Required Feature:** You **must** enable the `onnx` feature flag (or potentially `cuda`, `coreml`, `metal` if you need GPU acceleration and have handled the setup correctly) for the embedding functionality to be included.

## Runtime Dependency: ONNX Runtime

**Crucial:** Unlike the main `vectordb-cli` binary (which uses a build script to bundle runtime libraries), using `vectordb-core` as a library dependency requires *you* to ensure the ONNX Runtime shared libraries are discoverable when your application runs.

Options:

1.  **System-wide Installation:** Install ONNX Runtime globally by following the official instructions: [https://onnxruntime.ai/docs/install/](https://onnxruntime.ai/docs/install/). This is often the simplest method if possible.
2.  **Library Path Environment Variable:** Download the appropriate ONNX Runtime release for your platform and place the shared library files (`.so`, `.dylib`, or `.dll`) in a specific directory. Then, set the corresponding environment variable *before* running your application:
    *   **Linux:** `export LD_LIBRARY_PATH=/path/to/your/onnxruntime/lib:$LD_LIBRARY_PATH`
    *   **macOS:** `export DYLD_LIBRARY_PATH=/path/to/your/onnxruntime/lib:$DYLD_LIBRARY_PATH`
    *   **Windows:** Add `/path/to/your/onnxruntime/lib` to your system's `PATH` environment variable.

Failure to provide the ONNX Runtime libraries will result in runtime errors when attempting to load or use the embedding model (e.g., when initializing embedding providers or generating embeddings).

## Supported Languages for Parsing

`vectordb-core` includes language-specific parsers to intelligently chunk code for embedding. As of now, the following language extensions are explicitly supported for syntax-aware chunking:

*   Rust (`rs`)
*   Python (`py`)
*   JavaScript (`js`, `jsx`)
*   TypeScript (`ts`, `tsx`)
*   Go (`go`)
*   Ruby (`rb`)
*   Markdown (`md`)
*   YAML (`yaml`, `yml`)

Files with other extensions will be processed using a fallback plaintext chunking mechanism.

## Core Concepts (Illustrative - Check `lib.rs` for actual API)

*(Note: The exact API structure might differ after the refactor. Refer to the source code and generated docs for canonical information.)*

*   **Configuration:** Loading settings (e.g., paths to models, Qdrant URL) might involve a dedicated config struct loaded from files or environment variables.
*   **Embedding Provider:** A component (e.g., `OnnxProvider`) responsible for loading the ONNX model/tokenizer and generating embeddings from text.
*   **Vector Store Client:** An interface (possibly wrapping a `qdrant_client`) for interacting with the Qdrant database (indexing vectors, searching).
*   **Indexing Logic:** Functions or methods orchestrating the process of reading files, generating embeddings, and storing them in Qdrant.
*   **Querying Logic:** Functions or methods for taking a query, generating its embedding, and searching the vector store.

## Example Snippet (Conceptual)

This conceptual example shows initializing an embedding provider. **Actual usage will depend heavily on the refactored API.**

```rust
// Placeholder: Adapt based on the actual API in vectordb-core
use vectordb_core::embed::provider::EmbeddingProvider; // Fictional path
use vectordb_core::config::CoreConfig; // Fictional path
use std::path::PathBuf;
use anyhow::Result;

fn main() -> Result<()> {
    // Assume config loading logic exists
    let config = CoreConfig::load()?; // Fictional loading

    let model_path = config.onnx_model_path.expect("Model path needed");
    let tokenizer_path = config.onnx_tokenizer_path.expect("Tokenizer path needed");

    // Ensure ONNX Runtime libs are available via LD_LIBRARY_PATH/DYLD_LIBRARY_PATH or system install
    println!("Attempting to initialize ONNX provider...");

    // Fictional initialization - check actual API
    let provider = vectordb_core::embed::provider::OnnxProvider::new(
        PathBuf::from(model_path),
        PathBuf::from(tokenizer_path),
    )?;

    println!("Provider initialized. Getting dimension...");
    let dim = provider.dimension()?; // Fictional method
    println!("Embedding Dimension: {}", dim);

    // Further steps would involve using the provider to get embeddings
    // let embedding = provider.embed("some text")?;

    println!("Core library components accessible.");
    Ok(())
}
```

## Running the Example

1.  Ensure you have configured the necessary paths (e.g., ONNX model/tokenizer) if required by the library's config mechanism.
2.  **Set the library path environment variable** if you haven't installed ONNX Runtime system-wide (replace the example path):
    ```bash
    # Linux
    export LD_LIBRARY_PATH=/path/to/onnxruntime/lib:$LD_LIBRARY_PATH
    
    # macOS
    export DYLD_LIBRARY_PATH=/path/to/onnxruntime/lib:$DYLD_LIBRARY_PATH
    ```
3.  Run your application:
    ```bash
    cargo run
    ```

## API Documentation

For the definitive API reference:

1.  **Generate Docs Locally:** Run `cargo doc --package vectordb-core --open` from the workspace root.
2.  **Published Docs (if available):** Check if the crate is published to [crates.io](https://crates.io/) and find its documentation link there (e.g., on [docs.rs](https://docs.rs)). 