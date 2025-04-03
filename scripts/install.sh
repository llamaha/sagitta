#!/bin/bash
set -e

echo "=== vectordb-cli Installation Script ==="
echo "This script will install vectordb-cli with ONNX model support."

# Check for required tools
check_command() {
    if ! command -v $1 &> /dev/null; then
        echo "Error: $1 is required but not installed."
        if [ "$2" != "" ]; then
            echo "Installation hint: $2"
        fi
        exit 1
    fi
}

check_command cargo "Install Rust from https://rustup.rs/"
check_command curl "Install curl using your system's package manager"
check_command git "Install git using your system's package manager"

# Check if Git LFS is installed
GIT_LFS_INSTALLED=false
if command -v git-lfs &> /dev/null; then
    GIT_LFS_INSTALLED=true
else
    echo "Warning: Git LFS is not installed. ONNX models will not be properly downloaded."
    echo "For best results, install Git LFS first:"
    echo "  - On Debian/Ubuntu: sudo apt-get install git-lfs"
    echo "  - On macOS: brew install git-lfs"
    echo "  - More info: https://git-lfs.github.com/"
    echo ""
    echo "Proceeding with limited installation..."
fi

# Determine OS type
PLATFORM="unknown"
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    PLATFORM="linux"
elif [[ "$OSTYPE" == "darwin"* ]]; then
    PLATFORM="macos"
else
    echo "Warning: Unsupported OS. This script is designed for Linux and macOS."
    echo "The installation may not work as expected."
fi

echo "Detected platform: $PLATFORM"

# Create installation directory
INSTALL_DIR="$HOME/.vectordb-cli"
MODELS_DIR="$INSTALL_DIR/models"
BIN_DIR="$HOME/.local/bin"

mkdir -p "$INSTALL_DIR"
mkdir -p "$MODELS_DIR"
mkdir -p "$BIN_DIR"

# Check if ~/.local/bin is in PATH
if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
    echo "Warning: $HOME/.local/bin is not in your PATH."
    echo "Add the following to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
    echo "    export PATH=\"\$HOME/.local/bin:\$PATH\""
fi

# Download tokenizer files
echo "Downloading tokenizer files to $MODELS_DIR..."
TOKENIZER_URL="https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/tokenizer.json"
CONFIG_URL="https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/config.json"
SPECIAL_TOKENS_URL="https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/special_tokens_map.json"
TOKENIZER_CONFIG_URL="https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/tokenizer_config.json"

# Function to download file with progress bar
download_file() {
    echo "Downloading $1..."
    curl -# -L -o "$2" "$1"
}

# Download tokenizer files
download_file "$TOKENIZER_URL" "$MODELS_DIR/tokenizer.json"
download_file "$CONFIG_URL" "$MODELS_DIR/config.json"
download_file "$SPECIAL_TOKENS_URL" "$MODELS_DIR/special_tokens_map.json"
download_file "$TOKENIZER_CONFIG_URL" "$MODELS_DIR/tokenizer_config.json"

# Set environment variables
ENV_FILE="$INSTALL_DIR/env.sh"

cat > "$ENV_FILE" << EOF
# vectordb-cli environment variables
export VECTORDB_ONNX_MODEL="$MODELS_DIR/all-minilm-l6-v2.onnx"
export VECTORDB_ONNX_TOKENIZER="$MODELS_DIR"
EOF

echo "Created environment file at $ENV_FILE"
echo "Run 'source $ENV_FILE' to set up environment variables."

# Build and install vectordb-cli
echo "Building vectordb-cli from source..."

# Create temporary directory for cloning
TEMP_DIR=$(mktemp -d)
echo "Cloning repository to $TEMP_DIR..."

# Clone the repository with Git LFS if available
if [ "$GIT_LFS_INSTALLED" = true ]; then
    git lfs install
    git clone https://gitlab.com/amulvany/vectordb-cli.git "$TEMP_DIR"
    cd "$TEMP_DIR"
    git lfs pull
else
    git clone https://gitlab.com/amulvany/vectordb-cli.git "$TEMP_DIR"
    cd "$TEMP_DIR"
fi

# Copy the ONNX model from the repository if available
ONNX_MODEL_PATH="$MODELS_DIR/all-minilm-l6-v2.onnx"
REPO_MODEL_PATH="$TEMP_DIR/onnx/all-minilm-l6-v2.onnx"

if [ -f "$REPO_MODEL_PATH" ]; then
    echo "Copying ONNX model from repository..."
    cp "$REPO_MODEL_PATH" "$ONNX_MODEL_PATH"
    echo "ONNX model file installed at $ONNX_MODEL_PATH"
else
    echo "Warning: Could not find ONNX model in the repository."
    if [ -f "$ONNX_MODEL_PATH" ]; then
        echo "Using existing ONNX model file at $ONNX_MODEL_PATH"
    else
        echo "Warning: Creating a placeholder ONNX model file for demonstration."
        echo "In a production environment, you would need a proper ONNX model file."
        echo "This placeholder will not work for actual searches."
        echo "PLACEHOLDER ONNX MODEL" > "$ONNX_MODEL_PATH"
        echo "For real usage, please download and convert a proper ONNX model."
    fi
fi

# Build the project
echo "Building the project..."
cargo build --release

# Install the binary
cp "target/release/vectordb-cli" "$BIN_DIR/"

# Copy uninstall script to installation directory
mkdir -p "$INSTALL_DIR/scripts"
cp "scripts/uninstall.sh" "$INSTALL_DIR/scripts/"
chmod +x "$INSTALL_DIR/scripts/uninstall.sh"

# Clean up
cd - > /dev/null
rm -rf "$TEMP_DIR"

echo "Installation completed!"
echo ""
echo "To use vectordb-cli with ONNX support:"
echo "1. Add this to your shell configuration (~/.bashrc, ~/.zshrc, etc.):"
echo "   source $ENV_FILE"
echo ""
echo "2. Run vectordb-cli commands:"
echo "   vectordb-cli index ./path/to/your/code"
echo "   vectordb-cli query \"your search query\""
echo ""
echo "Enjoy semantic code search with vectordb-cli!" 