#!/bin/bash

# Script to test client-server interaction

set -e

# Build the project with server feature
echo "Building project with server feature..."
cargo build --features=server

# Get the built binary
BINARY="./target/debug/vectordb-cli"

# Start a Qdrant container if not already running
if ! docker ps | grep -q qdrant; then
    echo "Starting Qdrant container..."
    docker run -d --name qdrant-test -p 6333:6333 -p 6334:6334 qdrant/qdrant:latest
    
    # Wait for Qdrant to be ready
    echo "Waiting for Qdrant to start..."
    sleep 5
fi

# Start the server in the background
echo "Starting server in the background..."
$BINARY server -p 50051 &
SERVER_PID=$!

# Kill the server when the script exits
trap "kill $SERVER_PID" EXIT

# Wait for the server to start
echo "Waiting for server to start..."
sleep 3

# Test if the server is running
echo "Testing if server is running..."
if ps -p $SERVER_PID > /dev/null; then
    echo "Server is running."
else
    echo "Server failed to start."
    exit 1
fi

# Build and run the client example
echo "Building and running client example..."
cargo run --features=server --example client_example

# Cleanup the Qdrant container
if [[ "$1" == "--cleanup" ]]; then
    echo "Cleaning up Qdrant container..."
    docker stop qdrant-test || true
    docker rm qdrant-test || true
fi

echo "Client-server test completed successfully!" 