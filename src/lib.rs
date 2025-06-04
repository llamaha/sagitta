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
pub mod syntax;
/// Shared constants used across the library.
pub mod constants;
/// Code editing and validation logic.
pub mod edit;
/// Implementation of search functionality.
pub mod search_impl;
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

pub use config::{AppConfig, IndexingConfig, RepositoryConfig, EmbeddingEngineConfig, load_config, save_config, get_config_path_or_default, get_managed_repos_from_config};
// Re-export from sagitta-embed crate
pub use sagitta_embed::{EmbeddingModel, EmbeddingModelType, EmbeddingPool, EmbeddingProcessor};
// Re-export EmbeddingConfig for convenience
pub use sagitta_embed::config::EmbeddingConfig;
pub use error::{SagittaError, Result};
pub use qdrant_client_trait::QdrantClientTrait;
pub use constants::*;
pub use cache::EmbeddingCache; // Added cache re-export
pub use snippet_extractor::extract_snippet;

// Re-export core functionalities
// pub use indexing::index_paths; // Removed - indexing mod not directly exposed
pub use search_impl::search_collection; // Correctly export search_collection
pub use qdrant_ops::delete_all_points;

// Re-export other necessary items if needed by CLI directly
pub use edit::{apply_edit, validate_edit, EditTarget, EngineEditOptions, EngineValidationIssue, EngineValidationSeverity};
pub use repo_helpers::{delete_repository_data, get_collection_name, ensure_repository_collection_exists};
pub use repo_add::{handle_repo_add, AddRepoArgs, AddRepoError}; // Assuming repo_add is needed by CLI
pub use sync::{sync_repository, SyncOptions, SyncResult}; // Added sync re-export

// Re-export qdrant types needed by mcp or other crates
pub use qdrant_client::qdrant::{PointStruct, Filter, Condition, FieldCondition, Match, Range, PointsSelector, Value, Vectors, Vector, NamedVectors, ScoredPoint, SearchPoints, QueryPoints, QueryResponse, CollectionInfo, CountPoints, CountResponse, PointsOperationResponse, UpsertPoints, DeletePoints, CreateCollection, DeleteCollection, HealthCheckReply, Distance, VectorParams, VectorsConfig, SparseVectorParams, SparseVectorConfig, vectors_config, point_id::PointIdOptions, PointId, VectorParamsMap, HnswConfigDiff, OptimizersConfigDiff, WalConfigDiff, QuantizationConfig, ScalarQuantization, ProductQuantization, BinaryQuantization, /*quantization_config::Quantizer,*/ CompressionRatio, ListCollectionsResponse, CollectionDescription, AliasDescription, /*CollectionAliases,*/ ListAliasesRequest, /*UpdateCollectionAliases,*/ AliasOperations, CreateAlias, RenameAlias, DeleteAlias};

// Additional re-exports for enhanced repository functionality
pub use config::{get_config_path, ManagedRepositories};

use std::sync::Arc;
use async_trait::async_trait;

/// Basic addition function (example/placeholder).
pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

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
        app_config.onnx_model_path = Some("/path/to/model.onnx".to_string());
        app_config.onnx_tokenizer_path = Some("/path/to/tokenizer.json".to_string());
        app_config.tenant_id = Some("test-tenant".to_string());
        app_config.performance.vector_dimension = 512;
        
        // Set custom embedding configuration
        app_config.embedding = EmbeddingEngineConfig {
            max_sessions: 8,
            max_sequence_length: 256,
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
        assert_eq!(embedding_config.max_sessions, 8);
        assert_eq!(embedding_config.max_sequence_length, 256);
        assert_eq!(embedding_config.session_timeout_seconds, 600);
        assert_eq!(embedding_config.enable_session_cleanup, false);
        assert_eq!(embedding_config.tenant_id, Some("test-tenant".to_string()));
        assert_eq!(embedding_config.expected_dimension, Some(512));
        assert_eq!(embedding_config.embedding_batch_size, Some(64));
    }

    #[test]
    fn test_app_config_to_embedding_config_defaults() {
        // Test with default AppConfig
        let app_config = AppConfig::default();
        let embedding_config = app_config_to_embedding_config(&app_config);
        
        // Verify defaults are properly set
        assert_eq!(embedding_config.model_type, EmbeddingModelType::Default);
        assert_eq!(embedding_config.onnx_model_path, None);
        assert_eq!(embedding_config.onnx_tokenizer_path, None);
        assert_eq!(embedding_config.max_sessions, 4); // Default from EmbeddingEngineConfig
        assert_eq!(embedding_config.max_sequence_length, 128);
        assert_eq!(embedding_config.session_timeout_seconds, 300);
        assert_eq!(embedding_config.enable_session_cleanup, true);
        assert_eq!(embedding_config.tenant_id, None);
        assert_eq!(embedding_config.expected_dimension, Some(384)); // Default vector dimension
        assert_eq!(embedding_config.embedding_batch_size, Some(128)); // Default batch size
    }

    #[test]
    fn test_config_toml_to_decoupled_processing() {
        use sagitta_embed::processor::ProcessingConfig;
        
        // Simulate different max_sessions values that could be set in config.toml
        for max_sessions in [1, 2, 4, 8, 16] {
            let mut app_config = AppConfig::default();
            app_config.embedding.max_sessions = max_sessions;
            
            // Bridge to embedding config (this is what happens in practice)
            let embedding_config = app_config_to_embedding_config(&app_config);
            assert_eq!(embedding_config.max_sessions, max_sessions);
            
            // Create processing config for decoupled architecture
            let processing_config = ProcessingConfig::from_embedding_config(&embedding_config);
            
            // Verify that GPU memory control respects config.toml setting
            assert_eq!(processing_config.max_embedding_sessions, max_sessions,
                "max_embedding_sessions should respect config.toml [embedding].max_sessions = {}", 
                max_sessions);
        }
    }
}

