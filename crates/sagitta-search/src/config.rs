//!
//! Handles application configuration, including Qdrant settings, ONNX model paths,
//! and repository management details.
//! Configuration is typically loaded from a `config.toml` file.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};
use anyhow::anyhow;
use crate::error::SagittaError;
use crate::constants::COLLECTION_NAME_PREFIX;

const APP_NAME: &str = "sagitta";
const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
/// Represents configuration for a single managed repository
pub struct RepositoryConfig {
    /// Unique name identifying the repository.
    pub name: String,
    /// The remote Git URL (HTTPS or SSH).
    pub url: String,
    /// The local filesystem path where the repository is cloned.
    pub local_path: PathBuf,
    /// The default branch name (e.g., "main", "master").
    pub default_branch: String,
    /// List of branches currently tracked for synchronization.
    pub tracked_branches: Vec<String>,
    /// The name of the Git remote to use (usually "origin").
    #[serde(default)]
    pub remote_name: Option<String>,
    /// Map of branch names to the last commit hash synced for that branch.
    #[serde(default)]
    pub last_synced_commits: HashMap<String, String>,
    /// The currently checked-out branch in the local repository.
    #[serde(default)]
    pub active_branch: Option<String>,
    /// Optional path to an SSH private key for authentication.
    #[serde(default)]
    pub ssh_key_path: Option<PathBuf>,
    /// Optional passphrase for the SSH private key.
    #[serde(default)]
    pub ssh_key_passphrase: Option<String>,
    /// Optional list of languages detected and indexed in the last sync.
    #[serde(default)]
    pub indexed_languages: Option<Vec<String>>,
    /// If true, the repository was added via a local path rather than a URL.
    pub added_as_local_path: bool,
    /// Optional specific Git ref (tag, commit hash, branch name) to check out and index.
    /// If set, `repo sync` will index this specific ref statically and will *not* pull updates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_ref: Option<String>,
    /// List of other repositories that this repository depends on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<RepositoryDependency>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
/// Represents a dependency relationship between repositories
pub struct RepositoryDependency {
    /// Name of the repository this depends on (must exist in the repository manager)
    pub repository_name: String,
    /// Specific ref (branch/tag/commit) of the dependency to use
    pub target_ref: Option<String>,
    /// Human-readable description of why this dependency exists
    pub purpose: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
/// Configuration specific to the indexing process.
pub struct IndexingConfig {
    /// Maximum number of concurrent batch upserts to Qdrant.
    #[serde(default = "default_max_concurrent_upserts")]
    pub max_concurrent_upserts: usize,
}

impl Default for IndexingConfig {
    fn default() -> Self {
        Self { 
            max_concurrent_upserts: default_max_concurrent_upserts(),
        }
    }
}

fn default_max_concurrent_upserts() -> usize {
    8 // Default to 8 concurrent uploads
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
/// Configuration for performance-related settings
pub struct PerformanceConfig {
    /// Batch size for Qdrant upserts
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    /// Prefix for collection names in Qdrant
    #[serde(default = "default_collection_name_prefix")]
    pub collection_name_prefix: String,
    /// Maximum file size in bytes that will be processed
    #[serde(default = "default_max_file_size_bytes")]
    pub max_file_size_bytes: u64,
    /// Default vector dimension for embeddings
    #[serde(default = "default_vector_dimension", skip_serializing_if = "is_default_vector_dimension")]
    pub vector_dimension: u64,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            batch_size: default_batch_size(),
            collection_name_prefix: default_collection_name_prefix(),
            max_file_size_bytes: default_max_file_size_bytes(),
            vector_dimension: default_vector_dimension(),
        }
    }
}

fn default_batch_size() -> usize {
    256
}

fn default_collection_name_prefix() -> String {
    "repo_".to_string()
}

fn default_max_file_size_bytes() -> u64 {
    5 * 1024 * 1024 // 5 MB
}

fn default_vector_dimension() -> u64 {
    384
}

// Helper for serde to skip serializing vector_dimension when it has the default value
fn is_default_vector_dimension(dim: &u64) -> bool {
    *dim == default_vector_dimension()
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
/// Configuration for the embedding engine
pub struct EmbeddingEngineConfig {
    /// Session timeout in seconds (0 = no timeout)
    #[serde(default = "default_session_timeout_seconds")]
    pub session_timeout_seconds: u64,
    /// Enable session cleanup on idle
    #[serde(default = "default_enable_session_cleanup")]
    pub enable_session_cleanup: bool,
    /// Batch size for embedding operations (number of texts processed together)
    #[serde(default = "default_embedding_batch_size")]
    pub embedding_batch_size: usize,
}

impl Default for EmbeddingEngineConfig {
    fn default() -> Self {
        Self {
            session_timeout_seconds: default_session_timeout_seconds(),
            enable_session_cleanup: default_enable_session_cleanup(),
            embedding_batch_size: default_embedding_batch_size(),
        }
    }
}


fn default_session_timeout_seconds() -> u64 {
    300 // 5 minutes, match sagitta-embed default
}

fn default_enable_session_cleanup() -> bool {
    true
}

fn default_embedding_batch_size() -> usize {
    128 // Match sagitta-embed default
}


/// Main application configuration structure.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AppConfig {
    /// URL for the Qdrant instance.
    pub qdrant_url: String,
    /// Path to the ONNX embedding model file.
    pub onnx_model_path: Option<String>,
    /// Path to the ONNX tokenizer configuration directory or file.
    pub onnx_tokenizer_path: Option<String>,
    /// Embedding model to download and use (alternative to onnx_model_path/onnx_tokenizer_path).
    /// Options: "bge-small-fast" (INT8), "bge-small-fp32" (standard precision), or any HuggingFace model ID.
    /// Cannot be used together with onnx_model_path/onnx_tokenizer_path.
    pub embed_model: Option<String>,
    /// Optional path to a file containing the server API key.
    pub server_api_key_path: Option<String>,
    /// Base path where repositories are cloned/managed by default.
    pub repositories_base_path: Option<String>,
    /// Optional base path for storing vocabulary files.
    /// If None, a default platform-specific data directory will be used.
    pub vocabulary_base_path: Option<String>,
    /// List of configured repositories.
    #[serde(default)]
    pub repositories: Vec<RepositoryConfig>,
    /// Name of the currently active repository context.
    pub active_repository: Option<String>,
    /// Indexing configuration settings.
    #[serde(default)]
    pub indexing: IndexingConfig,
    /// Performance-related configuration settings
    #[serde(default)]
    pub performance: PerformanceConfig,
    /// Embedding engine configuration settings
    #[serde(default)]
    pub embedding: EmbeddingEngineConfig,
}

/// Configuration settings related to indexing.
// --- Default Implementation ---
impl Default for AppConfig {
    fn default() -> Self {
        Self {
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
}

/// Returns the default path to the configuration file.
pub fn get_config_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow!("Could not find config directory"))?
        .join("sagitta");
    Ok(config_dir.join(CONFIG_FILE_NAME))
}

/// Returns the default base path for vocabulary files based on OS conventions.
/// Uses $XDG_DATA_HOME/sagitta-cli/vocabularies or equivalent.
fn get_default_vocabulary_base_path() -> Result<PathBuf> {
    let base_dirs = dirs::data_dir()
        .ok_or_else(|| SagittaError::ConfigurationError("Could not determine user data directory".to_string()))?;
    let app_data_dir = base_dirs.join(APP_NAME).join("vocabularies");
    // Create the directory if it doesn't exist
    fs::create_dir_all(&app_data_dir).map_err(|e| {
        SagittaError::DirectoryCreationError {
            path: app_data_dir.clone(),
            source: e,
        }
    })?;
    Ok(app_data_dir)
}

/// Gets the full path for a specific collection's vocabulary file.
/// Uses the `vocabulary_base_path` from the config if set, otherwise derives a default path.
pub fn get_vocabulary_path(config: &AppConfig, collection_name: &str) -> Result<PathBuf> {
    let base_path = match &config.vocabulary_base_path {
        Some(p) => {
            let path = PathBuf::from(p);
            // Ensure base directory exists for custom path
            fs::create_dir_all(&path).map_err(|e| {
                SagittaError::DirectoryCreationError {
                    path: path.clone(),
                    source: e,
                }
            })?;
            path
        },
        None => get_default_vocabulary_base_path()?, // This already creates the directory
    };

    // Derive filename from collection name (e.g., repo_my-repo -> my-repo_vocab.json)
    let vocab_filename = format!(
        "{}_vocab.json", 
        collection_name.strip_prefix(COLLECTION_NAME_PREFIX).unwrap_or(collection_name)
    );
    Ok(base_path.join(vocab_filename))
}

/// Returns the base path where repositories should be stored.
/// Uses `repositories_base_path` from config if set, otherwise XDG data directory.
pub fn get_repo_base_path(config: Option<&AppConfig>) -> Result<PathBuf> {
    if let Some(cfg) = config {
        if let Some(base_path) = &cfg.repositories_base_path {
            let path = PathBuf::from(base_path);
             // Ensure base directory exists if explicitly configured
            fs::create_dir_all(&path).map_err(|e| {
                SagittaError::DirectoryCreationError {
                    path: path.clone(),
                    source: e,
                }
            })?;
            return Ok(path);
        }
    }
    // Default path calculation (as before)
    let base_dirs = dirs::data_dir()
        .ok_or_else(|| SagittaError::ConfigurationError("Could not determine user data directory".to_string()))?;
    let app_data_dir = base_dirs.join(APP_NAME).join("repositories");
    fs::create_dir_all(&app_data_dir).map_err(|e| {
        SagittaError::DirectoryCreationError {
            path: app_data_dir.clone(),
            source: e,
        }
    })?;
    Ok(app_data_dir)
}

impl AppConfig {
    /// Validates that the configuration is valid.
    /// Returns an error if mutually exclusive options are set.
    pub fn validate(&self) -> Result<()> {
        // Check that embed_model and onnx paths are not both set
        let has_onnx_paths = self.onnx_model_path.is_some() || self.onnx_tokenizer_path.is_some();
        let has_embed_model = self.embed_model.is_some();
        
        if has_onnx_paths && has_embed_model {
            return Err(anyhow!(
                "Configuration error: 'embed_model' cannot be used together with 'onnx_model_path' or 'onnx_tokenizer_path'. \
                Please use either 'embed_model' for automatic model downloading, or 'onnx_model_path' and 'onnx_tokenizer_path' \
                for manually specifying model files."
            ));
        }
        
        // If onnx paths are set, both must be set
        if self.onnx_model_path.is_some() != self.onnx_tokenizer_path.is_some() {
            return Err(anyhow!(
                "Configuration error: Both 'onnx_model_path' and 'onnx_tokenizer_path' must be set together."
            ));
        }
        
        Ok(())
    }
}

/// Gets the configuration path by checking ENV, override, or default XDG.
pub fn get_config_path_or_default(override_path: Option<&PathBuf>) -> Result<PathBuf> {
    // Check for test environment variable first
    if let Ok(test_path_str) = std::env::var("SAGITTA_TEST_CONFIG_PATH") {
        log::debug!("Using test config path from ENV: {test_path_str}");
        return Ok(PathBuf::from(test_path_str));
    }
    // Then check for direct override path
    if let Some(path) = override_path {
        log::debug!("Using override config path: {}", path.display());
        return Ok(path.clone());
    }
    // Otherwise, use default XDG path
    get_config_path()
}

/// Loads the application configuration from ENV, a specified path, or the default location.
///
/// If the configuration file or directory does not exist at the target path,
/// it creates them with default settings.
/// Returns an error if the file exists but cannot be read or parsed.
pub fn load_config(override_path: Option<&PathBuf>) -> Result<AppConfig> {
    let config_file_path = get_config_path_or_default(override_path)?;
    log::debug!("Attempting to load config from: {}", config_file_path.display());
    
    let app_config_dir = config_file_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid config file path provided or determined"))?;

    if !config_file_path.exists() {
        log::info!(
            "Config file not found at '{}'. Creating default.",
            config_file_path.display()
        );
        // Use the determined parent dir, whether from default or override
        fs::create_dir_all(app_config_dir)
            .with_context(|| format!("Failed to create config directory: {}", app_config_dir.display()))?;
        let default_config = AppConfig::default();
        // Save the new default config to the specified/determined path
        save_config(&default_config, override_path)?;
        Ok(default_config)
    } else {
        log::info!("Loading config from '{}'", config_file_path.display());
        let config_content = fs::read_to_string(&config_file_path)
            .with_context(|| format!("Failed to read config file at '{}'", config_file_path.display()))?;
        
        log::debug!("Read config file content successfully.");

        match toml::from_str::<AppConfig>(&config_content) {
            Ok(config) => {
                log::debug!("Parsed config successfully: {config:?}");
                // Validate the configuration
                config.validate()?;
                Ok(config)
            },
            Err(e) => {
                log::error!(
                    "Failed to parse config file at '{}': {}. Ensure it is valid TOML.",
                    config_file_path.display(),
                    e
                );
                // Consider returning default or erroring based on policy
                // For now, we error out if parsing fails.
                anyhow::bail!("Failed to parse configuration file: {}", e)
            }
        }
    }
}

/// Saves the provided application configuration to ENV, a specified path, or the default location.
///
/// Creates the configuration directory if it doesn't exist.
/// Overwrites the existing configuration file at the target path.
pub fn save_config(config: &AppConfig, override_path: Option<&PathBuf>) -> Result<()> {
    let config_file_path = get_config_path_or_default(override_path)?;
    let app_config_dir = config_file_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid config file path provided or determined"))?;

    // Ensure parent dir exists
    match fs::create_dir_all(app_config_dir) {
        Ok(_) => log::debug!("Ensured config directory exists: {}", app_config_dir.display()),
        Err(e) => {
            log::error!("Failed to create config directory {}: {}", app_config_dir.display(), e);
            // Explicitly panic here to make sure this isn't silently failing
            panic!("fs::create_dir_all failed for {}: {}", app_config_dir.display(), e);
        }
    }

    // Serialize the main config structure first
    let mut config_content = toml::to_string_pretty(config)
        .with_context(|| "Failed to serialize configuration to TOML")?;

    // Prepare the commented-out ONNX/embedding examples as a separate string
    let mut onnx_comments = String::new();
    
    // Add embed_model comments if not set
    if config.embed_model.is_none() && config.onnx_model_path.is_none() && config.onnx_tokenizer_path.is_none() {
        onnx_comments.push_str("\n# Embedding model configuration - choose ONE of the following options:");
        onnx_comments.push('\n');
        onnx_comments.push_str("\n# Option 1: Automatic model downloading (recommended)");
        onnx_comments.push_str("\n# The model will be downloaded to ~/.cache/huggingface/hub/");
        onnx_comments.push_str("\n#embed_model = \"bge-small-fast\"  # BGE Small v1.5 with INT8 quantization (fast)");
        onnx_comments.push_str("\n#embed_model = \"bge-small-fp32\"  # BGE Small v1.5 with FP32 (standard precision)");
        onnx_comments.push_str("\n# Or use any HuggingFace model ID:");
        onnx_comments.push_str("\n#embed_model = \"BAAI/bge-base-en-v1.5\"");
        onnx_comments.push('\n');
        onnx_comments.push_str("\n# Option 2: Manual model paths (for custom models)");
        onnx_comments.push_str("\n#onnx_model_path = \"/path/to/your/model.onnx\"");
        onnx_comments.push_str("\n#onnx_tokenizer_path = \"/path/to/your/tokenizer_directory\"");
    } else if config.embed_model.is_none() {
        // If only manual paths are set, still show embed_model as an alternative
        if config.onnx_model_path.is_none() {
            onnx_comments.push_str("\n# Path to the ONNX model file (required for indexing/querying)");
            onnx_comments.push_str("\n#onnx_model_path = \"/path/to/your/model.onnx\"");
            onnx_comments.push_str("\n# Example: /path/to/sagitta-cli/onnx/all-minilm-l6-v2.onnx");
        }
        if config.onnx_tokenizer_path.is_none() {
            if !onnx_comments.is_empty() { // Add a newline if model_path comments were also added
                onnx_comments.push('\n');
            }
            onnx_comments.push_str("\n# Path to the directory containing tokenizer.json (required for indexing/querying)");
            onnx_comments.push_str("\n#onnx_tokenizer_path = \"/path/to/your/tokenizer_directory\"");
            onnx_comments.push_str("\n# Example: /path/to/sagitta-cli/onnx/");
        }
        if !onnx_comments.is_empty() {
            onnx_comments.push('\n');
            onnx_comments.push_str("\n# Alternative: Use automatic model downloading instead");
            onnx_comments.push_str("\n#embed_model = \"bge-small-fast\"  # or \"bge-small-fp32\"");
        }
    }

    // Prepend ONNX comments if any, ensuring they are at the top level
    if !onnx_comments.is_empty() {
        config_content = format!("{}\n\n{}", onnx_comments.trim_start(), config_content);
    }

    fs::write(&config_file_path, config_content)
        .with_context(|| format!("Failed to write config file to '{}'", config_file_path.display()))?;

    log::debug!("Configuration saved to '{}'", config_file_path.display());
    Ok(())
}

/// Structure holding repository list and active status, for listing purposes.
#[derive(Debug, Serialize)] // Keep Serialize needed by original caller
pub struct ManagedRepositories {
    /// List of repository configurations.
    pub repositories: Vec<RepositoryConfig>,
    /// Name of the currently active repository, if any.
    pub active_repository: Option<String>,
}

/// Gets a snapshot of the repository configurations and the active repository name.
pub fn get_managed_repos_from_config(config: &AppConfig) -> ManagedRepositories {
    // Return a structure containing clones of the needed data
    ManagedRepositories {
        repositories: config.repositories.clone(),
        active_repository: config.active_repository.clone(),
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::path::Path;
    
    const REPO_DIR_NAME: &str = "repositories";

    fn setup_test_env(temp_dir: &Path) -> (PathBuf, PathBuf) {
        let fake_config_dir = temp_dir.join("config");
        let fake_data_dir = temp_dir.join("data");
        std::fs::create_dir_all(fake_config_dir.join(APP_NAME)).unwrap();
        std::fs::create_dir_all(fake_data_dir.join(APP_NAME).join(REPO_DIR_NAME)).unwrap();

        let config_path = fake_config_dir.join(APP_NAME).join(CONFIG_FILE_NAME);
        let data_path = fake_data_dir.join(APP_NAME).join(REPO_DIR_NAME);
        (config_path, data_path)
    }

    fn load_config_from_path(path: &Path) -> Result<AppConfig> {
        let config_content = fs::read_to_string(path)?;
        toml::from_str(&config_content).map_err(anyhow::Error::from)
    }

    fn save_config_to_path(config: &AppConfig, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(config)?;
        fs::write(path, content).map_err(anyhow::Error::from)
    }

    #[test]
    fn test_load_save_config_with_repos() {
        let temp_dir = tempdir().unwrap();
        let (config_path, data_path) = setup_test_env(temp_dir.path());

        let repo1_path = data_path.join("repo1");
        let repo1 = RepositoryConfig {
            name: "repo1".to_string(),
            url: "url1".to_string(),
            local_path: repo1_path.clone(),
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string()],
            remote_name: None,
            last_synced_commits: HashMap::new(),
            active_branch: Some("main".to_string()),
            ssh_key_path: None,
            ssh_key_passphrase: None,
            indexed_languages: Some(vec!["rs".to_string()]),
            added_as_local_path: false,
            target_ref: None,
        };

        let repo2_path = data_path.join("repo2");
        let repo2 = RepositoryConfig {
            name: "repo2".to_string(),
            url: "url2".to_string(),
            local_path: repo2_path.clone(),
            default_branch: "dev".to_string(),
            tracked_branches: vec!["dev".to_string(), "feat/new".to_string()],
            remote_name: Some("upstream".to_string()),
            last_synced_commits: HashMap::from([("dev".to_string(), "abc".to_string())]),
            active_branch: Some("dev".to_string()),
            ssh_key_path: Some(PathBuf::from("~/.ssh/id_rsa2")),
            ssh_key_passphrase: None,
            indexed_languages: None,
            added_as_local_path: false,
            target_ref: None,
        };

        let config = AppConfig {
            qdrant_url: "http://localhost:6334".to_string(),
            onnx_model_path: Some("model".to_string()),
            onnx_tokenizer_path: Some("tokenizer".to_string()),
            embed_model: None,
            server_api_key_path: None,
            repositories: vec![repo1.clone(), repo2.clone()],
            active_repository: Some("repo1".to_string()),
            repositories_base_path: Some(data_path.to_string_lossy().to_string()),
            vocabulary_base_path: None,
            indexing: IndexingConfig::default(),
            performance: PerformanceConfig::default(),
            embedding: EmbeddingEngineConfig::default(),
        };

        // Save and load
        save_config_to_path(&config, &config_path).unwrap();
        let loaded_config = load_config_from_path(&config_path).unwrap();

        // Assert
        assert_eq!(config, loaded_config);
        assert_eq!(loaded_config.repositories.len(), 2);
        assert_eq!(loaded_config.active_repository, Some("repo1".to_string()));
        assert_eq!(loaded_config.repositories[0].last_synced_commits.len(), 0);
        assert_eq!(loaded_config.repositories[1].last_synced_commits.get("dev"), Some(&"abc".to_string()));
        assert_eq!(loaded_config.repositories[1].ssh_key_path, Some(PathBuf::from("~/.ssh/id_rsa2")));
    }

    #[test]
    fn test_toml_parsing_legacy() {
        let legacy_toml = r#"
            qdrant_url = "http://custom-qdrant:1234"
            onnx_model_path = "/path/to/model.onnx"
        "#;
        let config: AppConfig = toml::from_str(legacy_toml).unwrap();
        assert_eq!(config.qdrant_url, "http://custom-qdrant:1234");
        assert_eq!(config.onnx_model_path, Some("/path/to/model.onnx".to_string()));
        assert!(config.repositories.is_empty());
        assert!(config.active_repository.is_none());
        for repo in &config.repositories {
            assert!(repo.active_branch.is_none());
            assert!(repo.remote_name.is_none());
            assert!(repo.ssh_key_path.is_none());
            assert!(repo.ssh_key_passphrase.is_none());
        }
    }

    #[test]
    fn test_repo_path_generation() {
        // Use tempdir for testing filesystem interactions
        let temp_dir = tempdir().unwrap();
        let base_path = temp_dir.path().join("my_repo_base");
        // NOTE: get_repo_base_path will create the directory if needed
        
        // Test with explicit config path
        let config = AppConfig {
            repositories_base_path: Some(base_path.to_string_lossy().to_string()),
            qdrant_url: "test".to_string(),
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            embed_model: None,
            server_api_key_path: None,
            repositories: Vec::new(),
            active_repository: None,
            vocabulary_base_path: None,
            indexing: IndexingConfig::default(),
            performance: PerformanceConfig::default(),
            embedding: EmbeddingEngineConfig::default(),
        };
        
        // Should use the custom path from config
        let repo_path = get_repo_base_path(Some(&config)).unwrap();
        assert_eq!(repo_path, base_path);
        assert!(base_path.exists()); // Verify get_repo_base_path created it
        
        // Test with None config (should use default XDG path)
        // We cannot easily assert the exact default path, but we can check it works
        let default_repo_path_result = get_repo_base_path(None);
        assert!(default_repo_path_result.is_ok());
        let default_repo_path = default_repo_path_result.unwrap();
        assert!(default_repo_path.to_string_lossy().contains("sagitta/repositories"));
        assert!(default_repo_path.exists()); // Verify it created the default dir
        
        // Create a repo config with a local path based on the temp base_path
        let repo_config = RepositoryConfig {
            name: "my-test-repo".to_string(),
            url: "some_url".to_string(),
            local_path: base_path.join("my-test-repo"),
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string()],
            remote_name: None,
            last_synced_commits: HashMap::new(),
            active_branch: None,
            ssh_key_path: None,
            ssh_key_passphrase: None,
            indexed_languages: None,
            added_as_local_path: false,
            target_ref: None,
        };
        assert_eq!(repo_config.local_path, base_path.join("my-test-repo"));
    }

