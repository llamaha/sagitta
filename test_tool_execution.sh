#!/bin/bash
# Tool Execution Test Suite for OpenAI-Compatible Providers
# This script verifies that tool execution works correctly without infinite loops

echo "=== Tool Execution Test Suite ==="
echo "Testing OpenAI-compatible provider tool execution..."
echo ""

# Test commands that previously caused infinite loops
TEST_COMMANDS=(
    "Please ping the server and tell me if it responds"
    "Do an ls -l and summarize what you see"
    "List all repositories, then search for 'config' in the first one"
    "Run 'echo hello world' and explain the output"
    "Find all repositories and ping the server"
    "Run the command 'nonexistent_command_xyz' and tell me what happens"
)

# Function to run a single test
run_test() {
    local test_num=$1
    local command="$2"
    
    echo "=== Test $test_num: $command ==="
    echo "Running: RUST_LOG=info ./target/release/test-openai-streaming -p \"$command\" -g"
    echo ""
    
    # Run the test with timeout to prevent hanging
    timeout 120s bash -c "RUST_LOG=info ./target/release/test-openai-streaming -p \"$command\" -g"
    local exit_code=$?
    
    if [ $exit_code -eq 0 ]; then
        echo "✅ Test $test_num PASSED"
    elif [ $exit_code -eq 124 ]; then
        echo "❌ Test $test_num FAILED (timeout - possible infinite loop)"
        return 1
    else
        echo "❌ Test $test_num FAILED (exit code: $exit_code)"
        return 1
    fi
    
    echo ""
    echo "Press Enter to continue to next test (or Ctrl+C to stop)..."
    read
    echo ""
}

# Function to run all tests automatically
run_all_tests() {
    local failed_tests=0
    
    for i in "${!TEST_COMMANDS[@]}"; do
        echo "=== Test $((i+1)): ${TEST_COMMANDS[$i]} ==="
        
        # Run with timeout to prevent hanging
        timeout 120s bash -c "RUST_LOG=warn ./target/release/test-openai-streaming -p \"${TEST_COMMANDS[$i]}\" -g > /dev/null 2>&1"
        local exit_code=$?
        
        if [ $exit_code -eq 0 ]; then
            echo "✅ Test $((i+1)) PASSED"
        elif [ $exit_code -eq 124 ]; then
            echo "❌ Test $((i+1)) FAILED (timeout - possible infinite loop)"
            ((failed_tests++))
        else
            echo "❌ Test $((i+1)) FAILED (exit code: $exit_code)"
            ((failed_tests++))
        fi
    done
    
    echo ""
    echo "=== Test Summary ==="
    local total_tests=${#TEST_COMMANDS[@]}
    local passed_tests=$((total_tests - failed_tests))
    
    echo "Total tests: $total_tests"
    echo "Passed: $passed_tests"
    echo "Failed: $failed_tests"
    
    if [ $failed_tests -eq 0 ]; then
        echo "🎉 All tests passed! Tool execution is working correctly."
        return 0
    else
        echo "⚠️  Some tests failed. Check for infinite loops or other issues."
        return 1
    fi
}

# Check if binary exists
if [ ! -f "./target/release/test-openai-streaming" ]; then
    echo "❌ test-openai-streaming binary not found!"
    echo "Please build first: cargo build --release --features cuda,openai-stream-cli"
    exit 1
fi

# Parse command line arguments
case "${1:-interactive}" in
    "auto")
        echo "Running all tests automatically..."
        echo ""
        run_all_tests
        ;;
    "interactive"|"")
        echo "Running tests interactively..."
        echo ""
        for i in "${!TEST_COMMANDS[@]}"; do
            run_test $((i+1)) "${TEST_COMMANDS[$i]}"
        done
        echo "🎉 All interactive tests completed!"
        ;;
    *)
        echo "Usage: $0 [auto|interactive]"
        echo "  auto:        Run all tests automatically with minimal output"
        echo "  interactive: Run tests one by one with full output (default)"
        exit 1
        ;;
esac