#!/bin/bash

echo "Testing auto-commit integration..."

# Check if auto-commit module exists and exports are correct
echo "1. Checking auto_commit module..."
if grep -q "pub mod auto_commit" crates/sagitta-code/src/lib.rs; then
    echo "✓ auto_commit module is exported"
else
    echo "✗ auto_commit module not found in lib.rs"
fi

# Check if AutoCommitManager is used in app initialization
echo -e "\n2. Checking AutoCommitManager initialization..."
if grep -q "AutoCommitManager" crates/sagitta-code/src/gui/app/initialization.rs; then
    echo "✓ AutoCommitManager is imported in initialization"
else
    echo "✗ AutoCommitManager not found in initialization"
fi

# Check if auto-commit config is in types
echo -e "\n3. Checking auto-commit configuration..."
if grep -q "AutoCommitConfig" crates/sagitta-code/src/config/types.rs; then
    echo "✓ AutoCommitConfig type exists"
else
    echo "✗ AutoCommitConfig type not found"
fi

# Check if repository history panel is properly integrated
echo -e "\n4. Checking repository history panel..."
if grep -q "RepositoryHistory" crates/sagitta-code/src/gui/app/panels.rs; then
    echo "✓ RepositoryHistory panel is registered"
else
    echo "✗ RepositoryHistory panel not found"
fi

# Check if history button exists in chat input
echo -e "\n5. Checking history button in UI..."
if grep -q "__OPEN_REPOSITORY_HISTORY__" crates/sagitta-code/src/gui/chat/input.rs; then
    echo "✓ History button is implemented"
else
    echo "✗ History button not found"
fi

echo -e "\n✅ Auto-commit integration test complete!"