use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use colored::Colorize;
use std::path::PathBuf;

use crate::config::{self, AppConfig};

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
    
    // Save the config
    config::save_config(config, override_path)?;
    
    println!("{}", format!("Repository base path set to {}", args.path.display()).green());
    
    // If we have repositories, print a note about existing repositories
    if !config.repositories.is_empty() {
        println!("{}", "Note: Existing repositories will remain at their current locations.".yellow());
        println!("{}", "New repositories will be stored at the configured base path.".yellow());
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::load_config;
    use tempfile::tempdir;

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
            repositories: Vec::new(),
            active_repository: None,
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
} 