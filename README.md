# vectordb-cli

A CLI tool for semantic code search and analysis.

## Features

- Semantic code search with ONNX neural network models
- Hybrid search combining semantic and lexical matching
- Support for multiple file formats
- Cross-platform support (Linux, macOS)

> **Note:** Repository management features are currently experimental and must be enabled with the `VECTORDB_EXPERIMENTAL_REPO=true` environment variable or by compiling with the `--features experimental_repo` flag.

## Supported File Types

The tool indexes common text-based source files and documentation (e.g., .rs, .go, .py, .js, .ts, .md, .txt, .yaml, .json, etc.). You can restrict indexing to specific file extensions using the `--file-types` argument.

> **Note:** GPU support is planned for a future release to significantly improve embedding generation performance.

## Installation

### Prerequisites

- **Git LFS**: Required for downloading the default ONNX model files if using the provided setup.
  ```bash
  # Debian/Ubuntu
  sudo apt-get install git-lfs
  
  # macOS
  brew install git-lfs
  
  # After installation
  git lfs install
  ```
- **Rust**: Required for building the project
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

### Manual Installation

```bash
# Clone the repository with Git LFS support
git lfs install
git clone https://gitlab.com/amulvany/vectordb-cli.git
cd vectordb-cli
git lfs pull # Downloads the default ONNX model files into ./onnx/

# Build the project
cargo build --release

# Copy the binary to a location in your PATH
cp target/release/vectordb-cli ~/.local/bin/
```

### ONNX Model Setup (Required)

The tool requires an ONNX embedding model and its corresponding tokenizer file to perform semantic search. 

**By default, if you clone the repository using Git LFS, the necessary files (`all-minilm-l12-v2.onnx` and `minilm_tokenizer.json`) will be downloaded into an `onnx/` subdirectory.**

If the tool cannot find these files in the default `./onnx/` location relative to where you run it, you **must** specify their paths using **either** environment variables **or** command-line arguments:

**Option 1: Environment Variables**

Add these to your shell profile (`~/.bashrc`, `~/.zshrc`, etc.):
```bash
# Path to the ONNX model file itself
export VECTORDB_ONNX_MODEL="/path/to/your/model.onnx"
# Path to the tokenizer JSON file
export VECTORDB_ONNX_TOKENIZER="/path/to/your/tokenizer.json"
```

**Option 2: Command-Line Arguments**

Provide the paths when running the `index` command:
```bash
vectordb-cli index ./your/code \
  --onnx-model /path/to/your/model.onnx \
  --onnx-tokenizer /path/to/your/tokenizer.json
```

**Failure to provide valid paths through one of these methods will result in an error.**

## Usage

### Indexing Your Code

```bash
# Index a directory (using default ONNX model location: ./onnx/)
vectordb-cli index ./your/code/directory

# Index with specific file types
vectordb-cli index ./your/code/directory --file-types rs,md

# Index using explicitly specified ONNX model paths
vectordb-cli index ./your/code/directory \
  --onnx-model /custom/path/model.onnx \
  --onnx-tokenizer /custom/path/tokenizer.json
```

The indexing process uses the configured ONNX model for high-quality semantic embeddings. Use `--file-types` to restrict which file extensions are indexed.

### Searching

```bash
# Hybrid semantic + lexical search (default)
vectordb-cli query "how does the error handling work"

# Limit number of results
vectordb-cli query "implement authentication" --limit 5

# Pure vector search (disable lexical matching)
vectordb-cli query "database connection logic" --vector-only

# Adjust hybrid search weights (weights should ideally sum to 1.0)
vectordb-cli query "configuration loading" --vector-weight 0.8 --bm25-weight 0.2
```

### Managing Multiple Repositories

```bash
# Add a repository
vectordb-cli repo add /path/to/repository --name my-repo-name

# Import repositories from YAML file
vectordb-cli repo import-yaml repos.yaml

# Skip existing repositories when importing
vectordb-cli repo import-yaml repos.yaml --skip-existing

# List configured repositories
vectordb-cli repo list

# Sync a repository (index the code)
vectordb-cli repo sync my-repo-name

# Remove a repository
vectordb-cli repo remove my-repo-name

# Set a repository as active
vectordb-cli repo set-active my-repo-name
```

#### Active Repository Concept

When you have multiple repositories configured, one of them is designated as the "active" repository. The active repository is used by default for all commands when you don't explicitly specify a repository using the `--repo` flag. For example:

```bash
# Uses the active repository
vectordb-cli query "how does error handling work"

# Explicitly specifies a repository
vectordb-cli query "how does error handling work" --repo my-other-repo
```

The active repository is:
- Automatically set when you add the first repository
- Changed when you use the `repo set-active` command
- Updated when you remove the currently active repository (the next available one becomes active)

You can see the current active repository at the bottom of the output from the `repo list` command:
```
Active repository: my-repo-name (repo-id-12345)
```

Having different repositories allows you to organize your searches across separate codebases or have multiple configurations for the same codebase (e.g., one for code files and another for documentation).

#### Example YAML for Repository Import

Create a YAML file with multiple repository definitions:

