//!
//! Handles application configuration, including Qdrant settings, ONNX model paths,
//! and repository management details.
//! Configuration is typically loaded from a `config.toml` file.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};
use anyhow::anyhow;

const APP_NAME: &str = "vectordb-cli";
const CONFIG_FILE_NAME: &str = "config.toml";
const REPO_DIR_NAME: &str = "repositories";
const DEFAULT_QDRANT_URL: &str = "http://localhost:6334";

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
/// Represents configuration for a single managed repository
pub struct RepositoryConfig {
    pub name: String,
    pub url: String,
    pub local_path: PathBuf,
    pub default_branch: String,
    pub tracked_branches: Vec<String>,
    #[serde(default)]
    pub remote_name: Option<String>,
    #[serde(default)]
    pub last_synced_commits: HashMap<String, String>,
    #[serde(default)]
    pub active_branch: Option<String>,
    // SSH Credential Fields (Optional)
    #[serde(default)]
    pub ssh_key_path: Option<PathBuf>,
    #[serde(default)]
    pub ssh_key_passphrase: Option<String>,
    // Indexed languages/extensions
    #[serde(default)]
    pub indexed_languages: Option<Vec<String>>,
    /// If true, the repository was added via a local path rather than a URL.
    pub added_as_local_path: bool,
    /// Optional specific Git ref (tag, commit hash, branch name) to check out and index.
    /// If set, `repo sync` will index this specific ref statically and will *not* pull updates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_ref: Option<String>,
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
/// Represents the application configuration, loaded from a `config.toml` file.
pub struct AppConfig {
    /// The URL for the Qdrant gRPC endpoint.
    #[serde(default = "default_qdrant_url")]
    pub qdrant_url: String,
    /// Optional path to the ONNX model file.
    pub onnx_model_path: Option<String>,
    /// Optional path to the directory containing the ONNX tokenizer files.
    pub onnx_tokenizer_path: Option<String>,
    /// Optional path to the API key file for server authentication.
    #[serde(default)]
    pub server_api_key_path: Option<String>,

    #[serde(default)]
    pub repositories: Vec<RepositoryConfig>,

    #[serde(default)]
    pub active_repository: Option<String>,
    
    /// Optional base path where all repositories will be stored.
    /// If not provided, uses the default XDG data directory.
    #[serde(default)]
    pub repositories_base_path: Option<PathBuf>,

    /// Indexing specific configuration.
    #[serde(default)]
    pub indexing: IndexingConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            qdrant_url: default_qdrant_url(),
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            server_api_key_path: None,
            repositories: Vec::new(),
            active_repository: None,
            repositories_base_path: None,
            indexing: IndexingConfig::default(),
        }
    }
}

fn default_qdrant_url() -> String {
    DEFAULT_QDRANT_URL.to_string()
}

/// Returns the default path to the configuration file.
pub fn get_config_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow!("Could not find config directory"))?
        .join("vectordb");
    Ok(config_dir.join(APP_NAME).join(CONFIG_FILE_NAME))
}

/// Returns the base directory where local repository clones should be stored.
///
/// If a repositories_base_path is configured in AppConfig, uses that.
/// Otherwise, falls back to XDG base directory specification (e.g., `~/.local/share/vectordb-cli/repositories`).
pub fn get_repo_base_path(config: Option<&AppConfig>) -> Result<PathBuf> {
    // First check if there's a configured base path
    if let Some(cfg) = config {
        if let Some(base_path) = &cfg.repositories_base_path {
            return Ok(base_path.clone());
        }
    }
    
    // Fall back to default XDG location
    dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find data directory"))
        .map(|data_dir| data_dir.join(APP_NAME).join(REPO_DIR_NAME))
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

    let mut config_content = toml::to_string_pretty(config)
        .with_context(|| "Failed to serialize configuration to TOML")?;

    // Add commented-out examples for ONNX paths if they are not set
    if config.onnx_model_path.is_none() {
        config_content.push_str("\n# Path to the ONNX model file (required for indexing/querying)");
        config_content.push_str("\n#onnx_model_path = \"/path/to/your/model.onnx\"");
        config_content.push_str("\n# Example: /path/to/vectordb-cli/onnx/all-minilm-l6-v2.onnx");

    }
    if config.onnx_tokenizer_path.is_none() {
        config_content.push_str("\n# Path to the directory containing tokenizer.json (required for indexing/querying)");
        config_content.push_str("\n#onnx_tokenizer_path = \"/path/to/your/tokenizer_directory\"");
        config_content.push_str("\n# Example: /path/to/vectordb-cli/onnx/");
    }

    fs::write(&config_file_path, config_content)
        .with_context(|| format!("Failed to write config file to '{}'", config_file_path.display()))?;

    log::debug!("Configuration saved to '{}'", config_file_path.display());
    Ok(())
}

/// Structure holding repository list and active status, for listing purposes.
#[derive(Debug, Serialize)] // Keep Serialize needed by original caller
pub struct ManagedRepositories {
    pub repositories: Vec<RepositoryConfig>,
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
            onnx_tokenizer_path: None,
            server_api_key_path: None,
            repositories: vec![repo1.clone(), repo2.clone()],
            active_repository: Some("repo1".to_string()),
            repositories_base_path: Some(data_path.clone()),
            indexing: IndexingConfig::default(),
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
        let base_path = PathBuf::from("/fake/data/vectordb-cli/repositories");
        
        // Test with explicit config path
        let config = AppConfig {
            repositories_base_path: Some(PathBuf::from("/custom/repos")),
            qdrant_url: "test".to_string(),
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            server_api_key_path: None,
            repositories: Vec::new(),
            active_repository: None,
            indexing: IndexingConfig::default(),
        };
        
        // Should use the custom path from config
        let repo_path = get_repo_base_path(Some(&config)).unwrap();
        assert_eq!(repo_path, PathBuf::from("/custom/repos"));
        
        // Test with None config (should use default XDG path)
        let default_repo_path = get_repo_base_path(None).unwrap();
        assert!(default_repo_path.to_string_lossy().contains("vectordb-cli/repositories"));
        
        // Create a repo config with a local path based on the base_path
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
        assert_eq!(repo_config.local_path, PathBuf::from("/fake/data/vectordb-cli/repositories/my-test-repo"));
    }
} 