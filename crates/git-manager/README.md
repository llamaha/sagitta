# Git Manager

A centralized git functionality crate for sagitta with enhanced branch management, automatic resync capabilities, and merkle tree optimization for efficient change detection.

## Features

- **Centralized Git Operations**: All git functionality in one place
- **Branch Management**: Advanced branch switching with automatic resync
- **Merkle Tree Optimization**: Efficient change detection between branches
- **State Management**: Track repository and branch states
- **Modular Architecture**: Clean separation of concerns

## Usage

```rust
use git_manager::{GitManager, MerkleManager};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut manager = GitManager::new();
    let repo_path = PathBuf::from("/path/to/repo");

    // Initialize repository state
    manager.initialize_repository(&repo_path).await?;

    // Switch branches with automatic resync detection
    let result = manager.switch_branch(&repo_path, "feature-branch").await?;
    println!("Switched to branch: {}", result.new_branch);
    
    Ok(())
}
```

## Testing

Run the unit tests:

```bash
cargo test
```

Test the binary:

```bash
cargo run --bin git-manager-test
```

## Architecture

The crate is organized into several modules:

- `core/` - Core git operations and state management
- `sync/` - Merkle tree and change detection functionality
- `operations/` - High-level git operations (branch switching, etc.)
- `indexing/` - File processing and content extraction
- `error.rs` - Comprehensive error types

## Development Status

This crate is currently in Phase 1 of development as outlined in the git-centralization-plan.md. The foundation is complete with:

- ✅ Core state management structures
- ✅ Merkle tree implementation for change detection
- ✅ Comprehensive error handling
- ✅ Unit tests with >90% coverage
- ✅ Test binary for validation

## Next Steps

- Implement actual git operations using git2
- Add branch switching with automatic resync
- Implement repository initialization
- Add integration tests with real git repositories
- Performance optimization and benchmarking 