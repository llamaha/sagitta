#!/bin/bash
set -e

echo "🔍 Checking for required changelog updates..."

# In GitLab CI, we need to fetch the target branch first
git fetch origin $CI_MERGE_REQUEST_TARGET_BRANCH_NAME:$CI_MERGE_REQUEST_TARGET_BRANCH_NAME
CHANGED_FILES=$(git diff --name-only $CI_MERGE_REQUEST_TARGET_BRANCH_NAME..HEAD)

# Check each crate directory
for crate_dir in $(find . -name "Cargo.toml" -not -path "./target/*" | xargs dirname | sort -u); do
  # Skip workspace root
  if [[ "$crate_dir" == "." ]]; then
    continue
  fi
  
  # Check if this crate has code changes
  if echo "$CHANGED_FILES" | grep -q "^${crate_dir#./}/"; then
    echo "📦 Code changes detected in $crate_dir"
    
    # Check if CHANGELOG.md exists and was updated
    if [[ -f "$crate_dir/CHANGELOG.md" ]]; then
      if echo "$CHANGED_FILES" | grep -q "^${crate_dir#./}/CHANGELOG.md$"; then
        echo "✅ CHANGELOG.md updated in $crate_dir"
      else
        echo "❌ Code changes in $crate_dir but CHANGELOG.md not updated"
        exit 1
      fi
    else
      echo "❌ No CHANGELOG.md found in $crate_dir"
      exit 1
    fi
  fi
done

echo "✅ All changelog checks passed!"