    #[test]
    fn test_get_vocabulary_path() {
        let collection_name = "repo_test-collection";

        // Test with default path
        let mut default_config = AppConfig::default();
        default_config.vocabulary_base_path = None;
        let default_path = get_vocabulary_path(&default_config, collection_name).unwrap();
        assert!(default_path.ends_with("test-collection_vocab.json"));

        // Test with custom path
        let temp_dir = tempdir().unwrap();
        let custom_base = temp_dir.path().join("my_vocabs");
        let mut custom_config = AppConfig::default();
        custom_config.vocabulary_base_path = Some(custom_base.to_str().unwrap().to_string());
        let custom_path = get_vocabulary_path(&custom_config, collection_name).unwrap();
        assert_eq!(custom_path, custom_base.join("test-collection_vocab.json"));
    }

    #[test]
    fn test_get_vocabulary_path_default() -> Result<()> {
        let default_config = AppConfig::default();
        let collection_name = "repo_test";
        let default_path = get_vocabulary_path(&default_config, collection_name)?;
        assert!(default_path.ends_with("test_vocab.json"));
        Ok(())
    }

    #[test]
    fn test_get_vocabulary_path_custom() -> Result<()> {
        let mut custom_config = AppConfig::default();
        let custom_dir = "/tmp/custom_vocab_dir";
        custom_config.vocabulary_base_path = Some(custom_dir.to_string());
        let collection_name = "repo_test";
        let custom_path = get_vocabulary_path(&custom_config, collection_name)?;
        assert!(custom_path.starts_with(&custom_dir));
        assert!(custom_path.ends_with("test_vocab.json"));
        Ok(())
    }

