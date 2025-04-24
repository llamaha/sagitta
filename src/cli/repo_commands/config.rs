use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use colored::Colorize;
use std::path::PathBuf;

use vectordb_core::AppConfig;
use vectordb_core::config::{save_config, get_config_path_or_default, IndexingConfig, load_config};
use crate::cli::CliArgs;
use crate::cli::commands::Commands;
use crate::cli::simple;

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
    config.repositories_base_path = Some(args.path.clone());
    
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
    AppConfig {
        qdrant_url: "http://localhost:6334".to_string(),
        onnx_model_path: None,
        onnx_tokenizer_path: None,
        repositories: Vec::new(),
        active_repository: None,
        repositories_base_path: None,
        server_api_key_path: None,
        indexing: IndexingConfig::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    use vectordb_core::config::{AppConfig, save_config, get_config_path_or_default, IndexingConfig, load_config};
    use crate::cli::CliArgs;
    use crate::cli::commands::Commands;
    use crate::cli::repo_commands::config::ConfigArgs;
    use crate::cli::simple;
    use std::path::PathBuf;

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
        };
        
        // Set repo base path
        let args = SetRepoBasePathArgs {
            path: repos_dir.clone(),
        };
        
        let result = handle_set_repo_base_path(args, &mut config, Some(&config_path));
        assert!(result.is_ok());
        
        // Check config was updated
        assert_eq!(config.repositories_base_path, Some(repos_dir.clone()));
        
        // Check directory was created
        assert!(repos_dir.exists());
        
        // Load config back from disk to verify it was saved
        if config_path.exists() {
            let loaded_config = load_config(Some(&config_path)).unwrap();
            assert_eq!(loaded_config.repositories_base_path, Some(repos_dir));
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
        };
        save_config(&config, Some(&config_path)).unwrap();

        // Args to set paths
        let _model_path = "/test/model.onnx".to_string();
        let _tokenizer_path = "/test/tokenizer".to_string();
        let _qdrant_url = "http://new-qdrant".to_string();

        let repo_base_path = dir.path().join("test_base_path");
        let base_path_args = ConfigArgs {
            command: ConfigCommand::SetRepoBasePath(SetRepoBasePathArgs { path: repo_base_path.clone() })
        };

        let cli_args = CliArgs { 
            command: Commands::Simple(simple::SimpleArgs::default()),
            onnx_model_path_arg: None, 
            onnx_tokenizer_dir_arg: None 
        };
        let result_base_path = handle_config(base_path_args, &mut config, Some(&config_path));
        assert!(result_base_path.is_ok(), "handle_config (SetRepoBasePath) failed: {:?}", result_base_path.err());

        let loaded_config_base = load_config(Some(&config_path)).unwrap();
        assert!(loaded_config_base.repositories_base_path.is_some());
        assert_eq!(loaded_config_base.repositories_base_path.unwrap(), repo_base_path);
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
        };
        save_config(&config, Some(&config_path)).unwrap();

        let args = ConfigArgs {
            command: ConfigCommand::SetRepoBasePath(SetRepoBasePathArgs { path: base_path.clone() })
        };

        let cli_args = CliArgs { 
            command: Commands::Simple(simple::SimpleArgs::default()),
            onnx_model_path_arg: None, 
            onnx_tokenizer_dir_arg: None 
        };
        let result = handle_config(args, &mut config, Some(&config_path));
        assert!(result.is_ok(), "handle_config failed: {:?}", result.err());

        config = load_config(Some(&config_path)).unwrap();
        assert_eq!(config.repositories_base_path, Some(base_path));
    }
} 