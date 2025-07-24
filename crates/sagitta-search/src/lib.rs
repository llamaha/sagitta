#![warn(missing_docs)] // Enforce documentation for all public items

//! # sagitta-search
//! `sagitta-search` is the core library powering the `sagitta-cli` application.
//! It provides semantic code search functionality, including:
//!
//! - **Indexing**: Parse and index codebases using tree-sitter for syntax analysis
//! - **Embedding**: Generate embeddings using ONNX Runtime with sentence-transformer models
//! - **Search**: Perform semantic similarity search using Qdrant vector database
//! - **Configuration**: Manage application settings and repository configurations
//! - **Caching**: Cache embeddings and other data for improved performance
//!
//! ## Features
//!
//! - **Multi-language support**: Python, Rust, JavaScript, TypeScript, and more
//! - **GPU acceleration**: Optional CUDA support for faster embedding generation
//! - **Parallel processing**: Efficient multi-threaded indexing and search
//! - **Flexible configuration**: TOML-based configuration with sensible defaults
//! - **Repository management**: Git integration for tracking code changes
//!
//! ## Usage
//!
//! This library is primarily designed for internal use by the Sagitta ecosystem.
//! While primarily designed for use by `sagitta-cli`, components of this library can be
//! used independently for custom semantic search applications.
//!
//! ## Example
//!
//! ```rust,no_run
//! use sagitta_search::{AppConfig, EmbeddingPool, app_config_to_embedding_config};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Load configuration
//! let config = AppConfig::default();
//!
//! // Convert to embedding config and initialize embedding pool
//! let embedding_config = app_config_to_embedding_config(&config);
//! let embedding_pool = EmbeddingPool::with_configured_sessions(embedding_config)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Note on ONNX Runtime
//!
//! This library requires ONNX Runtime for embedding generation. When building
//! applications that use this library, ensure you have the appropriate ONNX Runtime
//! libraries installed on your system. The `ort` crate will attempt to download
//! the ONNX Runtime shared libraries, unlike the bundled `sagitta-cli` executable.

// crates/sagitta-search/src/lib.rs

// Public modules
/// Configuration management for the application.
pub mod config;
/// Embedding generation and handling.
// pub mod embedding; // Migrated to sagitta-embed crate
/// Defines the core error types and Result alias.
pub mod error;
/// Trait defining the interface for a Qdrant client, enabling mocking.
pub mod qdrant_client_trait;
/// Low-level Qdrant client operations.
pub mod qdrant_ops;
/// Code parsing and chunking logic for different languages.
pub mod syntax {
    pub use code_parsers::*;
}
/// Shared constants used across the library.
pub mod constants;
/// Code editing and validation logic.
pub mod edit;
/// Implementation of search functionality.
pub mod search_impl;
/// Search module providing vector store traits and utilities.
pub mod search;
/// Caching mechanisms, primarily for embeddings.
pub mod cache; // Added cache module
/// Core logic for indexing repositories and files.
pub mod indexing; // Added indexing module
/// Logic for adding new repositories.
pub mod repo_add;
/// Helper functions for repository operations (Git, Qdrant, Indexing).
pub mod repo_helpers;
/// Extracts code snippets with context.
pub mod snippet_extractor;
/// Code tokenization logic.
pub mod tokenizer;
/// Manages vocabulary for sparse vectors.
pub mod vocabulary;
/// Filesystem utilities.
pub mod fs_utils; // Added fs_utils module
/// Repository synchronization logic.
pub mod sync; // Added sync module
/// Repository synchronization progress logic.
pub mod sync_progress;

#[cfg(test)]
/// Utilities specific to testing within the core library.
pub mod test_utils;

pub use config::{AppConfig, IndexingConfig, RepositoryConfig, RepositoryDependency, EmbeddingEngineConfig, load_config, save_config, get_config_path_or_default, get_managed_repos_from_config};
// Re-export from sagitta-embed crate
pub use sagitta_embed::{EmbeddingModel, EmbeddingModelType, EmbeddingPool, EmbeddingProcessor};
// Re-export EmbeddingConfig for convenience
pub use sagitta_embed::config::EmbeddingConfig;
pub use error::{SagittaError, Result};
pub use qdrant_client_trait::QdrantClientTrait;
pub use constants::*;
pub use cache::EmbeddingCache; // Added cache re-export
pub use snippet_extractor::extract_snippet;
// Re-export search module types
pub use search::{VectorStore, VectorStoreError, VectorSearchResult, UpsertResult, CollectionConfig, DistanceMetric, VectorPoint, SearchQuery, CollectionInfo, CollectionStatus, ScrollResult};

// Re-export core functionalities
// pub use indexing::index_paths; // Removed - indexing mod not directly exposed
pub use search_impl::{search_collection, SearchParams}; // Correctly export search_collection and SearchParams
pub use qdrant_ops::delete_all_points;

// Re-export other necessary items if needed by CLI directly
pub use edit::{apply_edit, validate_edit, EditTarget, EngineEditOptions, EngineValidationIssue, EngineValidationSeverity};
pub use repo_helpers::{delete_repository_data, get_collection_name};
pub use indexing::{ensure_collection_exists, IndexRepoFilesParams};
pub use repo_add::{handle_repo_add, AddRepoArgs, AddRepoError}; // Assuming repo_add is needed by CLI
pub use sync::{sync_repository, SyncOptions, SyncResult}; // Added sync re-export

// Re-export qdrant types needed by mcp or other crates
pub use qdrant_client::qdrant::{PointStruct, Filter, Condition, FieldCondition, Match, Range, PointsSelector, Value, Vectors, Vector, NamedVectors, ScoredPoint, SearchPoints, QueryPoints, QueryResponse, CollectionInfo as QdrantCollectionInfo, CountPoints, CountResponse, PointsOperationResponse, UpsertPoints, DeletePoints, CreateCollection, DeleteCollection, HealthCheckReply, Distance, VectorParams, VectorsConfig, SparseVectorParams, SparseVectorConfig, vectors_config, point_id::PointIdOptions, PointId, VectorParamsMap, HnswConfigDiff, OptimizersConfigDiff, WalConfigDiff, QuantizationConfig, ScalarQuantization, ProductQuantization, BinaryQuantization, /*quantization_config::Quantizer,*/ CompressionRatio, ListCollectionsResponse, CollectionDescription, AliasDescription, /*CollectionAliases,*/ ListAliasesRequest, /*UpdateCollectionAliases,*/ AliasOperations, CreateAlias, RenameAlias, DeleteAlias};

// Additional re-exports for enhanced repository functionality
pub use config::{get_config_path, ManagedRepositories, get_repo_base_path};

use std::sync::Arc;

