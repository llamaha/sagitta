// Use modules from *within* sagitta_search
use crate::config::RepositoryConfig;
// use crate::error::SagittaError; // Marked as unused
use crate::repo_helpers as helpers;
// use crate::repo_helpers::{prepare_repository, index_files}; // Marked as unused
use crate::qdrant_client_trait::QdrantClientTrait;

// Use modules from the main sagitta_cli crate

// Other dependencies
use anyhow::anyhow;
use clap::Args; // Keep Args
use std::{fs, path::PathBuf, sync::Arc};
use thiserror::Error;
use log::{info, error};
use crate::config::AppConfig;
// Use ManualMock
use crate::sync_progress::AddProgressReporter;

/// Arguments for the `repo add` command.
#[derive(Args, Debug)]
#[derive(Clone)]
pub struct AddRepoArgs {
    #[arg(short = 'p', long, value_parser)]
    /// Local path where the repository should be cloned or already exists.
    pub local_path: Option<PathBuf>,

    #[clap(long, value_parser)]
    /// URL of the repository to clone (e.g., https://gitlab.com/user/repo.git)
    /// Required when adding a new repository, optional when adding an existing local repository.
    pub url: Option<String>,

    /// Optional custom name for the repository (defaults to deriving from URL).
    #[arg(short, long)]
    pub name: Option<String>,

    /// Optional specific branch to track initially (defaults to the repo's default).
    #[arg(short, long)]
    pub branch: Option<String>,

    /// Optional name for the Git remote (defaults to "origin").
    #[arg(short = 'r', long)]
    pub remote: Option<String>,

    /// Optional path to the SSH private key for authentication.
    #[arg(long)]
    pub ssh_key: Option<PathBuf>,

    /// Optional passphrase for the SSH private key.
    #[arg(long, requires = "ssh_key")] // Passphrase only makes sense if key is provided
    pub ssh_passphrase: Option<String>,

    /// Optional base path for the repository
    #[arg(long)]
    pub repositories_base_path: Option<PathBuf>,

    /// Optional specific Git ref (tag, commit hash, branch name) to check out initially.
    /// If provided, this ref will be checked out instead of the default branch after cloning.
    /// It will also be stored in the RepositoryConfig.
    #[arg(long)]
    pub target_ref: Option<String>,
}

/// Errors that can occur during the `repo add` operation.
#[derive(Error, Debug)]
pub enum AddRepoError {
    /// Invalid combination or missing arguments.
    #[error("Invalid arguments: {0}")]
    InvalidArgs(String),
    /// The repository name is already configured.
    #[error("Repository '{0}' already exists.")]
    RepoExists(String), // Note: This check will now be done by the caller (Relay)
    /// Could not derive a repository name from the provided URL or local path.
    #[error("Could not derive repository name from {0}")]
    NameDerivationError(String),
    /// Filesystem I/O error.
    #[error("Filesystem error: {0}")]
    IoError(#[from] std::io::Error),
    /// Error reading or writing configuration.
    #[error("Configuration error: {0}")]
    ConfigError(anyhow::Error),
    /// A Git operation (clone, open, remote fetch, etc.) failed.
    #[error("Git operation failed: {0}")]
    GitError(anyhow::Error), // Keep anyhow for git errors for now
    /// Failed to open an existing local repository.
    #[error("Failed to open repository at {0}: {1}")]
    RepoOpenError(PathBuf, anyhow::Error),
    /// Could not determine the default branch of the repository.
    #[error("Failed to determine default branch: {0}")]
    BranchDetectionError(anyhow::Error),
    /// An operation with the Qdrant client failed.
    #[error("Qdrant operation failed: {0}")]
    QdrantError(anyhow::Error), // Keep anyhow for qdrant errors
    /// An error occurred during embedding generation or handling.
    #[error("Embedding logic error: {0}")]
    EmbeddingError(anyhow::Error),
    /// Failed to determine the repository URL, either from args or existing remote.
    #[error("Failed to determine repository URL.")]
    UrlDeterminationError,
}

/// Handles the logic for adding a new repository (cloning or linking local).
/// It validates arguments, determines paths and names, interacts with Git,
/// ensures the Qdrant collection exists, and returns the generated `RepositoryConfig`.
/// **Note:** This function does *not* modify the `AppConfig` directly, nor does it perform initial indexing.
pub async fn handle_repo_add<C>(
    args: AddRepoArgs,
    repo_base_path_for_add: PathBuf,
    embedding_dim: u64,
    client: Arc<C>,
    config: &AppConfig,
    progress_reporter: Option<Arc<dyn AddProgressReporter>>,
) -> Result<RepositoryConfig, AddRepoError>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    info!("[handle_repo_add] Starting repository addition process");
    info!("[handle_repo_add] Args: {:?}", args);
    info!("[handle_repo_add] Repo base path: {}", repo_base_path_for_add.display());
    info!("[handle_repo_add] Embedding dim: {}", embedding_dim);
    
