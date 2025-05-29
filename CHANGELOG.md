# sagitta-cli Release Notes

## [1.6.0] - 2025-04-19

### Added
- **Semantic Code Editing**: Powerful code editing capabilities that leverage semantic understanding:
  - Semantic element targeting for editing entire classes, functions, or methods
  - Line-based precision editing for targeted changes to specific sections
  - Validation-first workflow to ensure safety of edits
  - Both CLI and gRPC interfaces for programmatic access
- New CLI commands: `edit apply` and `edit validate` for code modification
- New gRPC endpoints: `EditCode` and `ValidateEdit` for programmatic editing

### Changed
- Updated tonic from 0.10.2 to 0.12.3 and prost from 0.12 to 0.13
- Improved embedding handler initialization using AppConfig
- Enhanced error handling throughout the codebase
- Refactored gRPC server implementation for better modularity

### Fixed
- Improved batch processing with retry logic and better error handling
- Fixed various issues with embedding handler initialization
- Enhanced server shutdown mechanism

### Documentation
- Added detailed documentation for the edit feature (docs/edit_feature.md)
- Updated the gRPC interface documentation
- Enhanced the README with new usage examples

## [1.5.0] - 2024-04-18

### Added
- Add support for configuring the repository storage location using `repo config set-repo-base-path` command.
- Add `repositories_base_path` configuration setting to specify the global repository storage location.
- Fix Qdrant port documentation to correctly identify 6333 as HTTP/REST (web UI) and 6334 as gRPC.
- Improve quickstart guide to clearly explain how to use the default ONNX model provided via Git LFS.

## [1.4.5] - 2024-04-15

### Added

## v1.4.3 - CLI Restructuring and Markdown Improvements

This release enhances the CLI structure, significantly improves markdown parsing, and includes various fixes and optimizations.

### Features and Improvements
- Improved markdown parser with section-based chunking to provide better document context
- Separated clear commands for simple index and repositories
- Moved simple commands into dedicated CLI subcommand
- Modularized repository commands for better maintainability
- Implemented progress bar for repository add operations
- Added dynamic embedding size support for CodeBERT models
- Replaced public API with a network layer

### Bug Fixes
- Fixed repository sync to prevent full reindex by passing config mutably
- Fixed SSH authentication issues
- Various minor fixes and improvements

### Documentation and Other Changes
- Increased test coverage from 38.52% to 42.27%
- Updated README with stability status information
- Reorganized CLI command structure for better usability
- Removed ambiguous top-level 'list' command

### Commits
- `21205cd` - Improve markdown parser with section-based chunking to provide better document context
- `c35af44` - Replace the public API with a network layer
- `269ca94` - Dynamic embedding size for CodeBERT
- `29859e9` - Fix SSH authentication
- `1903e80` - Increase test coverage to 42.27%
- `02878e8` - Implement progress bar for repo add
- `eb6f756` - Put sync command in own file
- `5fbf82a` - Minor fixes
- `781bf74` - Modularize repo commands
- `9db1e44` - Remove ambiguous top-level 'list' command
- `f2a669c` - Move simple commands into simple CLI subcommand
- `5e27149` - Separate clear commands for simple index and repositories
- `1a566f3` - Prevent full reindex on sync by passing config mutably
- `2ede043` - Add note to README.md about stability status

## v1.2.1 - Improved Repository Management and Binary Setup (2025-04-14)

This release enhances robustness around configuration and repository handling, and improves binary location setup.

### Changes

- Improvement to binary location setup
- Increased robustness around configuration and repository management
- Added excludes to published crate
- Fixed keywords limit for package publishing
- Updated documentation for language support and fallback chunking behavior

### Commits

- `7538b8ee` - Improvement to binary location setup
- `ff9367f2` - Improvement to binary location setup
- `dd3d2ccd` - Increase robustness around config and repo
- `24b5298` - Increase robustness around config and repo
- `541731b` - Too many keywords to publish
- `06fc94f` - Adding excludes to publish crate
- `8449124` - docs: Update language support table in README
- `1abc37b` - docs: Correct fallback description to whole-file chunking
- `37acbd8` - docs: Clarify fallback uses line-based chunking
- `7ce7e55` - fix: Remove unused imports after merges

## v1.2.0 - Repository Management System (2025-04-13)

This release introduces comprehensive Git repository management capabilities, allowing users to track and search across multiple repositories and branches.

### Changes

- Added repository management system
- Implemented multi-repository handling with add, list, use, and remove commands
- Added branch-awareness for repository indexing
- Improved sync functionality between Git repositories and search index
- Enhanced search capabilities to filter by repository and branch

