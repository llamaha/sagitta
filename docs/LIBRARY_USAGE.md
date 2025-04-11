# Library Usage (`vectordb_lib`)

This crate can also be used as a library in other Rust projects.

**1. Add Dependency:**

Add `vectordb_lib` to your `Cargo.toml`:

```toml
[dependencies]
vectordb_lib = "{version}" # Replace {version} with the desired version from crates.io
```

**2. Basic Usage:**

The core struct is `VectorDB`. You configure it using `VectorDBConfig`, providing paths to your database file and the required ONNX model and tokenizer files.

```rust
use vectordb_lib::{VectorDB, VectorDBConfig, Result};
use std::path::PathBuf;

fn main() -> Result<()> {
    let config = VectorDBConfig {
        db_path: "my_database.json".to_string(),
        onnx_model_path: PathBuf::from("path/to/model.onnx"),
        onnx_tokenizer_path: PathBuf::from("path/to/tokenizer.json"),
    };

    let mut db = VectorDB::new(config)?;

    // Index a directory
    db.index_directory("/path/to/your/code", &[])?;

    // Perform a search
    let query = "function to parse user input";
    let results = db.search(query, 10, None)?;

    for result in results {
        println!("Found: {} (Score: {:.4})", result.file_path, result.score);
    }

    Ok(())\n```

**3. Example:**

See the `examples/basic_usage.rs` file in the repository for a runnable example that sets up a temporary directory and uses placeholder model paths.

**4. Documentation:**

Full API documentation can be found on [docs.rs/vectordb-lib](https://docs.rs/vectordb-lib) (once published). 