    #[test]
    fn test_load_config_malformed_toml() {
        // Clear test environment variable to ensure we use the provided path
        std::env::remove_var("SAGITTA_TEST_CONFIG_PATH");
        
        let temp_dir = tempdir().unwrap();
        let (config_path, _data_path) = setup_test_env(temp_dir.path());

        // Create a malformed TOML file
        let malformed_toml_content = r#"
            qdrant_url = "http://localhost:6334"
            # Missing closing quote for onnx_model_path
            onnx_model_path = "/path/to/your/model.onnx
        "#;
        fs::write(&config_path, malformed_toml_content).unwrap();

        // Attempt to load the malformed config
        let result = load_config(Some(&config_path));

        // Assert that loading failed
        assert!(result.is_err(), "Expected load_config to fail for malformed TOML");
        
        // Optionally, check the error message or type if needed
        // e.g., assert!(result.unwrap_err().to_string().contains("parse"));
    }

    #[test]
    fn test_load_config_unreadable_file() {
        // Clear test environment variable to ensure we use the provided path
        std::env::remove_var("SAGITTA_TEST_CONFIG_PATH");
        
        let temp_dir = tempdir().unwrap();
        let (config_path, _data_path) = setup_test_env(temp_dir.path());

        // Instead of trying to make a file unreadable (which doesn't work reliably on Unix),
        // test with a directory path instead of a file path.
        // Attempting to read a directory as a file should fail reliably across platforms.
        let directory_path = config_path.parent().unwrap(); // Use the directory instead of the file

        let result = load_config(Some(&directory_path.to_path_buf()));

        assert!(result.is_err(), "Expected load_config to fail when given a directory path instead of a file path. Actual: {:?}", result);
    }