/// Basic addition function (example/placeholder).
pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }

    #[test]
    fn test_app_config_to_embedding_config_mapping() {
        use crate::config::{AppConfig, EmbeddingEngineConfig, PerformanceConfig};
        
        // Create a test AppConfig with custom embedding settings
        let mut app_config = AppConfig::default();
        // Clear the default embed_model since we want to test manual ONNX paths
        app_config.embed_model = None;
        app_config.onnx_model_path = Some("/path/to/model.onnx".to_string());
        app_config.onnx_tokenizer_path = Some("/path/to/tokenizer.json".to_string());
        app_config.performance.vector_dimension = 512;
        
        // Set custom embedding configuration
        app_config.embedding = EmbeddingEngineConfig {
            session_timeout_seconds: 600,
            enable_session_cleanup: false,
            embedding_batch_size: 64,
        };
        
        // Convert to EmbeddingConfig
        let embedding_config = app_config_to_embedding_config(&app_config);
        
        // Verify all fields are properly mapped
        assert_eq!(embedding_config.model_type, EmbeddingModelType::Onnx);
        assert_eq!(embedding_config.onnx_model_path, Some("/path/to/model.onnx".into()));
        assert_eq!(embedding_config.onnx_tokenizer_path, Some("/path/to/tokenizer.json".into()));
        assert_eq!(embedding_config.session_timeout_seconds, 600);
        assert_eq!(embedding_config.enable_session_cleanup, false);
        assert_eq!(embedding_config.expected_dimension, Some(512));
        assert_eq!(embedding_config.embedding_batch_size, Some(64));
    }

    #[test]
    fn test_app_config_to_embedding_config_defaults() {
        // Test with default AppConfig (which now includes bge-small-fast by default)
        let app_config = AppConfig::default();
        let embedding_config = app_config_to_embedding_config(&app_config);
        
        // With the new defaults, the default AppConfig includes embed_model = "bge-small-fast"
        // This should try to download the model and use ONNX type, or fall back to Default if download fails
        // In test environments without internet access, it might fall back to Default
        assert!(
            embedding_config.model_type == EmbeddingModelType::Onnx || 
            embedding_config.model_type == EmbeddingModelType::Default
        );
        
        // If ONNX model is used, paths should be set; if Default is used, they should be None
        if embedding_config.model_type == EmbeddingModelType::Onnx {
            assert!(embedding_config.onnx_model_path.is_some());
            assert!(embedding_config.onnx_tokenizer_path.is_some());
        } else {
            assert_eq!(embedding_config.onnx_model_path, None);
            assert_eq!(embedding_config.onnx_tokenizer_path, None);
        }
        
        assert_eq!(embedding_config.session_timeout_seconds, 300);
        assert_eq!(embedding_config.enable_session_cleanup, true);
        assert_eq!(embedding_config.expected_dimension, Some(384)); // Default vector dimension
        assert_eq!(embedding_config.embedding_batch_size, Some(8)); // Default batch size
    }

    #[tokio::test]
    async fn test_scan_for_orphaned_repositories() {
        use tempfile::TempDir;
        use std::fs;
        
        // Create a temporary directory structure
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().join("repositories");
        fs::create_dir_all(&base_path).unwrap();
        
        // Create some directories
        fs::create_dir(&base_path.join("configured-repo")).unwrap();
        fs::create_dir(&base_path.join("orphaned-repo1")).unwrap();
        fs::create_dir(&base_path.join("orphaned-repo2")).unwrap();
        
        // Make orphaned-repo2 a git repository
        fs::create_dir(&base_path.join("orphaned-repo2/.git")).unwrap();
        
        // Create a config with only one configured repo
        let config = AppConfig {
            repositories_base_path: Some(base_path.to_string_lossy().to_string()),
            repositories: vec![
                RepositoryConfig {
                    name: "configured-repo".to_string(),
                    url: "https://example.com/repo.git".to_string(),
                    local_path: base_path.join("configured-repo"),
                    default_branch: "main".to_string(),
                    tracked_branches: vec!["main".to_string()],
                    remote_name: Some("origin".to_string()),
                    active_branch: Some("main".to_string()),
                    ssh_key_path: None,
                    ssh_key_passphrase: None,
                    last_synced_commits: std::collections::HashMap::new(),
                    indexed_languages: None,
                    added_as_local_path: false,
                    target_ref: None,
                    dependencies: Vec::new(),
                    last_synced_commit: None,
                }
            ],
            ..Default::default()
        };
        
        // Scan for orphaned repositories
        let orphaned = scan_for_orphaned_repositories(&config).await.unwrap();
        
        // Verify results
        assert_eq!(orphaned.len(), 2);
        
        let names: Vec<String> = orphaned.iter().map(|o| o.name.clone()).collect();
        assert!(names.contains(&"orphaned-repo1".to_string()));
        assert!(names.contains(&"orphaned-repo2".to_string()));
        
        // Check git repository detection
        let git_repo = orphaned.iter().find(|o| o.name == "orphaned-repo2").unwrap();
        assert!(git_repo.is_git_repository);
        
        let non_git_repo = orphaned.iter().find(|o| o.name == "orphaned-repo1").unwrap();
        assert!(!non_git_repo.is_git_repository);
    }

    #[tokio::test]
    async fn test_scan_for_orphaned_repositories_no_base_path() {
        use tempfile::TempDir;
        
        // Test when base path doesn't exist
        let temp_dir = TempDir::new().unwrap();
        let non_existent_path = temp_dir.path().join("non_existent");
        
        let config = AppConfig {
            repositories_base_path: Some(non_existent_path.to_string_lossy().to_string()),
            ..Default::default()
        };
        
        let orphaned = scan_for_orphaned_repositories(&config).await.unwrap();
        assert_eq!(orphaned.len(), 0);
    }

    #[tokio::test]
    async fn test_add_orphaned_repository() {
        use tempfile::TempDir;
        use std::fs;
        
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("orphaned-repo");
        fs::create_dir_all(&repo_path).unwrap();
        
        let mut config = AppConfig::default();
        
        let orphaned_repo = OrphanedRepository {
            name: "orphaned-repo".to_string(),
            local_path: repo_path.clone(),
            is_git_repository: false,
            remote_url: Some("https://example.com/repo.git".to_string()),
            file_count: Some(10),
            size_bytes: Some(1024),
        };
        
        // Add the orphaned repository
        add_orphaned_repository(&mut config, &orphaned_repo).await.unwrap();
        
        // Verify it was added
        assert_eq!(config.repositories.len(), 1);
        let added_repo = &config.repositories[0];
        assert_eq!(added_repo.name, "orphaned-repo");
        assert_eq!(added_repo.url, "https://example.com/repo.git");
        assert_eq!(added_repo.local_path, repo_path);
        assert!(added_repo.added_as_local_path);
    }

    #[tokio::test]
    async fn test_add_orphaned_repository_no_remote() {
        use tempfile::TempDir;
        use std::fs;
        
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("local-repo");
        fs::create_dir_all(&repo_path).unwrap();
        
        let mut config = AppConfig::default();
        
        let orphaned_repo = OrphanedRepository {
            name: "local-repo".to_string(),
            local_path: repo_path.clone(),
            is_git_repository: false,
            remote_url: None,
            file_count: None,
            size_bytes: None,
        };
        
        // Add the orphaned repository
        add_orphaned_repository(&mut config, &orphaned_repo).await.unwrap();
        
        // Verify it was added with local URL
        assert_eq!(config.repositories.len(), 1);
        let added_repo = &config.repositories[0];
        assert_eq!(added_repo.name, "local-repo");
        assert!(added_repo.url.starts_with("local://"));
        assert!(added_repo.added_as_local_path);
    }

    #[tokio::test]
    async fn test_add_orphaned_repository_duplicate_name() {
        use tempfile::TempDir;
        use std::fs;
        
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("existing-repo");
        fs::create_dir_all(&repo_path).unwrap();
        
        let mut config = AppConfig {
            repositories: vec![
                RepositoryConfig {
                    name: "existing-repo".to_string(),
                    url: "https://example.com/existing.git".to_string(),
                    local_path: PathBuf::from("/some/other/path"),
                    default_branch: "main".to_string(),
                    tracked_branches: vec!["main".to_string()],
                    remote_name: Some("origin".to_string()),
                    active_branch: Some("main".to_string()),
                    ssh_key_path: None,
                    ssh_key_passphrase: None,
                    last_synced_commits: std::collections::HashMap::new(),
                    indexed_languages: None,
                    added_as_local_path: false,
                    target_ref: None,
                    dependencies: Vec::new(),
                    last_synced_commit: None,
                }
            ],
            ..Default::default()
        };
        
        let orphaned_repo = OrphanedRepository {
            name: "existing-repo".to_string(),
            local_path: repo_path,
            is_git_repository: false,
            remote_url: None,
            file_count: None,
            size_bytes: None,
        };
        
        // Try to add duplicate
        let result = add_orphaned_repository(&mut config, &orphaned_repo).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn test_remove_orphaned_repository() {
        use tempfile::TempDir;
        use std::fs;
        
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("to-remove");
        fs::create_dir_all(&repo_path).unwrap();
        fs::write(repo_path.join("test.txt"), "test content").unwrap();
        
        let orphaned_repo = OrphanedRepository {
            name: "to-remove".to_string(),
            local_path: repo_path.clone(),
            is_git_repository: false,
            remote_url: None,
            file_count: Some(1),
            size_bytes: Some(12),
        };
        
        // Remove the directory
        remove_orphaned_repository(&orphaned_repo).await.unwrap();
        
        // Verify it's gone
        assert!(!repo_path.exists());
    }

    #[tokio::test]
    async fn test_remove_orphaned_repository_not_exists() {
        let orphaned_repo = OrphanedRepository {
            name: "non-existent".to_string(),
            local_path: PathBuf::from("/non/existent/path"),
            is_git_repository: false,
            remote_url: None,
            file_count: None,
            size_bytes: None,
        };
        
        // Try to remove non-existent directory
        let result = remove_orphaned_repository(&orphaned_repo).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_enhanced_repository_list_with_orphaned() {
        use tempfile::TempDir;
        use std::fs;
        
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().join("repositories");
        fs::create_dir_all(&base_path).unwrap();
        
        // Create configured repo directory
        fs::create_dir(&base_path.join("configured-repo")).unwrap();
        // Create orphaned repo directory
        fs::create_dir(&base_path.join("orphaned-repo")).unwrap();
        
        let config = AppConfig {
            repositories_base_path: Some(base_path.to_string_lossy().to_string()),
            repositories: vec![
                RepositoryConfig {
                    name: "configured-repo".to_string(),
                    url: "https://example.com/repo.git".to_string(),
                    local_path: base_path.join("configured-repo"),
                    default_branch: "main".to_string(),
                    tracked_branches: vec!["main".to_string()],
                    remote_name: Some("origin".to_string()),
                    active_branch: Some("main".to_string()),
                    ssh_key_path: None,
                    ssh_key_passphrase: None,
                    last_synced_commits: std::collections::HashMap::new(),
                    indexed_languages: None,
                    added_as_local_path: false,
                    target_ref: None,
                    dependencies: Vec::new(),
                    last_synced_commit: None,
                },
                RepositoryConfig {
                    name: "missing-repo".to_string(),
                    url: "https://example.com/missing.git".to_string(),
                    local_path: base_path.join("missing-repo"),
                    default_branch: "main".to_string(),
                    tracked_branches: vec!["main".to_string()],
                    remote_name: Some("origin".to_string()),
                    active_branch: Some("main".to_string()),
                    ssh_key_path: None,
                    ssh_key_passphrase: None,
                    last_synced_commits: std::collections::HashMap::new(),
                    indexed_languages: None,
                    added_as_local_path: false,
                    target_ref: None,
                    dependencies: Vec::new(),
                    last_synced_commit: None,
                }
            ],
            ..Default::default()
        };
        
        // Get enhanced repository list
        let enhanced_list = get_enhanced_repository_list(&config).await.unwrap();
        
        // Verify results
        assert_eq!(enhanced_list.repositories.len(), 2);
        assert_eq!(enhanced_list.orphaned_repositories.len(), 1);
        assert_eq!(enhanced_list.summary.existing_count, 1);
        assert_eq!(enhanced_list.summary.missing_count, 1);
        assert_eq!(enhanced_list.summary.orphaned_count, 1);
        
        // Check orphaned repo
        assert_eq!(enhanced_list.orphaned_repositories[0].name, "orphaned-repo");
    }

    #[tokio::test]
    async fn test_reclone_missing_repository_not_found() {
        let config = AppConfig::default();
        
        // Try to reclone non-existent repository
        let result = reclone_missing_repository(&config, "non-existent").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found in config"));
    }

    #[tokio::test]
    async fn test_reclone_missing_repository_local_path() {
        use tempfile::TempDir;
        use std::fs;
        
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("local-repo");
        fs::create_dir_all(&repo_path).unwrap();
        
        let config = AppConfig {
            repositories: vec![
                RepositoryConfig {
                    name: "local-repo".to_string(),
                    url: "local://path".to_string(),
                    local_path: repo_path,
                    default_branch: "main".to_string(),
                    tracked_branches: vec!["main".to_string()],
                    remote_name: Some("origin".to_string()),
                    active_branch: Some("main".to_string()),
                    ssh_key_path: None,
                    ssh_key_passphrase: None,
                    last_synced_commits: std::collections::HashMap::new(),
                    indexed_languages: None,
                    added_as_local_path: true, // This makes it non-reclonable
                    target_ref: None,
                    dependencies: Vec::new(),
                    last_synced_commit: None,
                }
            ],
            ..Default::default()
        };
        
        // Try to reclone local path repository
        let result = reclone_missing_repository(&config, "local-repo").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be recloned"));
    }

    #[tokio::test]
    async fn test_scan_orphaned_with_git_timeout() {
        use tempfile::TempDir;
        use std::fs;
        
        // This test verifies that slow git operations don't block orphaned repo scanning
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().join("repositories");
        fs::create_dir_all(&base_path).unwrap();
        
        // Create a git repository
        let git_repo_path = base_path.join("slow-git-repo");
        fs::create_dir(&git_repo_path).unwrap();
        fs::create_dir(&git_repo_path.join(".git")).unwrap();
        
        let config = AppConfig {
            repositories_base_path: Some(base_path.to_string_lossy().to_string()),
            repositories: vec![], // No configured repos
            ..Default::default()
        };
        
        // Scan should complete quickly even if git operations would be slow
        let start = std::time::Instant::now();
        let orphaned = scan_for_orphaned_repositories(&config).await.unwrap();
        let duration = start.elapsed();
        
        // Should complete within reasonable time (well under 5 seconds)
        assert!(duration.as_secs() < 5);
        assert_eq!(orphaned.len(), 1);
        assert_eq!(orphaned[0].name, "slow-git-repo");
        assert!(orphaned[0].is_git_repository);
    }

    #[tokio::test]
    async fn test_enhanced_repository_info_filesystem_only() {
        use tempfile::TempDir;
        use std::fs;
        
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test-repo");
        fs::create_dir_all(&repo_path).unwrap();
        fs::write(repo_path.join("test.txt"), "content").unwrap();
        
        let repo_config = RepositoryConfig {
            name: "test-repo".to_string(),
            url: "https://example.com/test.git".to_string(),
            local_path: repo_path,
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string()],
            remote_name: Some("origin".to_string()),
            active_branch: Some("main".to_string()),
            ssh_key_path: None,
            ssh_key_passphrase: None,
            last_synced_commits: std::collections::HashMap::new(),
            indexed_languages: None,
            added_as_local_path: false,
            target_ref: None,
            dependencies: Vec::new(),
            last_synced_commit: None,
        };
        
        let enhanced_info = get_enhanced_repository_info(&repo_config).await.unwrap();
        
        // Verify filesystem status
        assert!(enhanced_info.filesystem_status.exists);
        assert!(enhanced_info.filesystem_status.accessible);
        assert!(!enhanced_info.filesystem_status.is_git_repository);
        assert_eq!(enhanced_info.filesystem_status.total_files, Some(1));
        
        // Should have no git status
        assert!(enhanced_info.git_status.is_none());
        
        // Sync state should be never synced
        assert_eq!(enhanced_info.sync_status.state, SyncState::NeverSynced);
    }

    #[test]
    fn test_config_toml_to_decoupled_processing() {
        use sagitta_embed::processor::ProcessingConfig;
        
        // max_sessions tests removed as feature no longer exists
        // ProcessingConfig now uses automatic session management
    }
}

