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

### Understanding Search Options

vectordb-cli offers different search modes optimized for various use cases:

#### Regular Query (`query` command)
- **Use Cases**: Better for conceptual, high-level searches where you're exploring ideas or topics rather than specific implementations. For example, "vector embedding concepts" or "algorithms for nearest neighbor search".
- **Ranking Priorities**: Prioritizes semantic similarity and concept matching across documentation, comments, and code.
- **Result Formatting**: Returns broader contextual snippets including comments and surrounding code to provide more context.

#### Code Search (`code-search` command)
- **Use Cases**: Optimized for finding specific implementation details, function definitions, or structural elements in code. Works better for queries like "clap Parser implementation for command arguments."
- **Ranking Priorities**: Prioritizes code structure, symbols, function signatures, and implementation patterns.
- **Result Formatting**: Specifically identifies relevant methods, provides code context, and formats the results to highlight implementation details.

#### Parse Code (`parse-code` command)
- **Use Cases**: Ideal for searching through code structure and understanding how code is organized. Works well for finding specific functions, classes, or types across your codebase.
- **Ranking Priorities**: Focuses on code organization, symbol resolution, and structural relationships.
- **Result Formatting**: Presents results organized by code structure, showing hierarchy and relationships between code elements.

### Configure Model

```bash
# Use ONNX model (default)
vectordb-cli model --onnx

# Specify custom ONNX paths
vectordb-cli model --onnx --onnx-model ./your-model.onnx --onnx-tokenizer ./your-tokenizer

# Use fast model (less accurate but faster)
vectordb-cli model --fast
```

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

## Effective Query Prompts

Use the following prompt with your preferred LLM to generate effective queries for vectordb-cli:

```
I need to search a codebase using vectordb-cli. Based on my goal described below, help me craft the most effective search query.

My goal: [DESCRIBE YOUR PROBLEM OR WHAT YOU'RE LOOKING FOR]

Please generate:
1. A concise semantic search query that focuses on concepts and functionality
2. Suggest whether I should use the regular 'query' command, 'code-search', or 'parse-code' command
3. Any additional flags or options I should include

Remember that:
- Regular 'query' works best for conceptual, high-level searches
- 'code-search' is better for finding specific implementations
- 'parse-code' is ideal for searching through code structure
- Be specific but avoid unnecessary details
- Use natural language rather than code syntax for better semantic matching
```

## License

MIT 