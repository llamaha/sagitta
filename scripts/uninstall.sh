#!/bin/bash
set -e

echo "=== vectordb-cli Uninstallation Script ==="
echo "This will remove vectordb-cli and its data."

# Define installation directories
INSTALL_DIR="$HOME/.vectordb-cli"
BIN_PATH="$HOME/.local/bin/vectordb-cli"
DATA_DIR="$HOME/.local/share/vectordb-cli"

# Confirm before proceeding
read -p "Are you sure you want to uninstall vectordb-cli? [y/N] " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]
then
    echo "Uninstallation cancelled."
    exit 0
fi

# Remove the binary
if [ -f "$BIN_PATH" ]; then
    echo "Removing binary from $BIN_PATH..."
    rm -f "$BIN_PATH"
else
    echo "Binary not found at $BIN_PATH, skipping..."
fi

# Remove installation directory with models
if [ -d "$INSTALL_DIR" ]; then
    echo "Removing installation directory $INSTALL_DIR..."
    rm -rf "$INSTALL_DIR"
else
    echo "Installation directory not found at $INSTALL_DIR, skipping..."
fi

# Remove data directory
if [ -d "$DATA_DIR" ]; then
    echo "Removing data directory $DATA_DIR..."
    rm -rf "$DATA_DIR"
else
    echo "Data directory not found at $DATA_DIR, skipping..."
fi

echo "Uninstallation completed."
echo ""
echo "You may also want to remove these from your shell config files:"
echo "- Any source lines for $INSTALL_DIR/env.sh"
echo "- Any PATH additions for $HOME/.local/bin"
echo ""
echo "Thank you for using vectordb-cli!" 