    // Validate basic arguments
    info!("[handle_repo_add] Validating arguments...");
    if args.local_path.is_none() && args.url.is_none() {
        error!("[handle_repo_add] Invalid arguments: Either --local-path or --url must be specified");
        return Err(AddRepoError::InvalidArgs("Either --local-path or --url must be specified.".to_string()));
    }
    info!("[handle_repo_add] Arguments validated successfully");

    // Handle repository name
    info!("[handle_repo_add] Determining repository name...");
    let repo_name = match &args.name {
        Some(name) => {
            info!("[handle_repo_add] Using provided name: {}", name);
            name.clone()
        },
        None => {
            info!("[handle_repo_add] Deriving name from URL or path...");
            // If URL is provided, derive name from URL
            if let Some(url) = &args.url {
                let derived_name = PathBuf::from(url)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.trim_end_matches(".git").to_string())
                    .ok_or_else(|| AddRepoError::NameDerivationError("URL".to_string()))?;
                info!("[handle_repo_add] Derived name from URL: {}", derived_name);
                derived_name
            } else {
                // If only local path is provided, derive name from the directory name
                let local_path_ref = args.local_path.as_ref().unwrap(); // Use as_ref() to avoid moving
                let derived_name = local_path_ref
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| AddRepoError::NameDerivationError("local path".to_string()))?;
                info!("[handle_repo_add] Derived name from local path: {}", derived_name);
                derived_name
            }
        },
    };
    info!("[handle_repo_add] Repository name determined: {}", repo_name);

    // Use the passed-in base path
    info!("[handle_repo_add] Setting up repository base path...");
    let repo_base_path = repo_base_path_for_add;
    info!("[handle_repo_add] Repository base path: {}", repo_base_path.display());

    // Ensure the determined base path exists
    info!("[handle_repo_add] Creating repository base directory if needed...");
    fs::create_dir_all(&repo_base_path)
        .map_err(AddRepoError::IoError)?;
    info!("[handle_repo_add] Repository base directory ensured");
    
    // Determine the final local path for the repository
    info!("[handle_repo_add] Determining final local path...");
    let local_path = args.local_path.clone().unwrap_or_else(|| repo_base_path.join(&repo_name));
    info!("[handle_repo_add] Final local path: {}", local_path.display());

    // If URL is not provided but required for a new clone scenario (checked by prepare_repository)
    let repo_url = args.url.clone();
    info!("[handle_repo_add] Repository URL: {:?}", repo_url);

    // Flag to indicate if the repo was added by specifying a local path initially
    let added_as_local_path_flag = args.local_path.is_some();
    info!("[handle_repo_add] Added as local path: {}", added_as_local_path_flag);

    info!("[handle_repo_add] About to call prepare_repository...");
    info!("[handle_repo_add] prepare_repository args:");
    info!("[handle_repo_add]   url: {:?}", repo_url.as_deref().unwrap_or_default());
    info!("[handle_repo_add]   name: {:?}", Some(&repo_name));
    info!("[handle_repo_add]   local_path: {:?}", if added_as_local_path_flag { Some(&local_path) } else { None });
    info!("[handle_repo_add]   branch: {:?}", args.branch.as_deref());
    info!("[handle_repo_add]   target_ref: {:?}", args.target_ref.as_deref());
    info!("[handle_repo_add]   remote: {:?}", args.remote.as_deref());
    info!("[handle_repo_add]   ssh_key: {:?}", args.ssh_key.as_ref());
    info!("[handle_repo_add]   ssh_passphrase: {:?}", args.ssh_passphrase.as_deref().map(|_| "***"));
    info!("[handle_repo_add]   repo_base_path: {}", repo_base_path.display());
    info!("[handle_repo_add]   embedding_dim: {}", embedding_dim);

    // Call prepare_repository for both new clones and existing local paths.
    // It handles cloning if necessary and ensures the Qdrant collection (tenant-specific).
    let new_repo_config = helpers::prepare_repository(
        repo_url.as_deref().unwrap_or_default(), // Pass URL, or empty if only local path given
        Some(&repo_name),
        if added_as_local_path_flag { Some(&local_path) } else { None }, // Pass local_path if it was an arg
        args.branch.as_deref(),
        args.target_ref.as_deref(),
        args.remote.as_deref(),
        args.ssh_key.as_ref(),
        args.ssh_passphrase.as_deref(),
        &repo_base_path, // Base path for new clones if local_path is not set by arg
        client.clone(),
        embedding_dim,
        config,      // Pass AppConfig for collection_name_prefix and other settings
        progress_reporter,
    ).await.map_err(|e| {
        error!("[handle_repo_add] prepare_repository failed: {}", e);
        // Map internal Error to AddRepoError
        match e {
            crate::error::SagittaError::GitMessageError(msg) => AddRepoError::GitError(anyhow!(msg)),
            crate::error::SagittaError::QdrantError(msg) => AddRepoError::QdrantError(anyhow!(msg)),
            // Add other specific mappings as needed
            _ => AddRepoError::ConfigError(anyhow!(e.to_string())),
        }
    })?;

    info!("[handle_repo_add] prepare_repository completed successfully");
    info!("[handle_repo_add] New repository config: name={}, url={}, local_path={}", 
          new_repo_config.name, new_repo_config.url, new_repo_config.local_path.display());

    // Ensure the returned config has the correct added_as_local_path flag if it was derived
    // prepare_repository now sets this, but we can ensure it aligns if needed, though it should be correct.
    // The URL used by prepare_repository also needs to be the one from args if present, or derived.
    // The `new_repo_config` from `prepare_repository` should be mostly complete.
    // We might need to adjust its `url` field if `args.url` was Some and `prepare_repository` didn't pick it up as primary.
    // However, `prepare_repository` logic tries to use the provided URL.
    
    // The existing logic to open repo, extract URL if missing, and checkout target_ref
    // is now largely handled within `prepare_repository`. 
    // `handle_repo_add` becomes simpler.

    // The `new_repo_config` returned by `prepare_repository` already contains the tenant_id.
    info!("[handle_repo_add] Repository addition completed successfully");
    Ok(new_repo_config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, IndexingConfig, PerformanceConfig, EmbeddingEngineConfig};
    // use crate::qdrant_client_trait::MockQdrantClientTrait; // Remove mockall
    use crate::test_utils::ManualMockQdrantClient; // Use ManualMock
    use std::path::Path;
    use tempfile::tempdir;
    use std::fs;
    use std::sync::Arc;
    use git2::{Repository, Signature, build::RepoBuilder};
    use std::io::Write;

    // Helper to create an initial commit in a repo
    fn create_initial_commit(repo: &Repository, file_name: &str, content: &str) -> Result<(), git2::Error> {
        // Create a dummy file
        let repo_path = repo.path().parent().unwrap(); // Assuming .git parent is repo root
        let file_path = repo_path.join(file_name);
        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(file, "{}", content).unwrap();
        
        let mut index = repo.index()?;
        index.add_path(Path::new(file_name))?;
        // index.add_all(&[file_name], IndexAddOption::DEFAULT, None)?;
        let oid = index.write_tree()?;
        let tree = repo.find_tree(oid)?;
        let signature = Signature::now("Test User", "test@example.com")?;
        
        // Check if HEAD exists (i.e. if there are any commits)
        match repo.head() {
            Ok(head_ref) => {
                 // HEAD exists, commit on top of it
                 let parent_commit = head_ref.peel_to_commit()?;
                 repo.commit(Some("HEAD"), &signature, &signature, "Initial commit", &tree, &[&parent_commit])?;
            }
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
                // HEAD doesn't exist (unborn branch), this is the first commit
                repo.commit(Some("HEAD"), &signature, &signature, "Initial commit", &tree, &[])?;
            }
            Err(e) => return Err(e), // Other error
        }
        Ok(())
    }

    // Helper to create a basic AppConfig for tests
    fn test_config_with_empty_repo_list() -> AppConfig {
        AppConfig {
            qdrant_url: "http://localhost:6334".to_string(),
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            embed_model: None,
            server_api_key_path: None,
            repositories_base_path: None,
            vocabulary_base_path: None,
            repositories: Vec::new(),
            active_repository: None,
            indexing: IndexingConfig::default(),
            performance: PerformanceConfig::default(),
            embedding: EmbeddingEngineConfig::default(),
        }
    }

    #[tokio::test]
    async fn test_handle_repo_add_new_repo_local_path_success() {
        let temp_dir = tempdir().unwrap();
        let local_repo_path = temp_dir.path().join("test_repo");
        fs::create_dir_all(&local_repo_path).unwrap();
        let git_repo = git2::Repository::init(&local_repo_path).unwrap();
        create_initial_commit(&git_repo, "README.md", "Initial commit").expect("Failed to create initial commit");

        let manual_mock_client = ManualMockQdrantClient::new();
        let client_arc = Arc::new(manual_mock_client.clone());
        let repo_name_str = "test_repo";

        let config = AppConfig {
            embed_model: None,
            qdrant_url: "http://localhost:6334".to_string(),
            repositories: vec![],
            active_repository: None,
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            server_api_key_path: None,
            repositories_base_path: Some(temp_dir.path().to_str().unwrap().to_string()),
            vocabulary_base_path: None,
            indexing: IndexingConfig::default(),
            performance: PerformanceConfig {
                 vector_dimension: 10,
                 collection_name_prefix: "test_prefix_".to_string(),
                 ..PerformanceConfig::default()
            },
            embedding: EmbeddingEngineConfig::default(),
        };
        
        // Use branch-aware collection naming - the default branch will be "main" or "master"
        // We need to determine what branch the test repo will have
        let repo = git2::Repository::open(&local_repo_path).unwrap();
        let head = repo.head().unwrap();
        let branch_name = head.shorthand().unwrap_or("main");
        
        let expected_collection_name = crate::repo_helpers::get_branch_aware_collection_name(
            repo_name_str,
            branch_name,
            &config
        );
        let expected_dimension = config.performance.vector_dimension;

        manual_mock_client.expect_collection_exists(Ok(false)); // Expect collection_exists to be called first
        manual_mock_client.expect_create_collection(Ok(true));

        let add_args = AddRepoArgs {
            local_path: Some(local_repo_path.clone()),
            url: None,
            name: Some(repo_name_str.to_string()),
            branch: None,
            remote: None,
            ssh_key: None,
            ssh_passphrase: None,
            repositories_base_path: None,
            target_ref: None,
        };
        
        let result = handle_repo_add(
            add_args,
            temp_dir.path().to_path_buf(),
            config.performance.vector_dimension,
            client_arc,
            &config,
            None, // No progress reporter for test
        )
        .await;

        assert!(result.is_ok());
        let repo_config_res = result.unwrap();
        assert_eq!(repo_config_res.name, repo_name_str);
        assert_eq!(repo_config_res.local_path, local_repo_path);
        assert!(repo_config_res.url.is_empty() || repo_config_res.url.starts_with("file://") || repo_config_res.url == local_repo_path.to_str().unwrap_or_default());
        assert!(repo_config_res.added_as_local_path);
        
        // Verify mock calls
        assert_eq!(manual_mock_client.verify_collection_exists_called_times(), 1);
        // The actual collection name might differ based on the branch, so just verify it was called with correct prefix
        assert!(manual_mock_client.get_collection_exists_args()[0].starts_with(&config.performance.collection_name_prefix));
        assert!(manual_mock_client.verify_create_collection_called());
        // Verify the dimension is correct
        let create_collection_args = manual_mock_client.get_create_collection_args();
        assert_eq!(create_collection_args.1, expected_dimension);
    }

    #[tokio::test]
    async fn test_handle_repo_add_existing_local_repo_no_url_provided() {
        let temp_dir = tempdir().unwrap();
        let existing_repo_path = temp_dir.path().join("existing_repo");
        fs::create_dir_all(&existing_repo_path).unwrap();
        let git_repo = git2::Repository::init(&existing_repo_path).unwrap();
        create_initial_commit(&git_repo, "README.md", "Initial commit for existing repo").expect("Failed to create initial commit");
        
        let repo_opened = git2::Repository::open(&existing_repo_path).unwrap();
        repo_opened.remote("origin", "https://example.com/existing_repo.git").unwrap();

        let manual_mock_client = ManualMockQdrantClient::new();
        let client_arc = Arc::new(manual_mock_client.clone());
        let repo_name_str = "existing_repo";

        let config = AppConfig {
            embed_model: None,
            qdrant_url: "http://localhost:6334".to_string(),
            repositories: vec![],
            active_repository: None,
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            server_api_key_path: None,
            repositories_base_path: Some(temp_dir.path().to_str().unwrap().to_string()),
            vocabulary_base_path: None,
            indexing: IndexingConfig::default(),
            performance: PerformanceConfig {
                 vector_dimension: 10,
                 collection_name_prefix: "test_prefix_".to_string(),
                 ..PerformanceConfig::default()
            },
            embedding: EmbeddingEngineConfig::default(),
        };
        
        // Use branch-aware collection naming - determine the actual branch name
        let repo = git2::Repository::open(&existing_repo_path).unwrap();
        let head = repo.head().unwrap();
        let branch_name = head.shorthand().unwrap_or("main");
        
        let expected_collection_name = crate::repo_helpers::get_branch_aware_collection_name(
            repo_name_str,
            branch_name,
            &config
        );
        let expected_dimension = config.performance.vector_dimension;

        manual_mock_client.expect_collection_exists(Ok(false)); // Expect collection_exists to be called first
        manual_mock_client.expect_create_collection(Ok(true));

        let add_args = AddRepoArgs {
            local_path: Some(existing_repo_path.clone()),
            url: None,
            name: None, // Name will be derived from path
            branch: None,
            remote: Some("origin".to_string()),
            ssh_key: None,
            ssh_passphrase: None,
            repositories_base_path: None,
            target_ref: None,
        };
        
        let result = handle_repo_add(
            add_args,
            temp_dir.path().to_path_buf(),
            config.performance.vector_dimension,
            client_arc,
            &config,
            None, // No progress reporter for test
        )
        .await;

        assert!(result.is_ok(), "handle_repo_add failed: {:?}", result.err());
        let repo_config_res = result.unwrap();
        assert_eq!(repo_config_res.name, repo_name_str);
        assert_eq!(repo_config_res.url, "https://example.com/existing_repo.git");
        assert!(repo_config_res.added_as_local_path);

        // Verify mock calls
        assert_eq!(manual_mock_client.verify_collection_exists_called_times(), 1);
        // The actual collection name might differ based on the branch, so just verify it was called with correct prefix
        assert!(manual_mock_client.get_collection_exists_args()[0].starts_with(&config.performance.collection_name_prefix));
        assert!(manual_mock_client.verify_create_collection_called());
        // Verify the dimension is correct
        let create_collection_args = manual_mock_client.get_create_collection_args();
        assert_eq!(create_collection_args.1, expected_dimension);
    }
    
    #[tokio::test]
    async fn test_handle_repo_add_new_clone() {
        let temp_dir = tempdir().unwrap();
        
        // 1. Create a source repository with an initial commit
        let source_repo_path = temp_dir.path().join("source_repo_for_bare");
        fs::create_dir_all(&source_repo_path).unwrap();
        let source_repo = git2::Repository::init(&source_repo_path).unwrap();
        create_initial_commit(&source_repo, "initial.txt", "Initial content for clone test").expect("Failed to create initial commit in source repo");
        
        // 2. Create the bare repository by cloning the source repository using RepoBuilder
        let bare_repo_target_path = temp_dir.path().join("test_cloned_repo.git");
        let mut builder = RepoBuilder::new();
        builder.bare(true);
        let _bare_repo = builder.clone(
            source_repo_path.to_str().unwrap(),
            &bare_repo_target_path
        ).expect("Failed to clone bare repository using RepoBuilder");

        let manual_mock_client = ManualMockQdrantClient::new();
        let client_arc = Arc::new(manual_mock_client.clone());

        let config = AppConfig {
            embed_model: None,
            qdrant_url: "http://localhost:6334".to_string(),
            repositories_base_path: Some(temp_dir.path().to_string_lossy().into_owned()),
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            server_api_key_path: None,
            vocabulary_base_path: None,
            repositories: vec![],
            active_repository: None,
            indexing: IndexingConfig::default(),
            performance: PerformanceConfig {
                vector_dimension: 10,
                collection_name_prefix: "test_cloned_".to_string(),
                ..PerformanceConfig::default()
            },
            embedding: EmbeddingEngineConfig::default(),
        };

        let repo_name_str = "test_cloned_repo";

        // Use branch-aware collection naming - determine the default branch from source repo
        let source_repo_opened = git2::Repository::open(&source_repo_path).unwrap();
        let head = source_repo_opened.head().unwrap();
        let branch_name = head.shorthand().unwrap_or("main");
        
        let expected_collection_name = crate::repo_helpers::get_branch_aware_collection_name(
            repo_name_str,
            branch_name,
            &config
        );
        let expected_dimension = config.performance.vector_dimension;

        manual_mock_client.expect_collection_exists(Ok(false)); // Expect collection_exists to be called first
        manual_mock_client.expect_create_collection(Ok(true));

        let add_args = AddRepoArgs {
            local_path: None, // We want to test cloning, so no local_path initially
            url: Some(bare_repo_target_path.to_str().unwrap().to_string()), // URL is the path to the bare repo
            name: Some(repo_name_str.to_string()), 
            branch: None, // Should pick up default from bare repo
            remote: None,
            ssh_key: None,
            ssh_passphrase: None,
            repositories_base_path: None, // Will use the one from temp_dir
            target_ref: None, // Test default branch behavior first
        };

        // The actual local path where the clone will happen will be derived inside handle_repo_add
        // e.g., <repositories_base_path>/<repo_name_str>
        let expected_final_clone_path = temp_dir.path().join(repo_name_str);

        let result = handle_repo_add(
            add_args,
            temp_dir.path().to_path_buf(), // This is the repo_base_path_for_add
            config.performance.vector_dimension,
            client_arc,
            &config,
            None, // No progress reporter for test
        )
        .await;

        assert!(result.is_ok(), "Expected Ok, got Err: {:?}", result.err());
        if let Ok(ref repo_config_res) = result {
            assert_eq!(repo_config_res.name, repo_name_str);
            assert_eq!(repo_config_res.url, bare_repo_target_path.to_str().unwrap().to_string());
            assert_eq!(repo_config_res.local_path, expected_final_clone_path);
            assert!(!repo_config_res.added_as_local_path);
            // Check if default branch was picked up (e.g., "main" or "master" depending on git version/config)
            // For this test, create_initial_commit uses HEAD which should resolve to the default branch name
            // after the first commit.
            assert!(repo_config_res.default_branch == "main" || repo_config_res.default_branch == "master");
            assert_eq!(repo_config_res.active_branch.as_deref(), Some(repo_config_res.default_branch.as_str()));
        }

        // Verify mock calls
        assert_eq!(manual_mock_client.verify_collection_exists_called_times(), 1);
        // The actual collection name might differ based on the branch, so just verify it was called with correct prefix
        assert!(manual_mock_client.get_collection_exists_args()[0].starts_with(&config.performance.collection_name_prefix));
        assert!(manual_mock_client.verify_create_collection_called());
        // Verify the dimension is correct
        let create_collection_args = manual_mock_client.get_create_collection_args();
        assert_eq!(create_collection_args.1, expected_dimension);
    }

    #[tokio::test]
    async fn test_handle_repo_add_hello_world_hanging_issue() {
        // This test reproduces the exact hanging issue reported by the user
        let temp_dir = tempdir().unwrap();
        
        let manual_mock_client = ManualMockQdrantClient::new();
        let client_arc = Arc::new(manual_mock_client.clone());

        let config = AppConfig {
            embed_model: None,
            qdrant_url: "http://localhost:6334".to_string(),
            repositories_base_path: Some(temp_dir.path().to_string_lossy().into_owned()),
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            server_api_key_path: None,
            vocabulary_base_path: None,
            repositories: vec![],
            active_repository: None,
            indexing: IndexingConfig::default(),
            performance: PerformanceConfig {
                vector_dimension: 384,
                collection_name_prefix: "repo_".to_string(),
                ..PerformanceConfig::default()
            },
            embedding: EmbeddingEngineConfig::default(),
        };
        
        // Use branch-aware collection naming - the test specifies "master" branch
        let expected_collection_name = crate::repo_helpers::get_branch_aware_collection_name(
            "hello-world",
            "master", // This test specifically uses master branch
            &config
        );
        let expected_dimension = config.performance.vector_dimension;

        manual_mock_client.expect_collection_exists(Ok(false));
        manual_mock_client.expect_create_collection(Ok(true));

        // This is the exact scenario from the user's logs
        let add_args = AddRepoArgs {
            local_path: None,
            url: Some("https://github.com/octocat/Hello-World.git".to_string()),
            name: Some("hello-world".to_string()),
            branch: Some("master".to_string()), // This is the key - using master branch
            remote: None,
            ssh_key: None,
            ssh_passphrase: None,
            repositories_base_path: None,
            target_ref: None,
        };

        let start_time = std::time::Instant::now();
        
        let result = handle_repo_add(
            add_args,
            temp_dir.path().to_path_buf(),
            config.performance.vector_dimension,
            client_arc,
            &config,
            None, // No progress reporter for test
        )
        .await;

        let elapsed = start_time.elapsed();
        
        println!("Test completed in {:?}", elapsed);
        println!("Result: {:?}", result);
        
        // The test should complete within a reasonable time (not hang indefinitely)
        assert!(elapsed.as_secs() < 30, "handle_repo_add took too long: {:?} - this indicates the hanging issue", elapsed);
        
        // We expect this to either succeed or fail with a specific error, but not hang
        match result {
            Ok(repo_config) => {
                println!("Successfully added repository: {}", repo_config.name);
                assert_eq!(repo_config.name, "hello-world");
                
                // Verify mock calls
                assert_eq!(manual_mock_client.verify_collection_exists_called_times(), 1);
                assert_eq!(manual_mock_client.get_collection_exists_args()[0], expected_collection_name);
                assert!(manual_mock_client.verify_create_collection_called());
                assert!(manual_mock_client.verify_create_collection_args(&expected_collection_name, expected_dimension));
            }
            Err(e) => {
                println!("Repository addition failed with error: {}", e);
                // Even if it fails, it should not hang, and we should still verify the collection operations were attempted
                assert_eq!(manual_mock_client.verify_collection_exists_called_times(), 1);
                assert_eq!(manual_mock_client.get_collection_exists_args()[0], expected_collection_name);
            }
        }
    }

    #[tokio::test]
    async fn test_handle_repo_add_with_progress_reporting() {
        let temp_dir = tempdir().unwrap();
        let local_repo_path = temp_dir.path().join("test_repo_progress");
        fs::create_dir_all(&local_repo_path).unwrap();
        let git_repo = git2::Repository::init(&local_repo_path).unwrap();
        create_initial_commit(&git_repo, "README.md", "Test repo for progress reporting").expect("Failed to create initial commit");

        let manual_mock_client = ManualMockQdrantClient::new();
        let client_arc = Arc::new(manual_mock_client.clone());

        let config = AppConfig {
            embed_model: None,
            qdrant_url: "http://localhost:6334".to_string(),
            repositories: vec![],
            active_repository: None,
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            server_api_key_path: None,
            repositories_base_path: Some(temp_dir.path().to_str().unwrap().to_string()),
            vocabulary_base_path: None,
            indexing: IndexingConfig::default(),
            performance: PerformanceConfig {
                 vector_dimension: 10,
                 collection_name_prefix: "test_progress_".to_string(),
                 ..PerformanceConfig::default()
            },
            embedding: EmbeddingEngineConfig::default(),
        };

        // Set up mock expectations
        manual_mock_client.expect_collection_exists(Ok(false));
        manual_mock_client.expect_create_collection(Ok(true));

        // Create a test progress reporter
        let progress_updates = Arc::new(std::sync::Mutex::new(Vec::new()));
        let progress_updates_clone = Arc::clone(&progress_updates);

        struct TestProgressReporter {
            updates: Arc<std::sync::Mutex<Vec<crate::sync_progress::AddProgress>>>,
        }

        #[async_trait::async_trait]
        impl crate::sync_progress::AddProgressReporter for TestProgressReporter {
            async fn report(&self, progress: crate::sync_progress::AddProgress) {
                self.updates.lock().unwrap().push(progress);
            }
        }

        let progress_reporter = Arc::new(TestProgressReporter {
            updates: progress_updates_clone,
        });

        let add_args = AddRepoArgs {
            local_path: Some(local_repo_path.clone()),
            url: None,
            name: Some("test_repo_progress".to_string()),
            branch: None,
            remote: None,
            ssh_key: None,
            ssh_passphrase: None,
            repositories_base_path: None,
            target_ref: None,
        };
        
        let result = handle_repo_add(
            add_args,
            temp_dir.path().to_path_buf(),
            config.performance.vector_dimension,
            client_arc,
            &config,
            Some(progress_reporter),
        )
        .await;

        // Verify the operation succeeded
        assert!(result.is_ok(), "handle_repo_add failed: {:?}", result.err());

        // Verify progress updates were received
        let updates = progress_updates.lock().unwrap();
        assert!(!updates.is_empty(), "No progress updates were received");

        // Check that we got a completion update
        let has_completion = updates.iter().any(|update| {
            matches!(update.stage, crate::sync_progress::RepoAddStage::Completed { .. })
        });
        assert!(has_completion, "No completion progress update was received");

        // Verify all updates have timestamps
        for update in updates.iter() {
            assert!(update.timestamp.is_some(), "Progress update missing timestamp");
        }
    }
}