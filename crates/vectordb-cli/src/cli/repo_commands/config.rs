use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use colored::Colorize;
use std::path::PathBuf;
use tempfile::tempdir;

use vectordb_core::AppConfig;
use vectordb_core::config::{save_config, get_config_path_or_default, IndexingConfig, load_config, PerformanceConfig};
use crate::cli::CliArgs;
use crate::cli::commands::Commands;
use crate::cli::simple;
use crate::cli::repo_commands::{RepoArgs, RepoCommand};

#[derive(Args, Debug)]
#[derive(Clone)]
pub struct ConfigArgs {
    #[command(subcommand)]
    command: ConfigCommand,
}

#[derive(Subcommand, Debug)]
#[derive(Clone)]
enum ConfigCommand {
    /// Set the base path where repositories will be stored.
    SetRepoBasePath(SetRepoBasePathArgs),
}

#[derive(Args, Debug)]
#[derive(Clone)]
pub struct SetRepoBasePathArgs {
    /// Path to the directory where repositories will be stored.
    pub path: PathBuf,
}

impl Default for ConfigArgs {
    fn default() -> Self {
        ConfigArgs {
            command: ConfigCommand::SetRepoBasePath(SetRepoBasePathArgs {
                path: PathBuf::new(),
            }),
        }
    }
}

pub fn handle_config(
    args: ConfigArgs,
    config: &mut AppConfig,
    override_path: Option<&PathBuf>,
) -> Result<()> {
    match args.command {
        ConfigCommand::SetRepoBasePath(path_args) => handle_set_repo_base_path(path_args, config, override_path),
    }
}

fn handle_set_repo_base_path(
    args: SetRepoBasePathArgs,
    config: &mut AppConfig,
    override_path: Option<&PathBuf>,
) -> Result<()> {
    // Validate the path exists or create it
    if !args.path.exists() {
        std::fs::create_dir_all(&args.path)
            .with_context(|| format!("Failed to create directory at {}", args.path.display()))?;
        println!("Created directory at {}", args.path.display());
    }

    // Set the config value
    config.repositories_base_path = Some(args.path.to_string_lossy().to_string());
    
    // Save the config using the core function
    vectordb_core::config::save_config(config, override_path)?;
    
    println!("{}", format!("Repository base path set to {}", args.path.display()).green());
    
    // If we have repositories, print a note about existing repositories
    if !config.repositories.is_empty() {
        println!("{}", "Note: Existing repositories will remain at their current locations.".yellow());
        println!("{}", "New repositories will be stored at the configured base path.".yellow());
    }
    
    Ok(())
}

