#!/bin/bash
set -e

echo "🔄 Generating changelogs for all modified crates..."

for crate_dir in crates/*/; do
  if [[ -d "$crate_dir" ]]; then
    crate_name=$(basename "$crate_dir")
    echo "📦 Generating changelog for $crate_name"
    
    git cliff --include-path "$crate_dir**" \
      --output "$crate_dir/CHANGELOG.md" \
      --unreleased
  fi
done

echo "✅ Changelog generation complete!"
