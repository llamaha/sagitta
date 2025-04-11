# vectordb-cli

A lightweight command-line tool for fast, local search across your codebases and text files using semantic retrieval.

## Features

-   **Semantic Search:** Finds relevant text chunks based on meaning using ONNX models.
-   **Local First:** Indexes and searches files directly on your machine. No data leaves your system.
-   **Simple Indexing:** Recursively indexes specified directories.
-   **Configurable:** Supports custom ONNX embedding models and tokenizers.
-   **Cross-Platform:** Runs on Linux and macOS.

## Prerequisites

-   **Rust:** Required for building the project. Install from [rustup.rs](https://rustup.rs/).
    ```bash
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    ```
-   **Build Tools:** Rust often requires a C linker and build tools.
    -   **Linux (Debian/Ubuntu):**
        ```bash
        sudo apt-get update && sudo apt-get install build-essential
        ```
    -   **macOS:** Install the Xcode Command Line Tools. If you don't have Xcode installed, running the following command in your terminal will prompt you to install them:
        ```bash
        xcode-select --install
        ```
-   **(Optional) Git LFS:** Needed only if you intend to use the default embedding model provided in the repository via Git LFS.
    ```bash
    # Debian/Ubuntu: sudo apt-get install git-lfs
    # macOS: brew install git-lfs
    git lfs install 
    ```

## Installation

1.  **Clone the Repository:**
    ```bash
    git clone https://gitlab.com/amulvany/vectordb-cli.git
    cd vectordb-cli
    ```

2.  **Download Default Model:** The default model (`all-minilm-l6-v2`) is included via Git LFS. To use it:
    ```bash
    # Install Git LFS (if not already installed)
    # Debian/Ubuntu: sudo apt-get install git-lfs
    # macOS: brew install git-lfs
    # Then, install LFS hooks for your user:
    git lfs install
    # Pull the LFS files (downloads the model/tokenizer)
    git lfs pull
    ```
    The default model (`onnx/all-minilm-l6-v2.onnx`) and its tokenizer (`onnx/tokenizer.json`) will be downloaded into the `onnx/` directory.

    **Important:** `vectordb-cli` needs access to these files. It will automatically find them if you run the tool from the repository root, or if the `onnx/` directory is present in the current working directory. If running from elsewhere, you **must** specify the paths using environment variables:
    ```bash
    # Set these in your shell or .bashrc/.zshrc, replacing /path/to/vectordb-cli appropriately
    export VECTORDB_ONNX_MODEL="/path/to/vectordb-cli/onnx/all-minilm-l6-v2.onnx"
    export VECTORDB_ONNX_TOKENIZER="/path/to/vectordb-cli/onnx/tokenizer.json"
    ```
    Failure to configure access to a valid model and tokenizer will result in an error.

3.  **Build:**
    ```bash
    cargo build --release
    ```

4.  **Install Binary:** Copy the compiled binary to a location in your `PATH`.
    ```bash
    # Example:
    cp target/release/vectordb-cli ~/.local/bin/
    ```

To enable GPU acceleration (CUDA on Linux, Core ML/Metal on macOS), you will need to compile with specific features. Please read the relevant documentation *before* attempting to build with GPU support:
- **CUDA (Linux):** See [CUDA Setup](docs/CUDA_SETUP.md). Compile with: `cargo build --release --features ort/cuda`
- **Core ML / Metal (macOS):** See [macOS GPU Setup](docs/MACOS_GPU_SETUP.md). Compile with: `cargo build --release --features ort/coreml`

## Embedding Models

`vectordb-cli` uses ONNX embedding models for semantic search. You can use the default model or provide your own.

### Default Model (all-MiniLM-L6-v2)

-   **Dimension:** 384
-   **Description:** A fast and effective model suitable for general semantic search.
-   **Setup:** If you followed step 2 of the Installation (using `git lfs pull`), the model and tokenizer are downloaded in the `onnx/` directory. Ensure `vectordb-cli` can find them (see Installation Step 2 notes on environment variables if needed).

### Using Other Embedding Models

Details on using alternative models like CodeBERT, including setup and configuration, can be found here: [Using CodeBERT and Other Models](docs/CODEBERT_SETUP.md).

## Usage

By default, `vectordb-cli` stores its database (`db.json`), cache (`cache.json`), and vector index (`hnsw_index.json`) in the standard user local data directory (e.g., `~/.local/share/vectordb-cli` on Linux, `~/Library/Application Support/vectordb-cli` on macOS).

**Working with Multiple Repositories:**

*   **Combined Index:** You can index multiple repositories into a single database. Subsequent queries will search across all of them. This can be done either by providing multiple paths to a single `index` command or by running `index` multiple times targeting the same database.
    ```bash
    # Index two repos into the default database in one command
    vectordb-cli index /path/to/repoA /path/to/repoB

    # Index two repos into the default database separately
    vectordb-cli index /path/to/repoA 
    vectordb-cli index /path/to/repoC
    ```
*   **Isolated Indexes:** To keep indexes for different projects or repositories completely separate, use the global `--db-path` flag to specify a different database file location for each one. The cache and vector index files will be stored alongside the specified `db.json`.
    ```bash
    # Index repoA into its own database
    vectordb-cli --db-path /data/databases/repoA_index.json index /path/to/repoA

    # Index repoB into a different database
    vectordb-cli --db-path /data/databases/repoB_index.json index /path/to/repoB

    # Query a specific isolated database
    vectordb-cli --db-path /data/databases/repoA_index.json query "search term for repo A"
    ```

### 1. Indexing Files

Create or update a search index for one or more directories. This process reads files, splits them into text chunks, generates embeddings for each chunk, and builds the search index. You must configure an ONNX model first (see [Embedding Models](#embedding-models)).

```bash
# Index a single directory using the default MiniLM model
vectordb-cli index /path/to/your/code

# Index multiple directories in one command
vectordb-cli index /path/to/repoA /path/to/repoB ~/another/project

# Index using CodeBERT via environment variables (assuming they are set)
# (Ensure CodeBERT model/tokenizer paths are set in env vars)
vectordb-cli index /path/to/your/code

# Index using CodeBERT via command-line flags and specific file types
vectordb-cli index /path/to/your/code \
  --onnx-model ./codebert_onnx/codebert_model.onnx \
  --onnx-tokenizer ./codebert_onnx/tokenizer \
  --file-types rs,md,py

# Index multiple directories with more threads
vectordb-cli index /path/to/repoA /path/to/repoB -j 8

# Index using a custom database location
vectordb-cli --db-path /data/shared_index.json index /path/to/team/project
```

### 2. Querying Files

Search across all indexed text chunks using semantic search.

```bash
# Basic query - finds relevant chunks
vectordb-cli query "database connection configuration"

# Limit results to 10 chunks
vectordb-cli query "error handling middleware" -l 10

# Filter search by file types (shows chunks only from matching files)
vectordb-cli query "user schema definition" -t sql,prisma

# Query using a custom database location
vectordb-cli --db-path /data/shared_index.json query "deployment script"
```

**Example Output:**

```
Found 3 relevant chunks (0.12 seconds):
---
1. src/db/connection.rs (Lines 55-68) (score: 0.8734)
  // Function to establish database connection
  pub fn connect(config: &DatabaseConfig) -> Result<PgConnection, ConnectionError> {
      let database_url = format!(
          "postgres://{}:{}@{}:{}/{}",
          config.user,
          config.password,
          config.host,
          config.port,
          config.name
      );
      PgConnection::establish(&database_url)
          .map_err(|e| ConnectionError::EstablishmentError(e.to_string()))
  }
---
2. config/production.yaml (Lines 12-18) (score: 0.8105)
  database:
    host: prod-db.example.com
    port: 5432
    user: prod_user
    password: "${PROD_DB_PASSWORD}"
    name: main_prod_db
---
3. tests/integration/db_test.rs (Lines 20-35) (score: 0.7950)
  #[test]
  fn test_database_connection() {
      let config = DatabaseConfig {
          host: "localhost".to_string(),
          port: 5433, // Test DB port
          user: "test_user".to_string(),
          password: "test_password".to_string(),
          name: "test_db".to_string(),
      };
      
      let connection = connect(&config);
      assert!(connection.is_ok(), "Failed to connect to test database");
  }
---
```

### 3. Writing Effective Queries

`vectordb-cli` uses semantic (meaning-based) search. Here are tips for getting the best results:

*   **Be Specific but Natural:** Instead of just keywords like "database config", try a more descriptive query like "database connection configuration for production" or "how to handle async errors in Rust middleware". The semantic search understands the intent.
*   **Include Context:** Add terms related to the language, framework, or feature area. Examples: "python async http request library", "react state management hook example", "kubernetes deployment yaml ingress setup".
*   **Use Code Snippets (Carefully):** You can paste short code snippets directly into the query. The default model (MiniLM) has some code understanding, but models like CodeBERT (if configured) are better suited for this. Keep snippets concise.
*   **Filter by File Type:** Use `-t` or `--file-types` (e.g., `-t py,md`) to narrow down results if you know the type of file you're looking for.
*   **Iterate:** If your first query doesn't yield the desired results, refine it based on the text chunks you see. Add more detail, remove ambiguity, or try different phrasing.

### 4. Database Statistics

Show information about the current database.

```bash
vectordb-cli stats
# Specify db path if not default
vectordb-cli --db-path /data/shared_index.json stats
```

### 5. Clearing the Database

Remove all indexed data (embeddings, cache, vector index).

```bash
vectordb-cli clear
# Specify db path if not default
vectordb-cli --db-path /data/shared_index.json clear
```

### 6. Listing Indexed Directories

List the directories that have been explicitly indexed into the database, along with the timestamp of their last successful indexing.

```bash
# List directories in the default database
vectordb-cli list

# List directories in a custom database
vectordb-cli --db-path /data/shared_index.json list
```

## How it Works

1.  **Indexing:**
    -   Files in the specified directories are scanned.
    -   Supported file types are read as plain text.
    -   Text content is split into chunks (based on paragraphs/double newlines).
    -   An ONNX embedding model generates vector representations for each chunk.
    -   Chunk metadata (file path, lines, text) and embeddings are stored in `db.json` (or the file specified by `--db-path`).
    -   File metadata (hash, timestamp) is stored in `cache.json` to avoid re-processing unchanged files on subsequent runs.
    -   An HNSW (Hierarchical Navigable Small World) index is built from the chunk embeddings and saved to `hnsw_index.json` for fast approximate nearest neighbor search.

2.  **Querying:**
    -   The search query is embedded using the same ONNX model.
    -   **Vector Search:** The HNSW index is used to find text chunks with embeddings semantically similar to the query embedding.
    -   The most relevant chunks are retrieved along with their file path, line numbers, and text, then displayed ranked by similarity score.

## Performance Notes

*   **Indexing:** Indexing large codebases or using a high number of threads (`-j` option) can consume significant RAM and CPU resources, especially during embedding generation and HNSW index construction.
*   **Querying:** Query performance is generally fast due to the HNSW index. Using CodeBERT instead of the default MiniLM model will typically result in slower query times due to the larger model size and higher embedding dimension.

## Known Issues / Future Work

*   The HNSW index is rebuilt entirely on every `index` command if *any* files were re-indexed, which can be slow if the total number of indexed chunks across all repositories is very large. Future versions could explore incremental index updates.
*   Consider adding more sophisticated chunking strategies (e.g., code-aware chunking).
*   Consider adding a mechanism to automatically remove deleted files from the index.

## License

This project is licensed under the [MIT License](LICENSE). (Assuming MIT, please update if different) 