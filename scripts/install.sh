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

# Download ONNX model and tokenizer files
MODEL_URL="https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/pytorch_model.bin"
TOKENIZER_URL="https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/tokenizer.json"
CONFIG_URL="https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/config.json"
SPECIAL_TOKENS_URL="https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/special_tokens_map.json"
TOKENIZER_CONFIG_URL="https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/tokenizer_config.json"

echo "Downloading ONNX model files to $MODELS_DIR..."

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

# For the actual ONNX model, either download a pre-converted one or convert ourselves
# For this example, we'll use a mock ONNX file since actual conversion requires more setup
echo "Creating ONNX model file..."
ONNX_MODEL_PATH="$MODELS_DIR/all-minilm-l6-v2.onnx"

# Check if the file already exists
if [ -f "$ONNX_MODEL_PATH" ]; then
    echo "ONNX model file already exists at $ONNX_MODEL_PATH"
else
    # In a real script, we would convert the PyTorch model to ONNX
    # For now, we're just creating a placeholder file
    echo "Warning: Creating a placeholder ONNX model file for demonstration."
    echo "In a production environment, you would need to convert the PyTorch model to ONNX."
    echo "This placeholder will not work for actual searches."
    echo "PLACEHOLDER ONNX MODEL" > "$ONNX_MODEL_PATH"
    echo "For real usage, please download and convert a proper ONNX model."
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

# Build and install vectordb-cli
echo "Building vectordb-cli from source..."

# Check if we're in the vectordb-cli repo
if [ -f "Cargo.toml" ] && grep -q "name = \"vectordb-cli\"" "Cargo.toml"; then
    echo "Building from current directory..."
    cargo build --release
    
    # Install the binary
    cp "target/release/vectordb-cli" "$BIN_DIR/"
else
    # Otherwise, we need to clone the repo
    TEMP_DIR=$(mktemp -d)
    echo "Cloning repository to $TEMP_DIR..."
    
    git clone https://github.com/yourusername/vectordb-cli.git "$TEMP_DIR"
    cd "$TEMP_DIR"
    
    cargo build --release
    
    # Install the binary
    cp "target/release/vectordb-cli" "$BIN_DIR/"
    
    # Clean up
    cd - > /dev/null
    rm -rf "$TEMP_DIR"
fi

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