extern crate log;

/// Converts AppConfig to EmbeddingConfig for use with sagitta-embed
pub fn app_config_to_embedding_config(app_config: &AppConfig) -> EmbeddingConfig {
    use sagitta_embed::EmbeddingModelType;
    use sagitta_embed::model::download::{EmbeddingModel, ModelDownloader};
    use std::path::PathBuf;

    // First validate the config
    if let Err(e) = app_config.validate() {
        // Log the error but continue with defaults if validation fails
        log::error!("Configuration validation error: {e}");
    }

    let (model_type, onnx_model_path, onnx_tokenizer_path) = if let Some(embed_model) = &app_config.embed_model {
        // Special handling for test mode
        if embed_model == "test-default" {
            log::info!("Using test-default model (Default embedding provider)");
            (EmbeddingModelType::Default, None, None)
        } else {
            // Use automatic model downloading
            let model = EmbeddingModel::parse(embed_model);
            match ModelDownloader::new() {
            Ok(downloader) => {
                match downloader.download_model(&model) {
                    Ok(paths) => {
                        log::info!("Using model: {embed_model}");
                        let tokenizer_dir = paths.tokenizer_dir().ok();
                        (
                            EmbeddingModelType::Onnx, 
                            Some(paths.model_path),
                            tokenizer_dir
                        )
                    },
                    Err(e) => {
                        log::error!("Failed to download model {embed_model}: {e}");
                        log::warn!("Falling back to Default model type due to download failure");
                        // Use Default model type when download fails to avoid validation errors
                        log::debug!("app_config_to_embedding_config: Setting model type to Default due to download failure");
                        (EmbeddingModelType::Default, None, None)
                    }
                }
                },
                Err(e) => {
                    log::error!("Failed to create model downloader: {e}");
                    // Use Default model type when downloader creation fails to avoid validation errors
                    (EmbeddingModelType::Default, None, None)
                }
            }
        }
    } else if let (Some(model_path), Some(tokenizer_path)) = 
        (&app_config.onnx_model_path, &app_config.onnx_tokenizer_path) {
        // Use manually specified paths
        (EmbeddingModelType::Onnx, Some(PathBuf::from(model_path)), Some(PathBuf::from(tokenizer_path)))
    } else {
        // No model configuration provided - use Default model type
        (EmbeddingModelType::Default, None, None)
    };

    log::debug!("Creating EmbeddingConfig with model_type: {model_type:?}, model_path: {onnx_model_path:?}, tokenizer_path: {onnx_tokenizer_path:?}");
    
    EmbeddingConfig {
        model_type,
        onnx_model_path,
        onnx_tokenizer_path,
        // max_sessions removed - using automatic session management
        session_timeout_seconds: app_config.embedding.session_timeout_seconds,
        enable_session_cleanup: app_config.embedding.enable_session_cleanup,
        expected_dimension: Some(app_config.performance.vector_dimension as usize),
        embedding_batch_size: Some(app_config.embedding.embedding_batch_size),
        ..Default::default()
    }
}

