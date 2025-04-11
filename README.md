# vectordb-cli

A lightweight command-line tool for fast, local search across your codebases and text files using both semantic (vector) and lexical (keyword) retrieval.

## Features

-   **Hybrid Search:** Combines deep semantic understanding (via ONNX models) with efficient BM25 lexical matching for relevant results.
-   **Local First:** Indexes and searches files directly on your machine. No data leaves your system.
-   **Simple Indexing:** Recursively indexes specified directories.
-   **Configurable:** Supports custom ONNX embedding models and tokenizers.
-   **Cross-Platform:** Runs on Linux and macOS.

## Prerequisites

-   **Rust:** Required for building the project. Install from [rustup.rs](https://rustup.rs/).
    ```bash
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
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

For GPU acceleration details, see [CUDA Setup](docs/CUDA_SETUP.md) and [macOS GPU Setup](docs/MACOS_GPU_SETUP.md).

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

Create or update a search index for one or more directories. You must configure an ONNX model first (see [Embedding Models](#embedding-models)).

```bash
# Index a single directory using the default MiniLM model
vectordb-cli index /path/to/your/code

# Index multiple directories in one command
vectordb-cli index /path/to/repoA /path/to/repoB ~/another/project

# Index using CodeBERT via environment variables (assuming they are set)
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

Search across all indexed files using hybrid (semantic + lexical) search.

```bash
# Basic query
vectordb-cli query "database connection configuration"

# Limit results to 10
vectordb-cli query "error handling middleware" -l 10

# Perform vector-only search
vectordb-cli query "async function examples" --vector-only

# Adjust hybrid search weights (vector 80%, BM25 20%)
vectordb-cli query "authentication logic" --vector-weight 0.8 --bm25-weight 0.2

# Filter search by file types
vectordb-cli query "user schema definition" -t sql,prisma

# Use fast (keyword-based) snippets instead of semantic ones
vectordb-cli query "data structure serialization" --fast-snippets

# Query using a custom database location
vectordb-cli --db-path /data/shared_index.json query "deployment script"
```

### 3. Writing Effective Queries

`vectordb-cli` combines semantic (meaning-based) and lexical (keyword-based) search. Here are tips for getting the best results:

*   **Be Specific but Natural:** Instead of just keywords like "database config", try a more descriptive query like "database connection configuration for production" or "how to handle async errors in Rust middleware". The semantic search understands the intent.
*   **Include Context:** Add terms related to the language, framework, or feature area. Examples: "python async http request library", "react state management hook example", "kubernetes deployment yaml ingress setup".
*   **Use Code Snippets (Carefully):** You can paste short code snippets directly into the query. The default model (MiniLM) has some code understanding, but models like CodeBERT (if configured) are better suited for this. Keep snippets concise.
*   **Experiment with Weights:** If you find keyword matches are too dominant (or not dominant enough), adjust the `--vector-weight` (default 0.7) and `--bm25-weight` (default 0.3). For pure semantic search, use `--vector-only`.
*   **Filter by File Type:** Use `-t` or `--file-types` (e.g., `-t py,md`) to narrow down results if you know the type of file you're looking for.
*   **Iterate:** If your first query doesn't yield the desired results, refine it based on the snippets you see. Add more detail, remove ambiguity, or try different phrasing.

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
    -   Supported file types are parsed (if tree-sitter parsers are available) or read as plain text.
    -   Text content is split into chunks (currently, often whole files, future work may improve chunking).
    -   An ONNX embedding model generates vector representations for each chunk/file.
    -   Embeddings are stored along with file paths in `db.json` (or the file specified by `--db-path`).
    -   File metadata is stored in `cache.json` to avoid re-processing unchanged files on subsequent runs.
    -   A BM25 index is built in memory for lexical search (based on term frequencies in indexed files).
    -   An HNSW (Hierarchical Navigable Small World) index is built from the embeddings and saved to `hnsw_index.json` for fast approximate nearest neighbor search.

2.  **Querying:**
    -   The search query is embedded using the same ONNX model.
    -   **Vector Search:** The HNSW index is used to find files with embeddings semantically similar to the query embedding.
    -   **BM25 Search:** The BM25 index is used to find files containing the query keywords, scored by relevance (term frequency, inverse document frequency).
    -   **Hybrid Ranking:** Scores from vector search and BM25 search are normalized and combined using configurable weights.
    -   Relevant snippets from the top-ranking files are extracted and displayed.

## Performance Notes

*   **Indexing:** Indexing large codebases or using a high number of threads (`-j` option) can consume significant RAM and CPU resources, especially during embedding generation and HNSW index construction.
*   **Querying:** Query performance is generally fast due to the HNSW index. Using CodeBERT instead of the default MiniLM model will typically result in slower query times due to the larger model size and higher embedding dimension.

## Known Issues / Future Work

*   The HNSW index is rebuilt entirely on every `index` command, which can be slow if the total number of indexed files across all repositories is very large. Future versions could explore incremental index updates.
*   File content chunking during indexing is currently basic (often whole files). More sophisticated chunking could improve snippet relevance.
*   Consider adding a mechanism to automatically remove deleted files from the index.

## License

This project is licensed under the [MIT License](LICENSE). (Assuming MIT, please update if different) 