fn get_default_config() -> AppConfig {
    let temp_dir = tempdir().unwrap();
    AppConfig {
        qdrant_url: "http://localhost:6334".to_string(),
        onnx_model_path: None,
        onnx_tokenizer_path: None,
        server_api_key_path: None,
        repositories_base_path: Some(temp_dir.path().join("repos").to_string_lossy().into_owned()),
        vocabulary_base_path: Some(temp_dir.path().join("vocab").to_string_lossy().into_owned()),
        repositories: Vec::new(),
        active_repository: None,
        indexing: IndexingConfig::default(),
        performance: PerformanceConfig {
            vector_dimension: 128,
            collection_name_prefix: "test_collection_".to_string(),
            ..PerformanceConfig::default()
        },
        oauth: None,
        tls_enable: false,
        tls_cert_path: None,
        tls_key_path: None,
        cors_allowed_origins: None,
        cors_allow_credentials: true,
        tenant_id: Some("test-tenant".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use vectordb_core::config::{AppConfig, save_config, get_config_path_or_default, IndexingConfig, load_config};
    use crate::cli::CliArgs;
    use crate::cli::commands::Commands;
    use crate::cli::repo_commands::config::ConfigArgs;
    use crate::cli::simple;
    use std::path::PathBuf;
    use crate::cli::repo_commands::{RepoArgs, RepoCommand};

    #[test]
    fn test_handle_set_repo_base_path() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let repos_dir = temp_dir.path().join("my_repos");
        
        // Create initial config
        let mut config = AppConfig {
            repositories_base_path: None,
            qdrant_url: "http://localhost:6334".to_string(),
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            server_api_key_path: None,
            repositories: Vec::new(),
            active_repository: None,
            indexing: IndexingConfig::default(),
            vocabulary_base_path: None,
            performance: PerformanceConfig::default(),
            oauth: None,
            tls_enable: false,
            tls_cert_path: None,
            tls_key_path: None,
            cors_allowed_origins: None,
            cors_allow_credentials: true,
            tenant_id: Some("test-tenant".to_string()),
        };
        
        // Set repo base path
        let args = SetRepoBasePathArgs {
            path: repos_dir.clone(),
        };
        
        let result = handle_set_repo_base_path(args, &mut config, Some(&config_path));
        assert!(result.is_ok());
        
        // Check config was updated
        assert_eq!(config.repositories_base_path, Some(repos_dir.to_string_lossy().to_string()));
        
        // Check directory was created
        assert!(repos_dir.exists());
        
        // Load config back from disk to verify it was saved
        if config_path.exists() {
            let loaded_config = load_config(Some(&config_path)).unwrap();
            assert_eq!(loaded_config.repositories_base_path, Some(repos_dir.to_string_lossy().to_string()));
        }
    }

    #[test]
    fn test_handle_config_set_paths() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");

        // Initial config (empty paths)
        let mut config = AppConfig {
            qdrant_url: "http://initial".to_string(),
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            server_api_key_path: None,
            repositories: vec![],
            active_repository: None,
            repositories_base_path: None,
            indexing: IndexingConfig::default(),
            vocabulary_base_path: None,
            performance: PerformanceConfig::default(),
            oauth: None,
            tls_enable: false,
            tls_cert_path: None,
            tls_key_path: None,
            cors_allowed_origins: None,
            cors_allow_credentials: true,
            tenant_id: Some("test-tenant".to_string()),
        };
        save_config(&config, Some(&config_path)).unwrap();

        // Args to set paths
        let model_path = "/test/model.onnx".to_string();
        let tokenizer_path = "/test/tokenizer".to_string();
        let qdrant_url = "http://new-qdrant".to_string();

        let args = ConfigArgs {
            command: ConfigCommand::SetRepoBasePath(SetRepoBasePathArgs { path: dir.path().join("test_base_path").clone() })
        };

        let cli_args = CliArgs { 
            command: Commands::Repo(RepoArgs {command: RepoCommand::Config(args.clone()) }),
            onnx_model_path_arg: None, 
            onnx_tokenizer_dir_arg: None,
            tenant_id: None,
        };

        let result = handle_config(args, &mut config, Some(&config_path));
        assert!(result.is_ok(), "handle_config (SetRepoBasePath) failed: {:?}", result.err());

        let loaded_config_base = load_config(Some(&config_path)).unwrap();
        assert!(loaded_config_base.repositories_base_path.is_some());
        assert_eq!(loaded_config_base.repositories_base_path.unwrap(), dir.path().join("test_base_path").to_string_lossy().to_string());
    }
    
    #[test]
    fn test_handle_config_set_base_path() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        let base_path = dir.path().join("my_repos");

        // Initial config
        let mut config = AppConfig {
            qdrant_url: "http://initial".to_string(),
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            server_api_key_path: None,
            repositories: vec![],
            active_repository: None,
            repositories_base_path: None,
            indexing: IndexingConfig::default(),
            vocabulary_base_path: None,
            performance: PerformanceConfig::default(),
            oauth: None,
            tls_enable: false,
            tls_cert_path: None,
            tls_key_path: None,
            cors_allowed_origins: None,
            cors_allow_credentials: true,
            tenant_id: Some("test-tenant".to_string()),
        };
        save_config(&config, Some(&config_path)).unwrap();

        let args = ConfigArgs {
            command: ConfigCommand::SetRepoBasePath(SetRepoBasePathArgs { path: base_path.clone() })
        };

        let cli_args = CliArgs { 
            command: Commands::Repo(RepoArgs {command: RepoCommand::Config(args.clone()) }), 
            onnx_model_path_arg: None, 
            onnx_tokenizer_dir_arg: None,
            tenant_id: None,
        };

        let result = handle_config(args, &mut config, Some(&config_path));
        assert!(result.is_ok(), "handle_config failed: {:?}", result.err());

        config = load_config(Some(&config_path)).unwrap();
        assert_eq!(config.repositories_base_path, Some(base_path.to_string_lossy().to_string()));
    }
} 