// Use modules from *within* vectordb_core
use crate::config::RepositoryConfig;
// use crate::error::VectorDBError; // Marked as unused
use crate::repo_helpers as helpers;
// use crate::repo_helpers::{prepare_repository, index_files}; // Marked as unused
use crate::qdrant_client_trait::QdrantClientTrait;

// Use modules from the main vectordb_cli crate
// Remove the below line if it exists:
// use vectordb_cli::vectordb::embedding_logic::EmbeddingHandler;

// Other dependencies
use anyhow::anyhow;
use clap::Args; // Keep Args
use colored::*;
use git2::Repository;
use std::{fs, path::PathBuf, sync::Arc, collections::HashMap};
use thiserror::Error;
use log::{info, error, warn};
use crate::config::AppConfig;
 // Use ManualMock
use std::io::Write;
use git2::build::RepoBuilder; // Import RepoBuilder

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

// Define a specific error type for this operation
#[derive(Error, Debug)]
pub enum AddRepoError {
    #[error("Invalid arguments: {0}")]
    InvalidArgs(String),
    #[error("Repository '{0}' already exists.")]
    RepoExists(String), // Note: This check will now be done by the caller (Relay)
    #[error("Could not derive repository name from {0}")]
    NameDerivationError(String),
    #[error("Filesystem error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Configuration error: {0}")]
    ConfigError(anyhow::Error),
    #[error("Git operation failed: {0}")]
    GitError(anyhow::Error), // Keep anyhow for git errors for now
    #[error("Failed to open repository at {0}: {1}")]
    RepoOpenError(PathBuf, anyhow::Error),
    #[error("Failed to determine default branch: {0}")]
    BranchDetectionError(anyhow::Error),
    #[error("Qdrant operation failed: {0}")]
    QdrantError(anyhow::Error), // Keep anyhow for qdrant errors
    #[error("Embedding logic error: {0}")]
    EmbeddingError(anyhow::Error),
    #[error("Failed to determine repository URL.")]
    UrlDeterminationError,
}

