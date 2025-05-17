//!
//! Handles application configuration, including Qdrant settings, ONNX model paths,
//! and repository management details.
//! Configuration is typically loaded from a `config.toml` file.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};
use anyhow::anyhow;
use crate::error::VectorDBError;
use crate::constants::COLLECTION_NAME_PREFIX;

const APP_NAME: &str = "vectordb";
const CONFIG_FILE_NAME: &str = "config.toml";
const REPO_DIR_NAME: &str = "repositories";
const DEFAULT_QDRANT_URL: &str = "http://localhost:6334";

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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
    /// ID of the tenant this repository belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
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
    /// Batch size for internal embedding operations
    #[serde(default = "default_internal_embed_batch_size")]
    pub internal_embed_batch_size: usize,
    /// Prefix for collection names in Qdrant
    #[serde(default = "default_collection_name_prefix")]
    pub collection_name_prefix: String,
    /// Maximum file size in bytes that will be processed
    #[serde(default = "default_max_file_size_bytes")]
    pub max_file_size_bytes: u64,
    /// Default vector dimension for embeddings
    #[serde(default = "default_vector_dimension")]
    pub vector_dimension: u64,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            batch_size: default_batch_size(),
            internal_embed_batch_size: default_internal_embed_batch_size(),
            collection_name_prefix: default_collection_name_prefix(),
            max_file_size_bytes: default_max_file_size_bytes(),
            vector_dimension: default_vector_dimension(),
        }
    }
}

fn default_batch_size() -> usize {
    256
}

fn default_internal_embed_batch_size() -> usize {
    128
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
/// Configuration for OAuth2 authentication
pub struct OAuthConfig {
    /// OAuth client ID
    pub client_id: String,
    /// OAuth client secret
    pub client_secret: String,
    /// OAuth authorization endpoint
    pub auth_url: String,
    /// OAuth token endpoint
    pub token_url: String,
    /// OAuth user info endpoint
    pub user_info_url: String,
    /// Redirect URI for OAuth flow
    pub redirect_uri: String,
    /// Optional OAuth introspection endpoint URL for token validation
    #[serde(skip_serializing_if = "Option::is_none")] // Keep field optional in TOML
    pub introspection_url: Option<String>,
    /// Optional scopes to request
    #[serde(default)]
    pub scopes: Vec<String>,
}

impl Default for OAuthConfig {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            client_secret: String::new(),
            auth_url: String::new(),
            token_url: String::new(),
            user_info_url: String::new(),
            redirect_uri: String::new(),
            introspection_url: None, // Default to None
            scopes: vec!["openid".to_string(), "profile".to_string(), "email".to_string()],
        }
    }
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
    /// Optional OAuth configuration
    #[serde(default)]
    pub oauth: Option<OAuthConfig>,

    // TLS Configuration for HTTP Server
    #[serde(default)]
    pub tls_enable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls_cert_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls_key_path: Option<String>,

    // CORS Configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cors_allowed_origins: Option<Vec<String>>, // e.g., ["http://localhost:3000", "https://app.example.com"]
    #[serde(default = "default_cors_allow_credentials")]
    pub cors_allow_credentials: bool,
    /// Optional tenant ID for this configuration (used as default for CLI/server operations)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
}

/// Configuration settings related to indexing.
// ... (IndexingConfig remains the same) ...


// --- Default Implementation ---
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            qdrant_url: "http://localhost:6334".to_string(),
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            server_api_key_path: None,
            repositories_base_path: None,
            vocabulary_base_path: None,
            repositories: Vec::new(),
            active_repository: None,
            indexing: IndexingConfig::default(),
            performance: PerformanceConfig::default(),
            oauth: None,
            tls_enable: false,
            tls_cert_path: None,
            tls_key_path: None,
            cors_allowed_origins: None, // Default to None (no CORS headers or restrictive default by tower-http)
            cors_allow_credentials: default_cors_allow_credentials(),
            tenant_id: None,
        }
    }
}

/// Returns the default path to the configuration file.
pub fn get_config_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow!("Could not find config directory"))?
        .join("vectordb");
    Ok(config_dir.join(CONFIG_FILE_NAME))
}

