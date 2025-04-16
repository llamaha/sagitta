#!/bin/bash

# Simple script to test the CLI functionality

set -e

# Build the project
cargo build

# Get the built binary
BINARY="./target/debug/vectordb-cli"

# Test server command help
echo "Testing server command help..."
$BINARY server --help

# Test server command with a small timeout 
echo "Testing server command with timeout..."
timeout 2s $BINARY server -p 50055 || true

echo "Tests completed successfully!" 