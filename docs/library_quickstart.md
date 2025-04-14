# Using `vectordb_lib` Programmatically

This guide provides a quickstart for using the `vectordb_lib` library within your own Rust projects.

## Prerequisites

*   **Rust:** Ensure you have a working Rust development environment ([rustup.rs](https://rustup.rs)).
*   **ONNX Runtime:** You need the ONNX Runtime shared libraries (`.so`/`.dylib`/`.dll`) installed or available on your system. The library requires these at runtime to load and execute the embedding models. See the [Runtime Dependency](#runtime-dependency-onnx-runtime) section below for details.

## Adding the Dependency

Add `vectordb-cli` to your project's `Cargo.toml`. You **must** enable the `onnx` feature flag:

```toml
[dependencies]
vectordb-cli = { version = "1.2.0", features = ["onnx"] } # Replace 1.2.0 with the desired version
anyhow = "1.0" # For error handling in the example
```

(If developing locally against this repository, you can use a path dependency: `vectordb-cli = { path = "../path/to/vectordb-cli", features = ["onnx"] }`)

## Runtime Dependency: ONNX Runtime

**Crucial:** Unlike the `vectordb-cli` binary (which uses a build script to bundle runtime libraries), using `vectordb_lib` as a library requires *you* to ensure the ONNX Runtime shared libraries are available when your application runs.

Options:

1.  **System-wide Installation:** Install ONNX Runtime globally by following the official instructions: [https://onnxruntime.ai/docs/install/](https://onnxruntime.ai/docs/install/).
2.  **Library Path Environment Variable:** Download the appropriate ONNX Runtime release for your platform and place the shared library files (`.so`, `.dylib`, or `.dll`) in a specific directory. Then, set the corresponding environment variable *before* running your application:
    *   **Linux:** `export LD_LIBRARY_PATH=/path/to/your/onnxruntime/lib:$LD_LIBRARY_PATH`
    *   **macOS:** `export DYLD_LIBRARY_PATH=/path/to/your/onnxruntime/lib:$DYLD_LIBRARY_PATH`

Failure to provide the ONNX Runtime libraries will result in runtime errors when attempting to load or use the embedding model (e.g., when calling `EmbeddingHandler::dimension` or `EmbeddingHandler::create_embedding_model`).

## Core Concepts

*   **`AppConfig`:** (`vectordb_lib::AppConfig`) Represents the application configuration, loaded via `vectordb_lib::load_config()` or created manually. It holds paths to ONNX models/tokenizers and repository settings.
*   **`EmbeddingHandler`:** (`vectordb_lib::EmbeddingHandler`) Manages the configuration and instantiation of the ONNX embedding model. You create this using paths (often from `AppConfig`).

## Quickstart Example

This example demonstrates loading the configuration and initializing the `EmbeddingHandler`.

```rust
// src/main.rs
use vectordb_lib::{AppConfig, EmbeddingHandler, load_config, EmbeddingModelType};
use std::path::PathBuf;
use anyhow::Result;

fn main() -> Result<()> {
    println!("--- Testing vectordb_lib ---");

    // 1. Load config (uses default paths if config file doesn't exist)
    //    Ensure your config (~/.config/vectordb-cli/config.toml) or environment
    //    variables point to valid ONNX model/tokenizer paths.
    println!("Loading configuration...");
    let config = load_config()
        .inspect(|c| println!("Config loaded: {:?}", c))
        .inspect_err(|e| eprintln!("Failed to load config: {}", e))?;

    // 2. Get ONNX paths from config
    let model_path = config.onnx_model_path.clone();
    let tokenizer_path = config.onnx_tokenizer_path.clone();

    if model_path.is_none() || tokenizer_path.is_none() {
        eprintln!("Error: ONNX model and tokenizer paths must be set in the configuration.");
        eprintln!("Please ensure `onnx_model_path` and `onnx_tokenizer_path` are set in ~/.config/vectordb-cli/config.toml or via environment variables.");
        anyhow::bail!("Missing ONNX paths in configuration");
    }
    let model_path_buf = PathBuf::from(model_path.unwrap());
    let tokenizer_path_buf = PathBuf::from(tokenizer_path.unwrap());
    println!("Using ONNX model path: {}", model_path_buf.display());
    println!("Using ONNX tokenizer path: {}", tokenizer_path_buf.display());

    // 3. Initialize EmbeddingHandler
    println!("Initializing EmbeddingHandler...");
    let handler = EmbeddingHandler::new(
        EmbeddingModelType::Onnx,
        Some(model_path_buf),
        Some(tokenizer_path_buf),
    )
    .inspect(|h| println!("Handler created: {:?}", h))
    .inspect_err(|e| eprintln!("Failed to create handler: {}", e))?;

    // 4. Get dimension (requires creating the model internally - needs ONNX Runtime libs!)
    println!("Getting embedding dimension...");
    let dim = handler.dimension()
        .inspect(|d| println!("Dimension: {}", d))
        .inspect_err(|e| eprintln!("Failed to get dimension (ensure ONNX Runtime libs are available!): {}", e))?;

    assert!(dim > 0, "Dimension should be positive");

    println!("--- Library test finished successfully ---");
    Ok(())
}
```

## Explanation

1.  **Load Config:** `load_config()` reads the configuration from the default location (`~/.config/vectordb-cli/config.toml`). It requires this file to exist or be creatable, and it *must* contain valid paths for `onnx_model_path` and `onnx_tokenizer_path` for this example to proceed.
2.  **Get Paths:** Extracts the model and tokenizer paths from the loaded configuration.
3.  **Initialize Handler:** Creates an `EmbeddingHandler` instance, providing the model type and the paths.
4.  **Get Dimension:** Calls `handler.dimension()`. This is a key step as it triggers the internal creation of the `EmbeddingModel`, which requires the ONNX Runtime shared libraries to be available.

## Running the Example

1.  Save the example code above as `src/main.rs` in your project.
2.  Ensure you have configured the ONNX model and tokenizer paths in `~/.config/vectordb-cli/config.toml` or via environment variables.
3.  **Set the library path environment variable** (replace the example path with the actual path to your ONNX Runtime `lib` directory):
    ```bash
    # Linux
    export LD_LIBRARY_PATH=/path/to/your/onnxruntime/lib:$LD_LIBRARY_PATH
    
    # macOS
    export DYLD_LIBRARY_PATH=/path/to/your/onnxruntime/lib:$DYLD_LIBRARY_PATH
    ```
4.  Run the application:
    ```bash
    cargo run
    ```

You should see output indicating the config loaded, the handler created, and the embedding dimension printed.

## Next Steps

This example covers basic setup. For more advanced usage, such as performing indexing or querying (which typically involves using a Qdrant client directly alongside the library's components), refer to the full API documentation:

*   **API Documentation:** [https://docs.rs/vectordb-cli](https://docs.rs/vectordb-cli) 