    #[test]
    fn test_load_config_with_env_override() {
        let temp_dir = tempdir().unwrap();
        let env_config_dir = temp_dir.path().join("env_config");
        std::fs::create_dir_all(&env_config_dir).unwrap();
        let env_config_path = env_config_dir.join(CONFIG_FILE_NAME);

        // Create a config file with specific content at the env path
        let env_config = AppConfig {
            qdrant_url: "http://env-override-qdrant:6334".to_string(),
            ..AppConfig::default()
        };
        save_config_to_path(&env_config, &env_config_path).unwrap();

        // Set the environment variable
        std::env::set_var("SAGITTA_TEST_CONFIG_PATH", env_config_path.to_str().unwrap());

        // Load config - it should use the path from the env var
        // Use the env config path as override to ensure we're testing the right thing
        let loaded_config = load_config(Some(&env_config_path)).unwrap();

        // Clean up the environment variable
        std::env::remove_var("SAGITTA_TEST_CONFIG_PATH");

        // Assert that the loaded config is the one from the env var path
        assert_eq!(loaded_config.qdrant_url, "http://env-override-qdrant:6334");
        assert_ne!(loaded_config.qdrant_url, AppConfig::default().qdrant_url);
    }

    #[test]
    fn test_save_config_creates_dir() {
        let temp_dir = tempdir().unwrap();
        let new_config_dir = temp_dir.path().join("new_config_dir");
        let config_path = new_config_dir.join(CONFIG_FILE_NAME);

        // Ensure the directory does not exist initially
        assert!(!new_config_dir.exists());

        // Temporarily clear the env var to ensure this test uses the override_path
        let original_env_var = std::env::var("SAGITTA_TEST_CONFIG_PATH").ok();
        std::env::remove_var("SAGITTA_TEST_CONFIG_PATH");

        let config = AppConfig::default();
        let save_result = save_config(&config, Some(&config_path));

        // Restore env var if it was originally set
        if let Some(val) = original_env_var {
            std::env::set_var("SAGITTA_TEST_CONFIG_PATH", val);
        }
        
        save_result.unwrap(); // Check result after restoring env var

        // Assert that the directory and file were created
        assert!(new_config_dir.exists(), "Config directory should have been created.");
        assert!(config_path.exists(), "Config file should have been created.");

        // Also check if the content is as expected
        let loaded_config = load_config_from_path(&config_path).unwrap();
        assert_eq!(config, loaded_config);
    }