pub async fn handle_repo_add<C>(
    args: AddRepoArgs,
    repo_base_path_for_add: PathBuf,
    embedding_dim: u64,
    client: Arc<C>,
    config: &AppConfig,
) -> Result<RepositoryConfig, AddRepoError>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    // Validate basic arguments
    if args.local_path.is_none() && args.url.is_none() {
        return Err(AddRepoError::InvalidArgs("Either --local-path or --url must be specified.".to_string()));
    }

    // Handle repository name
    let repo_name = match &args.name {
        Some(name) => name.clone(),
        None => {
            // If URL is provided, derive name from URL
            if let Some(url) = &args.url {
                PathBuf::from(url)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.trim_end_matches(".git").to_string())
                    .ok_or_else(|| AddRepoError::NameDerivationError("URL".to_string()))?
            } else {
                // If only local path is provided, derive name from the directory name
                let local_path_ref = args.local_path.as_ref().unwrap(); // Use as_ref() to avoid moving
                local_path_ref
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| AddRepoError::NameDerivationError("local path".to_string()))?
            }
        },
    };

    // Use the passed-in base path
    let repo_base_path = repo_base_path_for_add;

    // Ensure the determined base path exists
    fs::create_dir_all(&repo_base_path)
        .map_err(AddRepoError::IoError)?;
    
    // Determine the final local path for the repository
    let local_path = args.local_path.clone().unwrap_or_else(|| repo_base_path.join(&repo_name));

    // If URL is not provided but required
    let mut repo_url = args.url.clone();

    // Flag to indicate if the repo was added by specifying a local path initially
    let added_as_local_path_flag = args.local_path.is_some();

    let repo = if local_path.exists() {
         info!(
            "Local directory '{}' already exists. Assuming it's the intended clone. Skipping clone.",
            local_path.display()
        );
        let git_repo = Repository::open(&local_path)
            .map_err(|e| AddRepoError::RepoOpenError(local_path.clone(), e.into()))?;
        
        // If URL wasn't provided, try to extract it from the repository's remote
        if repo_url.is_none() {
            let remote_name = args.remote.as_deref().unwrap_or("origin");
            match git_repo.find_remote(remote_name) {
                Ok(remote) => {
                    if let Some(url) = remote.url() {
                        repo_url = Some(url.to_string());
                    } else {
                        warn!("Remote '{remote_name}' found but has no URL configured.");
                    }
                }
                Err(_) => {
                    return Err(AddRepoError::InvalidArgs(format!("Could not find remote '{remote_name}' in existing repository. Please specify --url.")));
                }
            }
        }
        
        git_repo
    } else {
        // For new clones, URL is required
        if repo_url.is_none() {
            return Err(AddRepoError::InvalidArgs("URL is required when adding a new repository (--local-path does not exist).".to_string()));
        }
        
        let url = repo_url.as_ref().unwrap(); // Safe because we checked above
        
        info!("\nSTEP 1/2: Cloning repository '{}' from {}", repo_name, url);
        
        // Create the directory if it doesn't exist
        fs::create_dir_all(&local_path)
            .map_err(AddRepoError::IoError)?;
        
        // Use direct git command instead of git2-rs for SSH authentication
        let mut cmd = std::process::Command::new("git");
        cmd.arg("clone")
           .arg(url)
           .arg(&local_path);
        
        // If SSH key is provided, use GIT_SSH_COMMAND to specify the key
        if let Some(ssh_key) = &args.ssh_key {
            let ssh_cmd = if let Some(_passphrase) = &args.ssh_passphrase {
                // With passphrase - note: for SSH keys with passphrase, the SSH agent should be running
                // and should have the key loaded, as Git can't handle passphrase input non-interactively
                format!("ssh -i {} -o IdentitiesOnly=yes", ssh_key.display())
            } else {
                // Without passphrase
                format!("ssh -i {} -o IdentitiesOnly=yes", ssh_key.display())
            };
            cmd.env("GIT_SSH_COMMAND", ssh_cmd);
            let key_path_display = ssh_key.display();
            info!("Using SSH key: {}", key_path_display);
        }
            
        let status = cmd.status()
            .map_err(|e| AddRepoError::GitError(e.into()))?;
        
        if !status.success() {
            return Err(AddRepoError::GitError(anyhow!("Git clone command failed with exit code: {}", status)));
        }
        
        info!("\nRepository cloned successfully to {}", local_path.display());
        
        // Open the repository after cloning
        Repository::open(&local_path)
            .map_err(|e| AddRepoError::RepoOpenError(local_path.clone(), e.into()))?
    };

    // --- Add Git Checkout Logic --- 
    if let Some(ref_name) = &args.target_ref {
        info!("Attempting to checkout target ref: {}", ref_name);
        let mut cmd = std::process::Command::new("git");
        cmd.current_dir(&local_path) // Run checkout in the repo directory
           .arg("checkout")
           .arg(ref_name);
        
        // Add GIT_SSH_COMMAND env var if SSH key was provided, needed for checkout if ref involves remote objects
        if let Some(ssh_key) = &args.ssh_key {
            let ssh_cmd = if args.ssh_passphrase.is_some() {
                format!("ssh -i {} -o IdentitiesOnly=yes", ssh_key.display())
            } else {
                format!("ssh -i {} -o IdentitiesOnly=yes", ssh_key.display())
            };
            cmd.env("GIT_SSH_COMMAND", ssh_cmd);
        }

        let checkout_output = cmd.output()
            .map_err(|e| AddRepoError::GitError(anyhow!("Failed to execute git checkout command: {}", e)))?;
        
        if !checkout_output.status.success() {
            let stderr = String::from_utf8_lossy(&checkout_output.stderr);
            let stdout = String::from_utf8_lossy(&checkout_output.stdout);
            error!(
                "Git checkout command failed for ref '{}'. Status: {}. Stderr: {}. Stdout: {}", 
                ref_name, checkout_output.status, stderr, stdout
            );
            return Err(AddRepoError::GitError(anyhow!(
                "Git checkout command failed for ref '{}'. Status: {}. Stderr: {}. Stdout: {}",
                ref_name, checkout_output.status, stderr, stdout
            )));
        }
        info!("Successfully checked out ref: {}", ref_name);
    }
    // --- End Git Checkout Logic ---

    let initial_branch_name = match args.branch {
        Some(branch_name) => branch_name.clone(),
        None => {
            // If target_ref was specified, we might be in detached HEAD, 
            // so finding the default branch via HEAD might not be reliable or desired.
            // Let's prioritize target_ref if provided, otherwise determine from HEAD.
            if let Some(ref_name) = &args.target_ref {
                 ref_name.clone() // Use target_ref as the initial "branch" identifier
            } else {
                 let head_ref = repo.find_reference("HEAD").map_err(|e| AddRepoError::GitError(e.into()))?;
                 let head_ref_resolved = head_ref.resolve().map_err(|e| AddRepoError::GitError(e.into()))?;
                 head_ref_resolved.shorthand()
                     .ok_or_else(|| AddRepoError::BranchDetectionError(anyhow!("Could not determine default branch name from HEAD")))?
                     .to_string()
            }
        }
    };
    // Only print if we didn't use target_ref above
    if args.target_ref.is_none() {
        info!("Default/Initial branch detected: {}", initial_branch_name);
    }

    info!("\n{}", 
        format!("STEP 2/2: Setting up vector database infrastructure for '{}'", repo_name).bold().cyan()
    );
    
    // Use helpers from crate::repo_helpers
    let collection_name = helpers::get_collection_name(&repo_name, config);
    info!("Ensuring Qdrant collection '{}' exists...", collection_name.cyan());
    
    // Use the passed-in embedding_dim
    info!("Using embedding dimension: {}", embedding_dim);
    
    // Use helpers from crate::repo_helpers
    helpers::ensure_repository_collection_exists(client.as_ref(), &collection_name, embedding_dim).await
        .map_err(|e| AddRepoError::QdrantError(e.into()))?;

    // Ensure we have the final URL
    let final_url = repo_url.ok_or(AddRepoError::UrlDeterminationError)?;
    
    // Determine active_branch based on target_ref presence
    let final_active_branch = args.target_ref.clone().unwrap_or_else(|| initial_branch_name.clone());

    // Use RepositoryConfig from crate::config
    let new_repo_config = RepositoryConfig {
        name: repo_name.clone(),
        url: final_url,
        local_path: local_path.clone(),
        default_branch: initial_branch_name.clone(), // Keep initial/default branch info
        tracked_branches: vec![initial_branch_name.clone()], // Only track initial for now
        active_branch: Some(final_active_branch), // Store target_ref or initial branch
        remote_name: Some(args.remote.clone().unwrap_or_else(|| "origin".to_string())),
        ssh_key_path: args.ssh_key.clone(),
        ssh_key_passphrase: args.ssh_passphrase.clone(),
        last_synced_commits: HashMap::new(),
        indexed_languages: None,
        added_as_local_path: added_as_local_path_flag,
        target_ref: args.target_ref.clone(), // Store the target_ref
    };

    Ok(new_repo_config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, IndexingConfig, PerformanceConfig};
    // use crate::qdrant_client_trait::MockQdrantClientTrait; // Remove mockall
    use crate::test_utils::ManualMockQdrantClient; // Use ManualMock
    use std::path::Path;
    use tempfile::tempdir;
    use std::fs;
    use std::sync::Arc;
    use git2::{Repository, Signature, /*IndexAddOption, FileMode*/};
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
            server_api_key_path: None,
            repositories_base_path: None,
            vocabulary_base_path: None,
            repositories: vec![],
            active_repository: None,
            indexing: IndexingConfig::default(),
            performance: PerformanceConfig::default(),
        }
    }

    #[tokio::test]
    async fn test_handle_repo_add_new_repo_local_path_success() {
        let temp_dir = tempdir().unwrap();
        let local_repo_path = temp_dir.path().join("test_repo");
        fs::create_dir_all(&local_repo_path).unwrap();
        let repo = git2::Repository::init(&local_repo_path).unwrap();
        create_initial_commit(&repo, "README.md", "Initial commit").expect("Failed to create initial commit");
        // Add a dummy remote 'origin' so the URL derivation logic doesn't fail immediately
        repo.remote("origin", "file:///dev/null").expect("Failed to add dummy remote"); 

        let manual_mock_client = ManualMockQdrantClient::new();
        let client_arc = Arc::new(manual_mock_client.clone());
        let repo_name_str = "test_repo";

        let config = AppConfig {
            qdrant_url: "dummy_url".to_string(),
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
        };
        
        let expected_collection_name = format!("{}{}", config.performance.collection_name_prefix, repo_name_str);
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
        assert_eq!(manual_mock_client.get_collection_exists_args()[0], expected_collection_name);
        assert!(manual_mock_client.verify_create_collection_called());
        assert!(manual_mock_client.verify_create_collection_args(&expected_collection_name, expected_dimension));
    }

    // Shared test setup
    fn create_test_config() -> AppConfig {
        let temp_dir = tempdir().expect("Failed to create temp dir for test config");
        let model_base = temp_dir.path().join("models");
        let vocab_base = temp_dir.path().join("vocab");
        let repo_base = temp_dir.path().join("repos");
        fs::create_dir_all(&model_base).expect("Failed to create model base dir");
        fs::create_dir_all(&vocab_base).expect("Failed to create vocab base dir");
        fs::create_dir_all(&repo_base).expect("Failed to create repo base dir");

        AppConfig {
            qdrant_url: "http://localhost:6334".to_string(),
            onnx_model_path: Some(model_base.join("model.onnx").to_string_lossy().into_owned()),
            onnx_tokenizer_path: Some(model_base.join("tokenizer.json").to_string_lossy().into_owned()),
            server_api_key_path: None,
            repositories_base_path: Some(repo_base.to_string_lossy().into_owned()),
            vocabulary_base_path: Some(vocab_base.to_string_lossy().into_owned()),
            repositories: Vec::new(),
            active_repository: None,
            indexing: IndexingConfig::default(),
            performance: PerformanceConfig {
                vector_dimension: 128,
                collection_name_prefix: "test_collection_".to_string(),
                ..PerformanceConfig::default()
            },
        }
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
            qdrant_url: "dummy_url".to_string(),
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
        };
        
        let expected_collection_name = format!("{}{}", config.performance.collection_name_prefix, repo_name_str);
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
        )
        .await;

        assert!(result.is_ok(), "handle_repo_add failed: {:?}", result.err());
        let repo_config_res = result.unwrap();
        assert_eq!(repo_config_res.name, repo_name_str);
        assert_eq!(repo_config_res.url, "https://example.com/existing_repo.git");
        assert!(repo_config_res.added_as_local_path);

        // Verify mock calls
        assert_eq!(manual_mock_client.verify_collection_exists_called_times(), 1);
        assert_eq!(manual_mock_client.get_collection_exists_args()[0], expected_collection_name);
        assert!(manual_mock_client.verify_create_collection_called());
        assert!(manual_mock_client.verify_create_collection_args(&expected_collection_name, expected_dimension));
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
            qdrant_url: "dummy_qdrant_url".to_string(),
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
            }
        };

        let repo_name_str = "test_cloned_repo";

        let expected_collection_name = format!("{}{}", config.performance.collection_name_prefix, repo_name_str);
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
        assert_eq!(manual_mock_client.get_collection_exists_args()[0], expected_collection_name);
        assert!(manual_mock_client.verify_create_collection_called());
        assert!(manual_mock_client.verify_create_collection_args(&expected_collection_name, expected_dimension));
    }
}