```yaml
repositories:
  - path: /path/to/repo1
    name: my-awesome-project
    file_types:
      - rs
      - go
    # embedding_model: onnx (This is now the only option and not needed)
    
  - path: ./relative/path/to/repo2
    name: another-project
    file_types:
      - rs
      - md
      - yaml
    auto_sync: true
    auto_sync_interval: 3600
```

The YAML file supports the following attributes:
- `path`: Path to the repository (absolute or relative to the YAML file)
- `name`: Optional repository name (defaults to directory name)
- `file_types`: Optional list of file extensions to index
- `auto_sync`: Optional auto-sync setting (true/false)
- `auto_sync_interval`: Optional auto-sync interval in seconds

#### Multiple Configurations for the Same Repository

You can create multiple configurations for the same repository by using different names. This is useful for creating specialized search indexes, such as having separate configurations for code and documentation:

```yaml
repositories:
  - path: /path/to/repo
    name: my-repo-code
    file_types:
      - rs
      - go
      - yaml
    embedding_model: onnx
    
  - path: /path/to/repo
    name: my-repo-docs
    file_types:
      - md
    embedding_model: onnx
```

This creates two separate indexes for the same repository, allowing targeted searches:

```bash
# Search only in code files
vectordb-cli query "implement feature" --repo my-repo-code

# Search only in documentation
vectordb-cli query "how to use feature" --repo my-repo-docs
```

### Understanding Search Types

vectordb-cli now uses hybrid search (ONNX semantic + BM25 lexical) by default via the **`query`** command:

- Best for conceptual, high-level searches and semantic understanding.
- Optimal for questions like "How does error handling work?" or "Where is configuration loaded?"
- Provides context-rich snippets from relevant files.
- Automatically adjusts vector/BM25 weights based on query characteristics, or you can override them.
- Examples:
  ```bash
  vectordb-cli query "how are errors handled in the API layer"
  vectordb-cli query "authentication implementation" --vector-weight 0.8 --bm25-weight 0.2
  ```

### Query Optimization Tips

- **Be specific but conversational**: Describe what you're looking for in natural language.
- **Include context**: Adding context improves semantic search (e.g., "how does error handling work in the API layer" vs just "error handling").
- **Mention languages or frameworks**: Including specific languages helps target relevant files.
- **Adjust weights for different needs**:
  - Increase `--vector-weight` for more conceptual/semantic matches.
  - Increase `--bm25-weight` for more exact keyword matches.
- **Use hybrid search effectively**:
  - Hybrid search (default) combines semantic understanding with lexical matches.
  - `--vector-only` is useful for concept-based searches when exact terms might differ.
  - Weights automatically adjust based on query characteristics, but you can override them.

### Prompt Template

Use the following prompt with your preferred LLM to generate effective queries for vectordb-cli:

```
I need to search a codebase using vectordb-cli. Based on my goal described below, help me craft the most effective search query using the 'query' command.

My goal: [DESCRIBE YOUR PROBLEM OR WHAT YOU'RE LOOKING FOR]

Please generate:
1. A primary search query that best captures my intent.
2. Any additional flags or options (like --limit, --vector-weight, --bm25-weight) that would improve results.

Consider these guidelines:
- Use the 'query' command for concept-based searches about "how" something works (best for semantics).
- For mixed keyword/semantic importance, suggest appropriate vector/BM25 weights.
- For narrow file type searches, include the --file-types parameter.

IMPORTANT: Design the query for vectordb-cli's hybrid search algorithm which:
- Uses both semantic embeddings and BM25 lexical search based on whole file content.
- Dynamically adjusts weights based on query length and structure.
- Performs better with natural language than code syntax in queries.

EXAMPLES:

Example 1 - Conceptual search:
Goal: "I want to understand how the error handling works in the HTTP client"
Recommendation:
- Command: vectordb-cli query "how does error handling work in the HTTP client implementation"
- This uses the default hybrid search to find conceptual matches across the codebase.

Example 2 - Customized weights for mixed search:
Goal: "Find code implementing JWT token validation that uses a specific library"
Recommendation:
- Command: vectordb-cli query "JWT token validation implementation" --vector-weight 0.5 --bm25-weight 0.5
- The balanced weights help find semantic matches while giving importance to exact terms.
```

This template helps you craft queries that take maximum advantage of vectordb-cli's `query` command.

## Database Backup

To backup your vector database:

```bash
# The database is located in the following directory
~/.local/share/vectordb-cli/

# To create a backup, simply copy or archive this directory
cp -r ~/.local/share/vectordb-cli/ ~/vectordb-backup/

# Alternatively, create a compressed backup
tar -czvf vectordb-backup.tar.gz ~/.local/share/vectordb-cli/
```

You can restore a backup by replacing the database directory with your backup copy or extracting the archive.

### Configure Model

```bash
# Use ONNX model (default)
vectordb-cli model --onnx

# Specify custom ONNX paths
vectordb-cli model --onnx --onnx-model ./your-model.onnx --onnx-tokenizer ./your-tokenizer

# Use fast model (less accurate but faster)
vectordb-cli model --fast
```

## License

MIT 