/// Returns the default base path for vocabulary files based on OS conventions.
/// Uses $XDG_DATA_HOME/vectordb-cli/vocabularies or equivalent.
fn get_default_vocabulary_base_path() -> Result<PathBuf> {
    let base_dirs = dirs::data_dir()
        .ok_or_else(|| VectorDBError::ConfigurationError("Could not determine user data directory".to_string()))?;
    let app_data_dir = base_dirs.join(APP_NAME).join("vocabularies");
    // Create the directory if it doesn't exist
    fs::create_dir_all(&app_data_dir).map_err(|e| {
        VectorDBError::DirectoryCreationError {
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
        Some(p) => PathBuf::from(p),
        None => get_default_vocabulary_base_path()?,
    };
    // Ensure base directory exists if explicitly configured
    if config.vocabulary_base_path.is_some() {
         fs::create_dir_all(&base_path).map_err(|e| {
            VectorDBError::DirectoryCreationError {
                path: base_path.clone(),
                source: e,
            }
        })?;
    }

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
                VectorDBError::DirectoryCreationError {
                    path: path.clone(),
                    source: e,
                }
            })?;
            return Ok(path);
        }
    }
    // Default path calculation (as before)
    let base_dirs = dirs::data_dir()
        .ok_or_else(|| VectorDBError::ConfigurationError("Could not determine user data directory".to_string()))?;
    let app_data_dir = base_dirs.join(APP_NAME).join("repositories");
    fs::create_dir_all(&app_data_dir).map_err(|e| {
        VectorDBError::DirectoryCreationError {
            path: app_data_dir.clone(),
            source: e,
        }
    })?;
    Ok(app_data_dir)
}

/// Gets the configuration path by checking ENV, override, or default XDG.
pub fn get_config_path_or_default(override_path: Option<&PathBuf>) -> Result<PathBuf> {
    // Check for test environment variable first
    if let Ok(test_path_str) = std::env::var("VECTORDB_TEST_CONFIG_PATH") {
        log::debug!("Using test config path from ENV: {}", test_path_str);
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

        match toml::from_str(&config_content) {
            Ok(config) => {
                log::debug!("Parsed config successfully: {:?}", config);
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
    fs::create_dir_all(app_config_dir)
        .with_context(|| format!("Failed to create config directory: {}", app_config_dir.display()))?;

    // Serialize the main config structure first
    let mut config_content = toml::to_string_pretty(config)
        .with_context(|| "Failed to serialize configuration to TOML")?;

    // Prepare the commented-out ONNX examples as a separate string
    let mut onnx_comments = String::new();
    if config.onnx_model_path.is_none() {
        onnx_comments.push_str("\n# Path to the ONNX model file (required for indexing/querying)");
        onnx_comments.push_str("\n#onnx_model_path = \"/path/to/your/model.onnx\"");
        onnx_comments.push_str("\n# Example: /path/to/vectordb-cli/onnx/all-minilm-l6-v2.onnx");
    }
    if config.onnx_tokenizer_path.is_none() {
        if !onnx_comments.is_empty() { // Add a newline if model_path comments were also added
            onnx_comments.push_str("\n");
        }
        onnx_comments.push_str("\n# Path to the directory containing tokenizer.json (required for indexing/querying)");
        onnx_comments.push_str("\n#onnx_tokenizer_path = \"/path/to/your/tokenizer_directory\"");
        onnx_comments.push_str("\n# Example: /path/to/vectordb-cli/onnx/");
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

fn default_cors_allow_credentials() -> bool {
    true // Common default for development, might be stricter in production
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::path::Path;

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
            tenant_id: None,
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
            tenant_id: None,
        };

        let config = AppConfig {
            qdrant_url: "http://localhost:6334".to_string(),
            onnx_model_path: Some("model".to_string()),
            onnx_tokenizer_path: None,
            server_api_key_path: None,
            repositories: vec![repo1.clone(), repo2.clone()],
            active_repository: Some("repo1".to_string()),
            repositories_base_path: Some(data_path.to_string_lossy().to_string()),
            vocabulary_base_path: None,
            indexing: IndexingConfig::default(),
            performance: PerformanceConfig::default(),
            oauth: None,
            tls_enable: false,
            tls_cert_path: None,
            tls_key_path: None,
            cors_allowed_origins: None,
            cors_allow_credentials: default_cors_allow_credentials(),
            tenant_id: None,
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
            server_api_key_path: None,
            repositories: Vec::new(),
            active_repository: None,
            vocabulary_base_path: None,
            indexing: IndexingConfig::default(),
            performance: PerformanceConfig::default(),
            oauth: None,
            tls_enable: false,
            tls_cert_path: None,
            tls_key_path: None,
            cors_allowed_origins: None,
            cors_allow_credentials: default_cors_allow_credentials(),
            tenant_id: None,
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
        assert!(default_repo_path.to_string_lossy().contains("vectordb/repositories"));
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
            tenant_id: None,
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
} 