#!/bin/bash

# Get the directory where the script is located
SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)

# Define the path to the binary relative to the script dir
BINARY_PATH="${SCRIPT_DIR}/target/release/vectordb-mcp"
STDERR_LOG="${SCRIPT_DIR}/mcp_stderr.log"
# Define the likely path to the shared libraries
LIB_PATH="${SCRIPT_DIR}/target/release/lib"

# Clear previous log
> "${STDERR_LOG}"

echo "Wrapper script running..." >> "${STDERR_LOG}"
# Set LD_LIBRARY_PATH
export LD_LIBRARY_PATH="${LIB_PATH}:${LD_LIBRARY_PATH}"
echo "Set LD_LIBRARY_PATH=${LD_LIBRARY_PATH}" >> "${STDERR_LOG}"
echo "Executing: ${BINARY_PATH}" >> "${STDERR_LOG}"

# Execute the binary, redirecting stderr to the log file
# Stdin and Stdout are passed through for MCP communication
"${BINARY_PATH}" 2>> "${STDERR_LOG}"

EXIT_CODE=$?
echo "Binary exited with code: ${EXIT_CODE}" >> "${STDERR_LOG}"
exit ${EXIT_CODE} 