/// Creates an EmbeddingPool with automatic session management.
/// This provides GPU memory control through intelligent pooling.
/// 
/// # Arguments
/// * `app_config` - The application configuration containing embedding settings
/// 
/// # Returns
/// * `Result<Arc<EmbeddingPool>>` - A thread-safe embedding pool with automatic session management
/// 
/// # Example
/// ```rust,no_run
/// use sagitta_search::{create_embedding_pool, embed_text_with_pool, AppConfig};
/// 
/// # async fn example(config: &AppConfig) -> Result<(), Box<dyn std::error::Error>> {
/// let pool = create_embedding_pool(config).await?;
/// let embeddings = embed_text_with_pool(&pool, &["Hello world"]).await?;
/// # Ok(())
/// # }
/// ```
pub async fn create_embedding_pool(app_config: &AppConfig) -> std::result::Result<Arc<EmbeddingPool>, SagittaError> {
    let embedding_config = app_config_to_embedding_config(app_config);
    let pool = EmbeddingPool::with_configured_sessions(embedding_config)
        .map_err(|e| SagittaError::EmbeddingError(e.to_string()))?;
    Ok(Arc::new(pool))
}

/// Creates an EmbeddingPool from an EmbeddingConfig with automatic session management.
/// This is a convenience function for cases where you already have an EmbeddingConfig.
/// 
/// # Arguments
/// * `embedding_config` - The embedding configuration
/// 
/// # Returns
/// * `Result<Arc<EmbeddingPool>>` - A thread-safe embedding pool with automatic session management
pub async fn create_embedding_pool_from_config(embedding_config: EmbeddingConfig) -> std::result::Result<Arc<EmbeddingPool>, SagittaError> {
    let pool = EmbeddingPool::with_configured_sessions(embedding_config)
        .map_err(|e| SagittaError::EmbeddingError(e.to_string()))?;
    Ok(Arc::new(pool))
}

