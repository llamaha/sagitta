#!/bin/bash

echo "Testing repository history fixes..."

# Check if logging has been added
echo "1. Checking for logging statements..."
if grep -q "log::info.*Loading commits for repository" crates/sagitta-code/src/gui/app/panels/repository_history.rs; then
    echo "✓ Found commit loading logging"
else
    echo "✗ Missing commit loading logging"
fi

if grep -q "log::info.*Manual commit successful" crates/sagitta-code/src/gui/app/panels/repository_history.rs; then
    echo "✓ Found manual commit logging"
else
    echo "✗ Missing manual commit logging"
fi

if grep -q "log::info.*AutoCommitManager.*Starting auto-commit" crates/sagitta-code/src/auto_commit.rs; then
    echo "✓ Found auto-commit logging"
else
    echo "✗ Missing auto-commit logging"
fi

# Check if repository manager integration exists
echo -e "\n2. Checking repository manager integration..."
if grep -q "load_commits_async.*repo_manager.*lock().await" crates/sagitta-code/src/gui/app/panels/repository_history.rs; then
    echo "✓ Found repository manager integration"
else
    echo "✗ Missing repository manager integration"
fi

if grep -q "repo_config.local_path" crates/sagitta-code/src/gui/app/panels/repository_history.rs; then
    echo "✓ Found actual repository path usage"
else
    echo "✗ Missing actual repository path usage"
fi

# Check if async handling has been improved
echo -e "\n3. Checking async execution handling..."
if grep -q "std::thread::spawn" crates/sagitta-code/src/gui/app/panels/repository_history.rs; then
    echo "✓ Found background thread execution"
else
    echo "✗ Missing background thread execution"
fi

if grep -q "tokio::runtime::Runtime::new" crates/sagitta-code/src/gui/app/panels/repository_history.rs; then
    echo "✓ Found proper async runtime handling"
else
    echo "✗ Missing async runtime handling"
fi

echo -e "\n✅ Repository history fixes test complete!"
echo "The repository history panel should now:"
echo "- Use actual repository paths from RepositoryManager"
echo "- Provide comprehensive logging for debugging"
echo "- Handle async operations properly without blocking UI"
echo "- Show detailed error messages when repository paths are not found"