    #[test]
    fn test_load_config_creates_default_if_not_exists() {
        let temp_dir = tempdir().unwrap();
        let new_config_dir = temp_dir.path().join("another_new_config_dir");
        let config_path = new_config_dir.join(CONFIG_FILE_NAME);

        // Ensure the directory and file do not exist initially
        assert!(!new_config_dir.exists());
        assert!(!config_path.exists());

        // Temporarily clear the env var to ensure this test uses the override_path
        let original_env_var = std::env::var("SAGITTA_TEST_CONFIG_PATH").ok();
        std::env::remove_var("SAGITTA_TEST_CONFIG_PATH");

        // Load config, expecting it to create a default one
        let load_result = load_config(Some(&config_path));

        // Restore env var if it was originally set
        if let Some(val) = original_env_var {
            std::env::set_var("SAGITTA_TEST_CONFIG_PATH", val);
        }

        let loaded_config = load_result.unwrap();
        let default_config = AppConfig::default();

        // Assert that the directory and file were created
        assert!(new_config_dir.exists(), "Config directory should have been created by load_config.");
        assert!(config_path.exists(), "Config file should have been created by load_config.");

        // Assert that the loaded config is the default config
        assert_eq!(loaded_config, default_config, "Loaded config should be the default config.");

        // Verify the content of the created file
        let file_content = fs::read_to_string(&config_path).unwrap();
        let parsed_config_from_file: AppConfig = toml::from_str(&file_content).unwrap();
        assert_eq!(parsed_config_from_file, default_config, "The content of the created config file should be the default config.");
    }