/// Helper function to embed text using EmbeddingPool with the same interface as EmbeddingHandler.
/// This bridges the gap between the old EmbeddingHandler::embed() and new EmbeddingPool::process_chunks().
pub async fn embed_text_with_pool(pool: &EmbeddingPool, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
    use sagitta_embed::processor::{ProcessedChunk, ChunkMetadata};
    use std::path::PathBuf;
    
    // Convert texts to ProcessedChunks
    let chunks: Vec<ProcessedChunk> = texts.iter().enumerate().map(|(i, text)| {
        ProcessedChunk {
            content: text.to_string(),
            metadata: ChunkMetadata {
                file_path: PathBuf::from("text"),
                start_line: 0,
                end_line: 0,
                language: "text".to_string(),
                file_extension: "txt".to_string(),
                element_type: "text".to_string(),
                context: None,
            },
            id: format!("text_{i}"),
        }
    }).collect();
    
    // Process chunks
    let embedded_chunks = pool.process_chunks(chunks).await
        .map_err(|e| SagittaError::EmbeddingError(e.to_string()))?;
    
    // Extract embeddings
    let embeddings = embedded_chunks.into_iter()
        .map(|chunk| chunk.embedding)
        .collect();
    
    Ok(embeddings)
}

/// Helper function to embed a single text using EmbeddingPool.
pub async fn embed_single_text_with_pool(pool: &EmbeddingPool, text: &str) -> Result<Vec<f32>> {
    let results = embed_text_with_pool(pool, &[text]).await?;
    results.into_iter().next()
        .ok_or_else(|| SagittaError::EmbeddingError("No embeddings returned for text".to_string()))
}

/// Adapter that implements EmbeddingProvider for EmbeddingPool.
/// This bridges the interface gap between the old EmbeddingHandler and new EmbeddingPool.
#[derive(Debug)]
pub struct EmbeddingPoolAdapter {
    pool: Arc<EmbeddingPool>,
}

impl EmbeddingPoolAdapter {
    /// Creates a new EmbeddingPoolAdapter with the given pool
    pub fn new(pool: Arc<EmbeddingPool>) -> Self {
        Self { pool }
    }
}

impl sagitta_embed::provider::EmbeddingProvider for EmbeddingPoolAdapter {
    fn dimension(&self) -> usize {
        self.pool.dimension()
    }

    fn model_type(&self) -> sagitta_embed::EmbeddingModelType {
        sagitta_embed::EmbeddingModelType::Onnx // EmbeddingPool is ONNX-based
    }

    fn embed_batch(&self, texts: &[&str]) -> std::result::Result<Vec<Vec<f32>>, sagitta_embed::SagittaEmbedError> {
        // This is a blocking call, but EmbeddingPool is async
        // We need to handle this properly without nesting runtimes
        use std::sync::mpsc;
        use std::thread;
        
        // Create a channel for communication between threads
        let (tx, rx) = mpsc::channel();
        let pool = Arc::clone(&self.pool);
        let texts_owned: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
        
        // Spawn a new thread that will create its own runtime
        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                let text_refs: Vec<&str> = texts_owned.iter().map(|s| s.as_str()).collect();
                embed_text_with_pool(&pool, &text_refs).await
                    .map_err(|e| sagitta_embed::SagittaEmbedError::provider(e.to_string()))
            });
            let _ = tx.send(result);
        });
        
        // Wait for the result
        rx.recv().map_err(|_| sagitta_embed::SagittaEmbedError::provider("Failed to receive result from embedding thread".to_string()))?
    }
}

/// Additional imports and re-exports for enhanced repository functionality
use serde::{Serialize, Deserialize};
use std::path::Path;

/// Enhanced repository information structure with comprehensive status details
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct EnhancedRepositoryInfo {
    /// Repository name
    pub name: String,
    /// Repository URL
    pub url: String,
    /// Local filesystem path
    pub local_path: std::path::PathBuf,
    /// Default branch name
    pub default_branch: String,
    /// Currently active/checked-out branch
    pub active_branch: Option<String>,
    /// All tracked branches
    pub tracked_branches: Vec<String>,
    /// Filesystem status
    pub filesystem_status: FilesystemStatus,
    /// Git repository status
    pub git_status: Option<GitRepositoryStatus>,
    /// Sync state information
    pub sync_status: SyncStatus,
    /// Indexed languages detected
    pub indexed_languages: Option<Vec<String>>,
    /// File extension statistics
    pub file_extensions: Vec<FileExtensionInfo>,
    /// Whether added as local path vs cloned
    pub added_as_local_path: bool,
    /// Target ref if specified
    pub target_ref: Option<String>,
    /// Last sync timestamp if available
    pub last_sync_time: Option<chrono::DateTime<chrono::Utc>>,
    /// Repository dependencies
    pub dependencies: Vec<RepositoryDependency>,
}

/// Filesystem status of the repository
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FilesystemStatus {
    /// Whether the path exists on filesystem
    pub exists: bool,
    /// Whether the path is accessible
    pub accessible: bool,
    /// Whether it contains a valid git repository
    pub is_git_repository: bool,
    /// Total file count (if calculable)
    pub total_files: Option<usize>,
    /// Repository size in bytes (if calculable)
    pub size_bytes: Option<u64>,
}

/// Git repository status information
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GitRepositoryStatus {
    /// Current commit hash
    pub current_commit: String,
    /// Current branch name (None if in detached HEAD state)
    pub current_branch: Option<String>,
    /// Whether repository is clean (no uncommitted changes)
    pub is_clean: bool,
    /// Remote URL from git configuration
    pub remote_url: Option<String>,
    /// Available branches
    pub available_branches: Vec<String>,
    /// Whether repository is in detached HEAD state
    pub is_detached_head: bool,
}

/// Sync state information
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SyncStatus {
    /// Overall sync state
    pub state: SyncState,
    /// Last synced commit hashes per branch
    pub last_synced_commits: std::collections::HashMap<String, String>,
    /// Branches that need syncing
    pub branches_needing_sync: Vec<String>,
    /// Whether sync is currently in progress
    pub sync_in_progress: bool,
}

