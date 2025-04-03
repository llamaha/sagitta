# vectordb-cli

A CLI tool for semantic code search and analysis.

## Features

- Semantic code search with ONNX neural network models (default)
- Fast token-based search for larger codebases
- Hybrid search combining semantic and lexical matching
- Code-aware search for functions, types, and more
- Cross-platform support (Linux, macOS)

## Installation

### Option 1: Easy Installation Script (Recommended)

```bash
# Download and run the installation script
curl -L https://raw.githubusercontent.com/yourusername/vectordb-cli/main/scripts/install.sh | bash

# Add to your shell configuration (~/.bashrc, ~/.zshrc, etc.)
echo 'source $HOME/.vectordb-cli/env.sh' >> ~/.bashrc

# Reload your shell configuration
source ~/.bashrc
```

### Option 2: Build from Source

```bash
# Clone the repository
git clone https://github.com/yourusername/vectordb-cli.git
cd vectordb-cli

# Build with ONNX support (default)
cargo build --release

# Copy the binary to a location in your PATH
cp target/release/vectordb-cli ~/.local/bin/
```

#### ONNX Model Files

The ONNX model files are stored in Git LFS. If you build from source, make sure to:

1. Install Git LFS: `git lfs install`
2. Clone with LFS support: `git lfs pull`

Or download the model files manually and place them in one of the following locations:
- `./onnx/all-minilm-l6-v2.onnx` (current directory)
- `$HOME/.vectordb-cli/models/all-minilm-l6-v2.onnx` (user's home directory)

You can also specify custom model paths:
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
vectordb-cli index ./your/code/directory --file-types rs,py,js,java

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
~/.vectordb-cli/uninstall.sh
```

Or manually:
```bash
rm -f ~/.local/bin/vectordb-cli
rm -rf ~/.vectordb-cli
rm -rf ~/.local/share/vectordb-cli
```

## License

MIT 