#!/bin/bash
set -e

echo "=== Testing Git Repository Robustness ==="

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test specific edge case modules
echo -e "${YELLOW}Running git edge case tests...${NC}"
cargo test -p sagitta-search git_edge_cases --features cuda -- --nocapture

echo -e "${YELLOW}Running integration tests...${NC}"
cargo test -p sagitta-search integration_tests --features cuda -- --nocapture

echo -e "${YELLOW}Running recovery tests...${NC}"
cargo test -p sagitta-search recovery --features cuda -- --nocapture

# Build all to ensure no compilation errors
echo -e "${YELLOW}Building all crates with cuda feature...${NC}"
cargo build --release --all --features cuda

echo -e "${GREEN}All tests passed!${NC}"