    #[test]
    fn test_get_vocabulary_path_creates_custom_base_dir() -> Result<()> {
        let temp_dir = tempdir().unwrap();
        let custom_vocab_base_str = temp_dir.path().join("custom_vocab_storage");
        
        assert!(!custom_vocab_base_str.exists(), "Custom vocab base directory should not exist yet.");

        let mut config = AppConfig::default();
        config.vocabulary_base_path = Some(custom_vocab_base_str.to_string_lossy().into_owned());

        let collection_name = "repo_sample_collection";
        let vocab_path = get_vocabulary_path(&config, collection_name)?;

        assert!(custom_vocab_base_str.exists(), "Custom vocab base directory should have been created.");
        assert!(vocab_path.starts_with(&custom_vocab_base_str),
                "Vocabulary path should be inside the custom base directory.");
        assert!(vocab_path.ends_with("sample_collection_vocab.json"),
                "Vocabulary filename is incorrect.");
        
        Ok(())
    }

    #[test]
    fn test_get_repo_base_path_creates_custom_dir() -> Result<()> {
        let temp_dir = tempdir().unwrap();
        let custom_repo_base_dir = temp_dir.path().join("custom_repo_storage");

        assert!(!custom_repo_base_dir.exists(), "Custom repo base directory should not exist yet.");

        let config = AppConfig {
            repositories_base_path: Some(custom_repo_base_dir.to_string_lossy().into_owned()),
            ..AppConfig::default()
        };

        let repo_base_path = get_repo_base_path(Some(&config))?;

        assert!(custom_repo_base_dir.exists(), "Custom repo base directory should have been created.");
        assert_eq!(repo_base_path, custom_repo_base_dir, "Returned path should be the custom one.");

        Ok(())
    }

