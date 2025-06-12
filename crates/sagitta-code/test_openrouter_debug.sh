#!/bin/bash

# OpenRouter Integration Debug Test Script
# Filters output to show only critical diagnostic information

echo "🔍 OpenRouter Integration Debug Test"
echo "===================================="
echo ""
echo "This script will:"
echo "1. Test basic OpenRouter connectivity"
echo "2. Send a test message that should trigger tool calls"
echo "3. Filter output to show only critical debug info"
echo ""

# Check for API key
if [ -z "$OPENROUTER_API_KEY" ]; then
    echo "❌ OPENROUTER_API_KEY not set!"
    echo "Please run: export OPENROUTER_API_KEY=your_key_here"
    exit 1
fi

echo "✓ API key found (length: ${#OPENROUTER_API_KEY})"
echo ""

# Create a test input file
cat > test_input.txt << 'EOF'
test
Create a new git branch called debug-test
exit
EOF

echo "🚀 Running debug test with filtered output..."
echo "Watch for these key indicators:"
echo "  🔍 DEBUG: API key found"
echo "  🔍 DEBUG: Testing OpenRouter client"
echo "  ❌ HTTP errors (400 Bad Request)"
echo "  🔧 Tool call events"
echo "  ✅ Tool completion events"
echo "  🔍 DEBUG: Chunk counting"
echo ""

# Run the CLI with filtered output
./target/debug/chat_cli < test_input.txt 2>&1 | grep -E "(DEBUG:|ERROR|❌|✅|🔧|HTTP|400|Tool|Stream|Chunk|Event)" | head -50

echo ""
echo "🔍 Test completed. Key things to check:"
echo "1. Did you see 'DEBUG: Testing OpenRouter client'?"
echo "2. Any 400 Bad Request errors?"
echo "3. Did tool calls appear with '🔧 [Tool call'?"
echo "4. Were tools executed with '✅ [Tool completed'?"
echo "5. How many chunks were received?"

# Cleanup
rm -f test_input.txt 