/// Sync state enumeration
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum SyncState {
    /// Repository has never been synced
    NeverSynced,
    /// Repository is up to date
    UpToDate,
    /// Repository needs syncing
    NeedsSync,
    /// Sync state cannot be determined
    Unknown,
}

/// File extension information
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FileExtensionInfo {
    /// File extension (without the dot)
    pub extension: String,
    /// Number of files with this extension
    pub count: usize,
    /// Total size in bytes for files with this extension
    pub size_bytes: u64,
}

/// Enhanced repository listing result
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct EnhancedRepositoryList {
    /// List of enhanced repository information
    pub repositories: Vec<EnhancedRepositoryInfo>,
    /// Active repository name
    pub active_repository: Option<String>,
    /// Total count of repositories
    pub total_count: usize,
    /// Summary statistics
    pub summary: RepositoryListSummary,
    /// Orphaned repositories found on filesystem but not in config
    pub orphaned_repositories: Vec<OrphanedRepository>,
}

/// Summary statistics for repository list
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RepositoryListSummary {
    /// Number of repositories that exist on filesystem
    pub existing_count: usize,
    /// Number of repositories that need syncing
    pub needs_sync_count: usize,
    /// Number of repositories with uncommitted changes
    pub dirty_count: usize,
    /// Total file count across all repositories
    pub total_files: usize,
    /// Total size in bytes across all repositories
    pub total_size_bytes: u64,
    /// Most common file extensions across all repositories
    pub common_extensions: Vec<FileExtensionInfo>,
    /// Number of orphaned repositories found on filesystem
    pub orphaned_count: usize,
    /// Number of missing repositories (in config but not on filesystem)
    pub missing_count: usize,
}

/// Represents a repository found on filesystem but not in configuration
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct OrphanedRepository {
    /// Directory name
    pub name: String,
    /// Full path to the directory
    pub local_path: std::path::PathBuf,
    /// Whether it contains a .git directory
    pub is_git_repository: bool,
    /// Remote URL if it can be determined from git config
    pub remote_url: Option<String>,
    /// File count
    pub file_count: Option<usize>,
    /// Size in bytes
    pub size_bytes: Option<u64>,
}

/// Get enhanced repository listing with comprehensive information
pub async fn get_enhanced_repository_list(config: &AppConfig) -> Result<EnhancedRepositoryList> {
    let mut enhanced_repos = Vec::new();
    let mut summary = RepositoryListSummary {
        existing_count: 0,
        needs_sync_count: 0,
        dirty_count: 0,
        total_files: 0,
        total_size_bytes: 0,
        common_extensions: Vec::new(),
        orphaned_count: 0,
        missing_count: 0,
    };
    
    // Collect extension statistics across all repositories
    let mut all_extensions: std::collections::HashMap<String, FileExtensionInfo> = std::collections::HashMap::new();
    
    let mut missing_count = 0;
    
    for repo_config in &config.repositories {
        let enhanced_info = get_enhanced_repository_info(repo_config).await?;
        
        // Update summary statistics
        if enhanced_info.filesystem_status.exists {
            summary.existing_count += 1;
        } else {
            missing_count += 1;
        }
        
        if enhanced_info.sync_status.state == SyncState::NeedsSync {
            summary.needs_sync_count += 1;
        }
        
        if let Some(git_status) = &enhanced_info.git_status {
            if !git_status.is_clean {
                summary.dirty_count += 1;
            }
        }
        
        if let Some(file_count) = enhanced_info.filesystem_status.total_files {
            summary.total_files += file_count;
        }
        
        if let Some(size) = enhanced_info.filesystem_status.size_bytes {
            summary.total_size_bytes += size;
        }
        
        // Aggregate extension statistics
        for ext_info in &enhanced_info.file_extensions {
            if let Some(existing) = all_extensions.get_mut(&ext_info.extension) {
                existing.count += ext_info.count;
                existing.size_bytes += ext_info.size_bytes;
            } else {
                all_extensions.insert(ext_info.extension.clone(), ext_info.clone());
            }
        }
        
        enhanced_repos.push(enhanced_info);
    }
    
    // Scan for orphaned repositories
    let orphaned_repos = scan_for_orphaned_repositories(config).await?;
    let orphaned_count = orphaned_repos.len();
    
    // Update summary with missing and orphaned counts
    summary.missing_count = missing_count;
    summary.orphaned_count = orphaned_count;
    
    // Sort extensions by count and take top 10
    let mut sorted_extensions: Vec<_> = all_extensions.into_values().collect();
    sorted_extensions.sort_by(|a, b| b.count.cmp(&a.count));
    summary.common_extensions = sorted_extensions.into_iter().take(10).collect();
    
    Ok(EnhancedRepositoryList {
        repositories: enhanced_repos,
        active_repository: config.active_repository.clone(),
        total_count: config.repositories.len(),
        summary,
        orphaned_repositories: orphaned_repos,
    })
}

/// Get enhanced information for a single repository
pub async fn get_enhanced_repository_info(repo_config: &RepositoryConfig) -> Result<EnhancedRepositoryInfo> {
    
    
    // Check filesystem status
    let filesystem_status = get_filesystem_status(&repo_config.local_path).await?;
    
    // Get git status if it's a git repository
    let git_status = if filesystem_status.is_git_repository {
        match get_git_repository_status(&repo_config.local_path).await {
            Ok(status) => Some(status),
            Err(e) => {
                log::debug!("Failed to get git repository status for {}: {}", repo_config.name, e);
                None
            }
        }
    } else {
        None
    };
    
    // Determine sync status
    let sync_status = get_sync_status(repo_config, git_status.as_ref()).await?;
    
    // Get file extension statistics
    let file_extensions = if filesystem_status.exists {
        get_file_extension_stats(&repo_config.local_path).await?
    } else {
        Vec::new()
    };
    
    // Determine the current active branch from git status
    let active_branch = if let Some(git_status) = &git_status {
        // Use the current branch from git status if available
        git_status.current_branch.clone()
    } else {
        // Fall back to repo config if no git status
        repo_config.active_branch.clone()
    };
    
    Ok(EnhancedRepositoryInfo {
        name: repo_config.name.clone(),
        url: repo_config.url.clone(),
        local_path: repo_config.local_path.clone(),
        default_branch: repo_config.default_branch.clone(),
        active_branch,
        tracked_branches: repo_config.tracked_branches.clone(),
        filesystem_status,
        git_status,
        sync_status,
        indexed_languages: repo_config.indexed_languages.clone(),
        file_extensions,
        added_as_local_path: repo_config.added_as_local_path,
        target_ref: repo_config.target_ref.clone(),
        last_sync_time: None, // TODO: Could be extracted from metadata
        dependencies: repo_config.dependencies.clone(),
    })
}