    #[test]
    fn test_get_repo_base_path_creates_default_dir_no_config() -> Result<()> {
        // To test the true default path, we need to ensure our test environment
        // doesn't interfere with dirs::data_dir(). This is tricky to isolate perfectly
        // without deeper mocking of the dirs crate or running in a very controlled environment.
        // For now, we rely on dirs::data_dir() behaving consistently and that our test
        // execution has permissions to create directories in the default location.
        // We will check for a subdirectory that should be created.

        // We need a way to predict the default path to check for its creation.
        // Let's simulate the logic within get_repo_base_path for default path generation.
        let expected_default_data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow!("Could not determine user data directory for test setup"))?;
        let expected_default_app_data_dir = expected_default_data_dir.join(APP_NAME).join(REPO_DIR_NAME);

        // It's possible this directory already exists from previous tests or application runs.
        // If it exists, we can't definitively prove *this* call created it unless we delete it first,
        // which is risky. So, we'll just ensure it exists after the call.
        // For a clean test, ideally this path would be unique per test run or cleaned up.
        // tempfile::tempdir() is not suitable here as we want to test the actual default path logic.

        let repo_base_path = get_repo_base_path(None)?;

        assert!(expected_default_app_data_dir.exists(), 
                "Default repo base directory ({}) should exist after calling get_repo_base_path(None).",
                expected_default_app_data_dir.display());
        assert_eq!(repo_base_path, expected_default_app_data_dir,
                 "Returned path should be the default XDG data directory path.");

