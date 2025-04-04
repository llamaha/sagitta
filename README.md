# vectordb-cli

A CLI tool for semantic code search and analysis.

## Features

- Semantic code search with ONNX neural network models (default)
- Fast token-based search for larger codebases
- Hybrid search combining semantic and lexical matching
- Code-aware search for functions, types, and more
- Support for multiple languages: Rust, Ruby, and Go
- Cross-platform support (Linux, macOS)

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

### Option 1: Easy Installation Script (Recommended)

```bash
# Download and run the installation script
curl -L https://gitlab.com/amulvany/vectordb-cli/-/raw/main/scripts/install.sh | bash

# Add to your shell configuration (~/.bashrc, ~/.zshrc, etc.)
echo 'source $HOME/.vectordb-cli/env.sh' >> ~/.bashrc

# Reload your shell configuration
source ~/.bashrc
```

### Option 2: Manual Installation

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

The ONNX model files are stored in Git LFS and are required for the application to work properly. The installation script ensures these files are properly downloaded and copied to the right location.

**Important**: Git LFS is a required dependency for this project. Without it, the ONNX models won't be downloaded correctly and the semantic search functionality won't work.

The model files will be placed in one of these locations:
- `./onnx/` (in the cloned repository)
- `$HOME/.vectordb-cli/models/` (when installed via script)

You can also specify custom model paths using environment variables:
```bash
export VECTORDB_ONNX_MODEL=/path/to/your/model.onnx
export VECTORDB_ONNX_TOKENIZER=/path/to/your/tokenizer_directory
```

## Usage

### Indexing Your Code

```bash
# Index a directory
vectordb-cli index ./your/code/directory

# Index with specific file types
vectordb-cli index ./your/code/directory --file-types rs,rb,go

# Use fast model instead of ONNX (for large codebases)
vectordb-cli index ./your/code/directory --fast
```

### Searching

```bash
# Semantic search
vectordb-cli query "how does the error handling work"

# Limit number of results
vectordb-cli query "implement authentication" --limit 5

# Code-aware search
vectordb-cli code-search "database connection"

# Search by code type
vectordb-cli code-search "user authentication" --type function
```

### Configure Model

```bash
# Use ONNX model (default)
vectordb-cli model --onnx

# Specify custom ONNX paths
vectordb-cli model --onnx --onnx-model ./your-model.onnx --onnx-tokenizer ./your-tokenizer

# Use fast model (less accurate but faster)
vectordb-cli model --fast
```

## Uninstallation

```bash
# Run the uninstallation script
bash $HOME/.vectordb-cli/scripts/uninstall.sh
```

Or manually:
```bash
rm -f ~/.local/bin/vectordb-cli
rm -rf ~/.vectordb-cli
rm -rf ~/.local/share/vectordb-cli
```

## License

MIT 