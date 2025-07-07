#!/bin/bash
set -e

echo "🔍 Checking for required version bumps..."

# In GitLab CI, we need to fetch the target branch first
git fetch origin $CI_MERGE_REQUEST_TARGET_BRANCH_NAME:$CI_MERGE_REQUEST_TARGET_BRANCH_NAME
CHANGED_FILES=$(git diff --name-only $CI_MERGE_REQUEST_TARGET_BRANCH_NAME..HEAD)

# Check each crate directory
for crate_dir in $(find . -name "Cargo.toml" -not -path "./target/*" | xargs dirname | sort -u); do
  # Skip workspace root if it exists
  if [[ "$crate_dir" == "." ]]; then
    continue
  fi
  
  # Check if this crate has code changes
  if echo "$CHANGED_FILES" | grep -q "^${crate_dir#./}/"; then
    echo "📦 Code changes detected in $crate_dir"
    
    # Check if Cargo.toml version was bumped
    if git diff --name-only $CI_MERGE_REQUEST_TARGET_BRANCH_NAME..HEAD | grep -q "^${crate_dir#./}/Cargo.toml$"; then
      if git diff $CI_MERGE_REQUEST_TARGET_BRANCH_NAME..HEAD "${crate_dir}/Cargo.toml" | grep -q "^+version"; then
        echo "✅ Version bump detected in $crate_dir"
      else
        echo "❌ Cargo.toml changed but no version bump in $crate_dir"
        exit 1
      fi
    else
      echo "❌ Code changes in $crate_dir but no version bump in Cargo.toml"
      exit 1
    fi
  fi
done

echo "✅ All version bump checks passed!"