        Ok(())
    }

    #[test]
    fn test_get_managed_repos_from_config() {
        let temp_dir = tempdir().unwrap();
        let (_config_path, data_path) = setup_test_env(temp_dir.path());

        let repo1_path = data_path.join("repo1_managed");
        let repo1 = RepositoryConfig {
            name: "repo1_managed".to_string(),
            url: "url1_managed".to_string(),
            local_path: repo1_path.clone(),
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string()],
            active_branch: Some("main".to_string()),
            ..Default::default() // Use default for other fields
        };

        let repo2_path = data_path.join("repo2_managed");
        let repo2 = RepositoryConfig {
            name: "repo2_managed".to_string(),
            url: "url2_managed".to_string(),
            local_path: repo2_path.clone(),
            default_branch: "dev".to_string(),
            tracked_branches: vec!["dev".to_string()],
            active_branch: Some("dev".to_string()),
            ..Default::default()
        };

        let original_config = AppConfig {
            repositories: vec![repo1.clone(), repo2.clone()],
            active_repository: Some("repo1_managed".to_string()),
            ..AppConfig::default()
        };

        let managed_repos = get_managed_repos_from_config(&original_config);

        assert_eq!(managed_repos.repositories.len(), 2);
        assert_eq!(managed_repos.repositories[0].name, "repo1_managed");
        assert_eq!(managed_repos.repositories[1].name, "repo2_managed");
        assert_eq!(managed_repos.active_repository, Some("repo1_managed".to_string()));

        // Ensure it's a clone (modify original and check managed_repos isn't affected - though not strictly necessary for this simple getter)
        // let mut mut_original_config = original_config.clone();
        // mut_original_config.active_repository = Some("changed".to_string());
        // assert_eq!(managed_repos.active_repository, Some("repo1_managed".to_string()));
    }

    #[test]
    fn test_indexing_config_default() {
        let default_indexing = IndexingConfig::default();
        assert_eq!(default_indexing.max_concurrent_upserts, default_max_concurrent_upserts());
        // Add more assertions here if IndexingConfig gets more fields with defaults
    }

    #[test]
    fn test_performance_config_default() {
        let default_perf = PerformanceConfig::default();
        assert_eq!(default_perf.batch_size, default_batch_size());
        assert_eq!(default_perf.collection_name_prefix, default_collection_name_prefix());
        assert_eq!(default_perf.max_file_size_bytes, default_max_file_size_bytes());
        assert_eq!(default_perf.vector_dimension, default_vector_dimension());
    }

    #[test]
    fn test_vector_dimension_not_serialized_when_default() {
        let config = PerformanceConfig::default();
        let serialized = toml::to_string(&config).expect("Failed to serialize config");
        
        // vector_dimension should not appear in the serialized output when it's the default value
        assert!(!serialized.contains("vector_dimension"), 
                "vector_dimension should not be serialized when it has the default value. Serialized: {}", serialized);
        
        // But other fields should still be present
        assert!(serialized.contains("batch_size"));
        assert!(serialized.contains("collection_name_prefix"));
        assert!(serialized.contains("max_file_size_bytes"));
    }

    #[test]
    fn test_vector_dimension_serialized_when_non_default() {
        let mut config = PerformanceConfig::default();
        config.vector_dimension = 512; // Non-default value
        
        let serialized = toml::to_string(&config).expect("Failed to serialize config");
        
        // vector_dimension should appear when it's not the default value
        assert!(serialized.contains("vector_dimension = 512"), 
                "vector_dimension should be serialized when it has a non-default value. Serialized: {}", serialized);
    }


    #[test]
    fn test_config_validation_mutually_exclusive() {
        let mut config = AppConfig::default();
        
        // Valid: no model configuration
        assert!(config.validate().is_ok());
        
        // Valid: only embed_model
        config.embed_model = Some("bge-small-fast".to_string());
        assert!(config.validate().is_ok());
        
        // Valid: only onnx paths
        config.embed_model = None;
        config.onnx_model_path = Some("/path/to/model.onnx".to_string());
        config.onnx_tokenizer_path = Some("/path/to/tokenizer".to_string());
        assert!(config.validate().is_ok());
        
        // Invalid: both embed_model and onnx paths
        config.embed_model = Some("bge-small-fast".to_string());
        assert!(config.validate().is_err());
        
        // Invalid: only model path without tokenizer path
        config.embed_model = None;
        config.onnx_model_path = Some("/path/to/model.onnx".to_string());
        config.onnx_tokenizer_path = None;
        assert!(config.validate().is_err());
        
        // Invalid: only tokenizer path without model path
        config.onnx_model_path = None;
        config.onnx_tokenizer_path = Some("/path/to/tokenizer".to_string());
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_embed_model_field() {
        let toml_with_embed_model = r#"
            qdrant_url = "http://localhost:6334"
            embed_model = "bge-small-fast"
        "#;
        
        let config: AppConfig = toml::from_str(toml_with_embed_model).unwrap();
        assert_eq!(config.embed_model, Some("bge-small-fast".to_string()));
        assert!(config.onnx_model_path.is_none());
        assert!(config.onnx_tokenizer_path.is_none());
        assert!(config.validate().is_ok());
    }
} 