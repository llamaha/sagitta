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
check_command git "Install git using your system's package manager"

# Check for Git LFS
if ! command -v git-lfs &> /dev/null; then
    echo "Error: Git LFS is required but not installed."
    echo "Please install Git LFS first:"
    echo "  - On Debian/Ubuntu: sudo apt-get install git-lfs"
    echo "  - On macOS: brew install git-lfs"
    echo "  - More info: https://git-lfs.github.com/"
    exit 1
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

# Set environment variables
ENV_FILE="$INSTALL_DIR/env.sh"

cat > "$ENV_FILE" << EOF
# vectordb-cli environment variables
export VECTORDB_ONNX_MODEL="$MODELS_DIR/all-minilm-l6-v2.onnx"
export VECTORDB_ONNX_TOKENIZER="$MODELS_DIR"
EOF

echo "Created environment file at $ENV_FILE"
echo "Run 'source $ENV_FILE' to set up environment variables."

# Build and install vectordb-cli with Git LFS
echo "Setting up Git LFS..."
git lfs install

echo "Cloning vectordb-cli repository with LFS support..."
TEMP_DIR=$(mktemp -d)
git clone https://gitlab.com/amulvany/vectordb-cli.git "$TEMP_DIR"
cd "$TEMP_DIR"

echo "Fetching LFS objects (ONNX model files)..."
git lfs pull

echo "Copying ONNX model and tokenizer files..."
mkdir -p "$MODELS_DIR"
cp -v onnx/* "$MODELS_DIR/"

echo "Building vectordb-cli..."
cargo build --release

echo "Installing vectordb-cli binary..."
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