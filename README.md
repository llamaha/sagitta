# vectordb-cli

A CLI tool for semantic code search and analysis.

## Features

- Semantic code search with ONNX neural network models (default)
- Fast token-based search for larger codebases
- Hybrid search combining semantic and lexical matching
- Code-aware search for functions, types, and more
- Support for multiple programming languages and file formats
- Cross-platform support (Linux, macOS)

## Supported File Types

Currently, the following file types are supported with code parsers:
- Rust (rs)
- Ruby (rb)
- Go (go)
- JavaScript (js)
- TypeScript (ts)
- Markdown (md) - with regex-based parsing
- Text (txt) - with basic text analysis
- Configuration files (json, yaml, yml, toml, xml)

When using the `--fast` flag, vectordb-cli will index any non-binary file type at the file level, even if not in the supported list above.

> **Note:** Support for Python, C, and C++ is planned for a future release.
>
> **Note:** GPU support is planned for a future release to significantly improve embedding generation performance.

## Installation

### Prerequisites

- **Git LFS**: Required for downloading ONNX model files
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
git lfs pull

# Build with ONNX support (default)
cargo build --release

# Copy the binary to a location in your PATH
cp target/release/vectordb-cli ~/.local/bin/
```

#### ONNX Model Files

The ONNX model files are stored in Git LFS and are required for the application to work properly.

**Important**: Git LFS is a required dependency for this project. Without it, the ONNX models won't be downloaded correctly and the semantic search functionality won't work.

The model files will be placed in the `./onnx/` directory in the cloned repository.

### Environment Variables (Required)

**You must specify the model paths using environment variables:**

```bash
export VECTORDB_ONNX_MODEL=/path/to/your/model.onnx
export VECTORDB_ONNX_TOKENIZER=/path/to/your/tokenizer_directory
```

These environment variables should be added to your shell profile (~/.bashrc, ~/.zshrc, etc.) to ensure they're always available when using vectordb-cli.

## Usage

### Indexing Your Code

```bash
# Index a directory
vectordb-cli index ./your/code/directory

# Index with specific file types
vectordb-cli index ./your/code/directory --file-types rs,rb,go,md

# Use fast model instead of ONNX (for large codebases)
vectordb-cli index ./your/code/directory --fast
```

The indexing process supports different modes:

1. **Default mode**: Uses the ONNX neural network model for high-quality semantic embeddings. Only indexes the supported file types listed above.

2. **Fast mode**: Uses a token-based model that processes files more quickly but with less semantic accuracy.
   - When using `--fast` without specifying file types, it indexes all non-binary files in the directory
   - Ideal for large codebases or when quick indexing is more important than semantic accuracy

3. **Targeted mode**: Specify exactly which file types to index using the `--file-types` parameter
   - Example: `--file-types rs,go,yaml` to only index Rust, Go, and YAML files
   - Can be combined with `--fast` to use the faster embedding model while restricting file types

### Searching

```bash
# Semantic search
vectordb-cli query "how does the error handling work"

# Limit number of results
vectordb-cli query "implement authentication" --limit 5

# Search in a specific repository
vectordb-cli query "error handling" --repo my-repo-name

# Search across all configured repositories
vectordb-cli query "configuration options" --all-repos

# Code-aware search
vectordb-cli code-search "database connection"

# Search by code type
vectordb-cli code-search "user authentication" --type function

# Parse and search through code structure
vectordb-cli parse-code "function update_user"
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
    embedding_model: onnx
    
  - path: ./relative/path/to/repo2
    name: another-project
    file_types:
      - rs
      - md
      - yaml
    embedding_model: fast
    auto_sync: true
    auto_sync_interval: 3600
```

The YAML file supports the following attributes:
- `path`: Path to the repository (absolute or relative to the YAML file)
- `name`: Optional repository name (defaults to directory name)
- `file_types`: Optional list of file extensions to index
- `embedding_model`: Optional model type ("onnx" or "fast")
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

vectordb-cli offers three specialized search modes:

1. **Regular Query** (`query` command)
   - Best for conceptual, high-level searches and semantic understanding
   - Combines vector similarity with BM25 lexical matching (hybrid search by default)
   - Optimal for questions like "How does error handling work?" or "Where is configuration loaded?"
   - Provides context-rich results with surrounding code and comments
   - Automatically adjusts vector/BM25 weights based on query characteristics
   - Examples:
     ```bash
     vectordb-cli query "how are errors handled in the API layer"
     vectordb-cli query "authentication implementation" --vector-weight 0.8 --bm25-weight 0.2
     ```

2. **Code Search** (`code-search` command)
   - Optimized for finding specific code implementations and patterns
   - Code-aware searching that understands functions, types, classes, and modules
   - Ideal for queries like "parse JSON function" or "database connection implementation"
   - Supports specialized searches with `--type` parameter:
     - `function`: Find function/method implementations
     - `type`: Find class/struct/type definitions
     - `dependency`: Find external dependencies and imports
     - `usage`: Find where certain elements are used
   - Examples:
     ```bash
     vectordb-cli code-search "JWT token validation" --type function
     vectordb-cli code-search "user authentication" --type type
     ```

3. **Parse Code** (`parse-code` command)
   - Analyzes code structure to find specific elements
   - Performs exact matching on function/type names
   - Perfect for finding specific functions, classes, or types by name
   - Examples:
     ```bash
     vectordb-cli parse-code "function update_user"
     vectordb-cli parse-code "class AuthController"
     ```

### Query Optimization Tips

- **Be specific but conversational**: Describe what you're looking for in natural language
- **Include context**: Adding context improves semantic search (e.g., "how does error handling work in the API layer" vs just "error handling")
- **Mention languages or frameworks**: Including specific languages helps target relevant files
- **Adjust weights for different needs**:
  - Increase `--vector-weight` for more conceptual/semantic matches
  - Increase `--bm25-weight` for more exact keyword matches
- **Use hybrid search effectively**:
  - Hybrid search (default) combines semantic understanding with lexical matches
  - `--vector-only` is useful for concept-based searches when exact terms might differ
  - Weights automatically adjust based on query characteristics, but you can override them

### Prompt Template

Use the following prompt with your preferred LLM to generate effective queries for vectordb-cli:

```
I need to search a codebase using vectordb-cli. Based on my goal described below, help me craft the most effective search query.

My goal: [DESCRIBE YOUR PROBLEM OR WHAT YOU'RE LOOKING FOR]

Please generate:
1. A primary search query that best captures my intent
2. The recommended search command to use (query, code-search, or parse-code)
3. Any additional flags or options that would improve results

Consider these guidelines:
- For concept-based searches about "how" something works, use the regular 'query' command (best for semantics)
- For finding specific implementations, use 'code-search' (understands code structure)
- For finding exact function/class/type definitions, use 'parse-code' (structure-based)
- For mixed keyword/semantic importance, suggest appropriate vector/BM25 weights
- For narrow file type searches, include the --file-types parameter

IMPORTANT: Design the query for vectordb-cli's hybrid search algorithm which:
- Uses both semantic embeddings and BM25 lexical search
- Dynamically adjusts weights based on query length and structure
- Performs better with natural language than code syntax in queries
- Can focus on code structure when using "code-search" command
```

This template helps you craft queries that take maximum advantage of vectordb-cli's search capabilities, whether you're looking for concept-level understanding or specific code implementations.

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