#[macro_use]
extern crate log;

/// Helper function to convert AppConfig to EmbeddingConfig for the new sagitta-embed crate
pub fn app_config_to_embedding_config(app_config: &AppConfig) -> EmbeddingConfig {
    // Determine the model type based on the paths provided
    let model_type = if app_config.onnx_model_path.is_some() && app_config.onnx_tokenizer_path.is_some() {
        EmbeddingModelType::Onnx
    } else {
        EmbeddingModelType::Default
    };

    EmbeddingConfig {
        model_type,
        onnx_model_path: app_config.onnx_model_path.as_ref().map(std::path::PathBuf::from),
        onnx_tokenizer_path: app_config.onnx_tokenizer_path.as_ref().map(std::path::PathBuf::from),
        max_sessions: app_config.embedding.max_sessions,
        max_sequence_length: app_config.embedding.max_sequence_length,
        session_timeout_seconds: app_config.embedding.session_timeout_seconds,
        enable_session_cleanup: app_config.embedding.enable_session_cleanup,
        tenant_id: app_config.tenant_id.clone(),
        expected_dimension: Some(app_config.performance.vector_dimension as usize),
        embedding_batch_size: Some(app_config.embedding.embedding_batch_size),
        ..Default::default()
    }
}

/// Creates an EmbeddingPool that properly respects the max_sessions configuration.
/// This provides GPU memory control through session pooling.
/// 
/// # Arguments
/// * `app_config` - The application configuration containing embedding settings
/// 
/// # Returns
/// * `Result<Arc<EmbeddingPool>>` - A thread-safe embedding pool that respects max_sessions
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

/// Creates an EmbeddingPool from an EmbeddingConfig that properly respects max_sessions.
/// This is a convenience function for cases where you already have an EmbeddingConfig.
/// 
/// # Arguments
/// * `embedding_config` - The embedding configuration
/// 
/// # Returns
/// * `Result<Arc<EmbeddingPool>>` - A thread-safe embedding pool that respects max_sessions
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
            id: format!("text_{}", i),
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
    Ok(results.into_iter().next().unwrap())
}

/// Adapter that implements EmbeddingProvider for EmbeddingPool.
/// This bridges the interface gap between the old EmbeddingHandler and new EmbeddingPool.
#[derive(Debug)]
pub struct EmbeddingPoolAdapter {
    pool: Arc<EmbeddingPool>,
}

impl EmbeddingPoolAdapter {
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
    /// Tenant ID
    pub tenant_id: Option<String>,
    /// Last sync timestamp if available
    pub last_sync_time: Option<chrono::DateTime<chrono::Utc>>,
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
    };
    
    // Collect extension statistics across all repositories
    let mut all_extensions: std::collections::HashMap<String, FileExtensionInfo> = std::collections::HashMap::new();
    
    for repo_config in &config.repositories {
        let enhanced_info = get_enhanced_repository_info(repo_config).await?;
        
        // Update summary statistics
        if enhanced_info.filesystem_status.exists {
            summary.existing_count += 1;
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
    
    // Sort extensions by count and take top 10
    let mut sorted_extensions: Vec<_> = all_extensions.into_values().collect();
    sorted_extensions.sort_by(|a, b| b.count.cmp(&a.count));
    summary.common_extensions = sorted_extensions.into_iter().take(10).collect();
    
    Ok(EnhancedRepositoryList {
        repositories: enhanced_repos,
        active_repository: config.active_repository.clone(),
        total_count: config.repositories.len(),
        summary,
    })
}

/// Get enhanced information for a single repository
pub async fn get_enhanced_repository_info(repo_config: &RepositoryConfig) -> Result<EnhancedRepositoryInfo> {
    use std::path::Path;
    
    // Check filesystem status
    let filesystem_status = get_filesystem_status(&repo_config.local_path).await?;
    
    // Get git status if it's a git repository
    let git_status = if filesystem_status.is_git_repository {
        get_git_repository_status(&repo_config.local_path).await.ok()
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
    
    // Determine the current active branch
    let active_branch = if let Some(git_status) = &git_status {
        if git_status.is_detached_head {
            None // Don't show branch name for detached HEAD
        } else {
            Some(git_status.current_commit.clone())
        }
    } else {
        repo_config.active_branch.clone()
    };
    
    Ok(EnhancedRepositoryInfo {
        name: repo_config.name.clone(),
        url: repo_config.url.clone(),
        local_path: repo_config.local_path.clone(),
        default_branch: repo_config.default_branch.clone(),
        active_branch: repo_config.active_branch.clone(),
        tracked_branches: repo_config.tracked_branches.clone(),
        filesystem_status,
        git_status,
        sync_status,
        indexed_languages: repo_config.indexed_languages.clone(),
        file_extensions,
        added_as_local_path: repo_config.added_as_local_path,
        target_ref: repo_config.target_ref.clone(),
        tenant_id: repo_config.tenant_id.clone(),
        last_sync_time: None, // TODO: Could be extracted from metadata
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
        .map_err(|e| SagittaError::RepositoryError(format!("Failed to get git repository info: {}", e)))?;
    
    // Get available branches
    let available_branches = git_manager.list_branches(path).unwrap_or_default();
    
    // Check if in detached HEAD state
    let is_detached_head = repo_info.current_branch.starts_with("detached-");
    
    Ok(GitRepositoryStatus {
        current_commit: repo_info.current_commit,
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
    let state = if repo_config.last_synced_commits.is_empty() {
        SyncState::NeverSynced
    } else if let Some(git_status) = git_status {
        // Check if current commit matches last synced commit for active branch
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
        SyncState::Unknown
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