/// Get filesystem status for a repository path
async fn get_filesystem_status(path: &Path) -> Result<FilesystemStatus> {
    use walkdir::WalkDir;
    
    let exists = path.exists();
    let accessible = exists && path.is_dir();
    
    // Check if it's a git repository
    let is_git_repository = if accessible {
        path.join(".git").exists()
    } else {
        false
    };
    
    let (total_files, size_bytes) = if accessible {
        let mut file_count = 0;
        let mut total_size = 0u64;
        
        // Walk through the directory and count files/sizes
        for entry in WalkDir::new(path)
            .into_iter()
            .filter_entry(|e| {
                // Skip hidden directories and common ignore patterns
                let name = e.file_name().to_string_lossy();
                !name.starts_with('.') && 
                name != "target" && 
                name != "node_modules" &&
                name != "__pycache__"
            })
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                file_count += 1;
                if let Ok(metadata) = entry.metadata() {
                    total_size += metadata.len();
                }
            }
        }
        
        (Some(file_count), Some(total_size))
    } else {
        (None, None)
    };
    
    Ok(FilesystemStatus {
        exists,
        accessible,
        is_git_repository,
        total_files,
        size_bytes,
    })
}

/// Get git repository status
async fn get_git_repository_status(path: &Path) -> Result<GitRepositoryStatus> {
    use git_manager::GitManager;
    
    let git_manager = GitManager::new();
    let repo_info = git_manager.get_repository_info(path)
        .map_err(|e| SagittaError::RepositoryError(format!("Failed to get git repository info: {e}")))?;
    
    // Get available branches
    let available_branches = git_manager.list_branches(path).unwrap_or_default();
    
    // Check if in detached HEAD state
    let is_detached_head = repo_info.current_branch.starts_with("detached-");
    
    // Extract current branch name (None if detached)
    let current_branch = if is_detached_head {
        None
    } else {
        Some(repo_info.current_branch.clone())
    };
    
    Ok(GitRepositoryStatus {
        current_commit: repo_info.current_commit,
        current_branch,
        is_clean: repo_info.is_clean,
        remote_url: repo_info.remote_url,
        available_branches,
        is_detached_head,
    })
}

/// Determine sync status for a repository
async fn get_sync_status(repo_config: &RepositoryConfig, git_status: Option<&GitRepositoryStatus>) -> Result<SyncStatus> {
    let mut branches_needing_sync = Vec::new();
    
    // Determine overall sync state
    let state = if let Some(git_status) = git_status {
        // Check new field first, fall back to deprecated field
        if let Some(last_synced_commit) = &repo_config.last_synced_commit {
            if last_synced_commit == &git_status.current_commit {
                SyncState::UpToDate
            } else {
                let active_branch = repo_config.active_branch.as_ref()
                    .unwrap_or(&repo_config.default_branch);
                branches_needing_sync.push(active_branch.clone());
                SyncState::NeedsSync
            }
        } else if !repo_config.last_synced_commits.is_empty() {
            // Fall back to deprecated field for backward compatibility
            let active_branch = repo_config.active_branch.as_ref()
                .unwrap_or(&repo_config.default_branch);
            
            if let Some(last_synced) = repo_config.last_synced_commits.get(active_branch) {
                if last_synced == &git_status.current_commit {
                    SyncState::UpToDate
                } else {
                    branches_needing_sync.push(active_branch.clone());
                    SyncState::NeedsSync
                }
            } else {
                branches_needing_sync.push(active_branch.clone());
                SyncState::NeedsSync
            }
        } else {
            SyncState::NeverSynced
        }
    } else {
        // Can't determine git status (e.g., no commits, corrupted repo, doesn't exist)
        if repo_config.last_synced_commit.is_some() || !repo_config.last_synced_commits.is_empty() {
            SyncState::Unknown
        } else {
            SyncState::NeverSynced
        }
    };
    
    Ok(SyncStatus {
        state,
        last_synced_commits: repo_config.last_synced_commits.clone(),
        branches_needing_sync,
        sync_in_progress: false, // TODO: Could be determined from running processes
    })
}

/// Get file extension statistics for a repository
async fn get_file_extension_stats(path: &Path) -> Result<Vec<FileExtensionInfo>> {
    use walkdir::WalkDir;
    use std::collections::HashMap;
    
    let mut extension_stats: HashMap<String, (usize, u64)> = HashMap::new();
    
    for entry in WalkDir::new(path)
        .into_iter()
        .filter_entry(|e| {
            // Skip hidden directories and common ignore patterns
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.') && 
            name != "target" && 
            name != "node_modules" &&
            name != "__pycache__"
        })
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let file_path = entry.path();
            let extension = file_path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("no_extension")
                .to_lowercase();
            
            let file_size = entry.metadata()
                .map(|m| m.len())
                .unwrap_or(0);
            
            let (count, size) = extension_stats.entry(extension).or_insert((0, 0));
            *count += 1;
            *size += file_size;
        }
    }
    
    let mut result: Vec<FileExtensionInfo> = extension_stats
        .into_iter()
        .map(|(extension, (count, size_bytes))| FileExtensionInfo {
            extension,
            count,
            size_bytes,
        })
        .collect();
    
    // Sort by count descending
    result.sort_by(|a, b| b.count.cmp(&a.count));
    
    Ok(result)
}

/// Scan the repository base path for directories that are not in the configuration
pub async fn scan_for_orphaned_repositories(config: &AppConfig) -> Result<Vec<OrphanedRepository>> {
    use config::get_repo_base_path;
    
    let base_path = get_repo_base_path(Some(config))?;
    log::debug!("Scanning for orphaned repositories in: {}", base_path.display());
    
    // If base path doesn't exist, return empty list
    if !base_path.exists() {
        log::debug!("Repository base path does not exist, no orphaned repos");
        return Ok(Vec::new());
    }
    
    // Get list of configured repository names
    let configured_names: std::collections::HashSet<String> = config.repositories
        .iter()
        .map(|r| r.name.clone())
        .collect();
    
    let mut orphaned_repos = Vec::new();
    
    // Read directories in base path
    if let Ok(entries) = std::fs::read_dir(&base_path) {
        for entry in entries.filter_map(|e| e.ok()) {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_dir() {
                    let dir_name = entry.file_name().to_string_lossy().to_string();
                    
                    // Skip if this directory is configured
                    if configured_names.contains(&dir_name) {
                        continue;
                    }
                    
                    log::debug!("Found potential orphaned directory: {dir_name}");
                    let dir_path = entry.path();
                    
                    // Check if it's a git repository
                    let is_git_repository = dir_path.join(".git").exists();
                    
                    // Try to get remote URL if it's a git repo (but don't wait too long)
                    let remote_url = if is_git_repository {
                        // Use a timeout to prevent hanging on slow git operations
                        tokio::time::timeout(
                            std::time::Duration::from_secs(2),
                            get_git_remote_url(&dir_path)
                        ).await.ok().and_then(|r| r.ok())
                    } else {
                        None
                    };
                    
                    // For orphaned repos, we don't need exact file count/size - just check if it's empty
                    let is_empty = dir_path.read_dir()
                        .map(|mut entries| entries.next().is_none())
                        .unwrap_or(true);
                    
                    let (file_count, size_bytes) = if is_empty {
                        (Some(0), Some(0))
                    } else {
                        // Skip detailed stats for now to avoid slow scanning
                        (None, None)
                    };
                    
                    orphaned_repos.push(OrphanedRepository {
                        name: dir_name,
                        local_path: dir_path,
                        is_git_repository,
                        remote_url,
                        file_count,
                        size_bytes,
                    });
                }
            }
        }
    }
    
    log::debug!("Found {} orphaned repositories", orphaned_repos.len());
    Ok(orphaned_repos)
}

