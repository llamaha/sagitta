use anyhow::{bail, Result};
use clap::Args;
use std::path::PathBuf;
use colored::*;

// Use config types from vectordb_core
use vectordb_core::{AppConfig, save_config};

#[derive(Args, Debug)]
#[derive(Clone)]
pub struct UseRepoArgs {
    /// Name of the repository to set as active.
    pub name: String,
}

pub fn use_repository(
    args: UseRepoArgs, 
    config: &mut AppConfig, 
    override_path: Option<&PathBuf>
) -> Result<()> {
    let repo_name = args.name;

    if config.repositories.iter().any(|r| r.name == repo_name) {
        config.active_repository = Some(repo_name.clone());
        save_config(&config, override_path)?;
        println!(
            "{}",
            format!("Set active repository to '{}'.", repo_name.cyan()).green()
        );
    } else {
        bail!(
            "Repository '{}' not found. Use 'repo list' to see available repositories.",
            repo_name
        );
    }

    Ok(())
} 