### Commits

- `ac51c889` - Merge branch 'implement-repository-management' into 'main'
- `db9ea422` - Implement repository management

## v1.1.0 - Rust AST Parsing and Code-Aware Indexing (2025-04-13)

This release adds syntax-aware parsing for Rust code and improves the indexing system with Abstract Syntax Tree analysis.

### Changes

- Implemented Abstract Syntax Tree (AST) parsing for Rust code
- Added language-aware chunking for more meaningful code search results
- Improved parsing for other languages
- Enhanced fallback parser performance and utility
- Updated documentation with language support details

### Commits

- `93a35a6a` - Merge branch 'adding-rust-ast-parsing' into 'main'
- `29d147de` - Added Rust AST parsing
- `e9a80c91` - feat: Improve fallback parser performance and utility
- `b4d7bd42` - fix: Correct NameError in codebert.py script output

## v1.0.0 - Qdrant Migration and Library Restructuring (2025-04-12)

First stable release with migration to Qdrant vector database and significant refactoring as a public crate.

### Changes

- Migrated to Qdrant vector database for improved performance and scalability
- Refactored codebase as a public crate with library and CLI components
- Added proper CI/CD with GitLab pipeline integration
- Fixed libonnxruntime path location with build script and RPATH
- Improved error handling and documentation

### Commits

- `16a2588d` - Merge branch 'migrate-to-qdrant' into 'main'
- `030cb439` - Migrate to qdrant for additional performance
- `e11908f3` - feat: Use build script and RPATH for ONNX runtime linking
- `0e937512` - feat: Use build script and RPATH for ONNX runtime linking

## v0.3.0 - ONNX Runtime Integration (2025-04-03)

This release adds ONNX model support and optimizes embedding generation performance.

### Changes

- Integrated ONNX Runtime for enhanced embedding model support
- Added session pooling, tokenizer caching, and batch processing
- Implemented runtime warmup for better performance
- Added test script for validating ONNX features
- Added support for CUDA GPU acceleration (Linux) and Core ML (macOS)
- Included the all-MiniLM-L6-v2 ONNX model by default

### Commits

- `833b30c1` - fix: Use build script and RPATH for ONNX runtime linking
- `e34decac` - Phase 2: Add session pooling, tokenizer caching, and batch processing
- `b6811b40` - Phase 3 ONNX optimizations: runtime warmup, improved error handling, advanced batching strategies
- `e5e19ee9` - Add test script for validating ONNX features and Phase 3 optimizations
- `6fdf46c8` - Implement Phase 3 ONNX optimizations

## v0.2.0 - Search Improvements and Code Parsing (2025-04-02)

This release focuses on improving search quality and adding language-specific code parsing.

### Changes

- Added enhanced query preprocessing and diversity in search results
- Implemented code-aware ranking based on language-specific structures
- Added support for JavaScript, TypeScript, Python, Markdown, and YAML parsing
- Improved score normalization and path-based boosting
- Made hybrid search the default search method

### Commits

- `a23d87e2` - Implement code search improvements: score normalization, filepath pre-filtering, dynamic weight optimization
- `f2ab9992` - Implement search improvements: enhanced query preprocessing, diversity, ranking signals, and score normalization
- `3f651985` - Implement Stage 3: Code-Aware Ranking Enhancement
- `8ad92c7c` - Implement Stage 2: Hybrid Retrieval with BM25
- `75b35c2d` - Implement Stage 1: HNSW Optimization and File-Level Embeddings

## v0.1.0 - Initial Release (2025-03-31)

First release of sagitta-cli with basic functionality.

### Changes

- Initial implementation of semantic code search using vector embeddings
- Basic indexing of codebases with file-level chunking
- Integration with HNSW index for efficient vector search
- Command-line interface for indexing and querying
- Support for Rust language parsing

### Commits

- `ce4d4ebe` - fix: Resolve warnings & test failure in search module
- `c29f80b2` - docs: Add build tool prerequisites to README

## [Unreleased]

### Added

## [1.5.0] - 2024-04-18

### Added

- Add support for configuring the repository storage location using `repo config set-repo-base-path` command.
- Add `repositories_base_path` configuration setting to specify the global repository storage location.
- Fix Qdrant port documentation to correctly identify 6333 as HTTP/REST (web UI) and 6334 as gRPC.
- Improve quickstart guide to clearly explain how to use the default ONNX model provided via Git LFS.

## [1.4.5] - 2024-04-15

### Added