/// Get git remote URL from a repository path
async fn get_git_remote_url(path: &Path) -> Result<String> {
    use git_manager::GitManager;
    
    let git_manager = GitManager::new();
    let repo_info = git_manager.get_repository_info(path)
        .map_err(|e| SagittaError::RepositoryError(format!("Failed to get git repository info: {e}")))?;
    
    repo_info.remote_url.ok_or_else(|| SagittaError::RepositoryError("No remote URL found".to_string()))
}


/// Reclone a missing repository
pub async fn reclone_missing_repository(
    config: &AppConfig,
    repository_name: &str,
) -> Result<()> {
    use config::get_repo_base_path;
    
    // Find the repository config
    let repo_config = config.repositories
        .iter()
        .find(|r| r.name == repository_name)
        .ok_or_else(|| SagittaError::RepositoryError(format!("Repository '{repository_name}' not found in config")))?;
    
    // Determine the local path
    let local_path = if repo_config.added_as_local_path {
        // For repos added as local path, we can't reclone
        return Err(SagittaError::RepositoryError(
            format!("Repository '{repository_name}' was added as a local path and cannot be recloned. Use 'add' to re-add it.")
        ));
    } else {
        // For cloned repos, use the standard path
        let base_path = get_repo_base_path(Some(config))?;
        base_path.join(&repo_config.name)
    };
    
    // Remove existing directory if it exists
    if local_path.exists() {
        std::fs::remove_dir_all(&local_path)
            .map_err(|e| SagittaError::RepositoryError(format!("Failed to remove existing directory: {e}")))?;
    }
    
    // Clone the repository using git2
    log::info!("Cloning repository {} from {}", repo_config.name, repo_config.url);
    
    // Create callbacks for authentication
    let mut callbacks = git2::RemoteCallbacks::new();
    
    // Setup SSH authentication if configured
    if let (Some(ssh_key_path), ssh_passphrase) = (&repo_config.ssh_key_path, &repo_config.ssh_key_passphrase) {
        callbacks.credentials(move |_url, username_from_url, _allowed_types| {
            git2::Cred::ssh_key(
                username_from_url.unwrap_or("git"),
                None,
                ssh_key_path,
                ssh_passphrase.as_deref(),
            )
        });
    } else {
        // Try default SSH keys
        callbacks.credentials(|_url, username_from_url, _allowed_types| {
            git2::Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
        });
    }
    
    // Setup fetch options
    let mut fetch_options = git2::FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);
    
    // Clone the repository
    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fetch_options);
    
    if let Some(target_ref) = &repo_config.target_ref {
        // If target_ref is specified, clone with that reference
        builder.branch(target_ref);
    } else if repo_config.default_branch != "main" && repo_config.default_branch != "master" {
        // Clone with specific branch
        builder.branch(&repo_config.default_branch);
    }
    
    builder.clone(&repo_config.url, &local_path)
        .map_err(|e| SagittaError::RepositoryError(format!("Failed to clone repository: {e}")))?;
    
    log::info!("Successfully recloned repository '{repository_name}'");
    
    Ok(())
}

/// Add an orphaned repository to configuration
pub async fn add_orphaned_repository(
    config: &mut AppConfig,
    orphaned_repo: &OrphanedRepository,
) -> Result<()> {
    // Use the directory name as the repository name
    let name = orphaned_repo.name.clone();
    
    // Use the remote URL if available, otherwise use a placeholder
    let url = orphaned_repo.remote_url
        .clone()
        .unwrap_or_else(|| format!("local://{}", orphaned_repo.local_path.display()));
    
    // Determine the default branch if it's a git repository
    let default_branch = if orphaned_repo.is_git_repository {
        match git2::Repository::open(&orphaned_repo.local_path) {
            Ok(repo) => {
                // Try to get current branch
                match repo.head() {
                    Ok(head) => {
                        if let Some(branch_name) = head.shorthand() {
                            branch_name.to_string()
                        } else {
                            "main".to_string()
                        }
                    }
                    Err(_) => "main".to_string(),
                }
            }
            Err(_) => "main".to_string(),
        }
    } else {
        "main".to_string()
    };
    
    // Create repository configuration
    #[allow(deprecated)]
    let repo_config = RepositoryConfig {
        name: name.clone(),
        url: url.clone(),
        local_path: orphaned_repo.local_path.clone(),
        default_branch: String::new(), // Deprecated field
        tracked_branches: Vec::new(), // Deprecated field
        remote_name: Some("origin".to_string()),
        active_branch: None, // Deprecated field
        ssh_key_path: None,
        ssh_key_passphrase: None,
        last_synced_commits: std::collections::HashMap::new(), // Deprecated field
        last_synced_commit: None,
        indexed_languages: None,
        added_as_local_path: true, // Mark as added from local path
        target_ref: None,
        dependencies: Vec::new(),
    };
    
    // Check if repository with same name already exists
    if config.repositories.iter().any(|r| r.name == name) {
        return Err(SagittaError::RepositoryError(format!("Repository with name '{name}' already exists")));
    }
    
    // Add to configuration
    config.repositories.push(repo_config);
    
    log::info!("Added orphaned repository '{name}' to configuration");
    
    Ok(())
}

/// Remove an orphaned repository from filesystem
pub async fn remove_orphaned_repository(
    orphaned_repo: &OrphanedRepository,
) -> Result<()> {
    use std::fs;
    
    // Safety check: ensure the path exists and is a directory
    if !orphaned_repo.local_path.exists() || !orphaned_repo.local_path.is_dir() {
        return Err(SagittaError::RepositoryError(
            format!("Directory '{}' does not exist or is not a directory", orphaned_repo.local_path.display())
        ));
    }
    
    // Remove the directory
    fs::remove_dir_all(&orphaned_repo.local_path)
        .map_err(|e| SagittaError::RepositoryError(
            format!("Failed to remove directory '{}': {}", orphaned_repo.local_path.display(), e)
        ))?;
    
    log::info!("Removed orphaned repository directory: {}", orphaned_repo.local_path.display());
    
    Ok(())
}