use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use colored::*;
use std::path::PathBuf;

use crate::config::{self, AppConfig};
use crate::cli::repo_commands::helpers::switch_repository_branch;

#[derive(Args, Debug)]
#[derive(Clone)]
pub struct UseBranchArgs {
    /// Name of the branch to checkout and set active.
    pub name: String,
}

pub async fn handle_use_branch(args: UseBranchArgs, config: &mut AppConfig, override_path: Option<&PathBuf>) -> Result<()> {
    let repo_name = match config.active_repository.clone() {
        Some(name) => name,
        None => bail!("No active repository set. Use 'repo use <n>' first."),
    };

    let repo_config_index = config
        .repositories
        .iter()
        .position(|r| r.name == repo_name)
        .ok_or_else(|| anyhow!("Active repository '{}' configuration not found.", repo_name))?;

    let repo_config = &config.repositories[repo_config_index];
    let target_branch_name = &args.name;

    switch_repository_branch(repo_config, target_branch_name)
        .context("Failed to switch repository branch")?;

    let repo_config_mut = &mut config.repositories[repo_config_index];
    repo_config_mut.active_branch = Some(target_branch_name.to_string());
    if !repo_config_mut.tracked_branches.contains(target_branch_name) {
        repo_config_mut.tracked_branches.push(target_branch_name.to_string());
    }

    config::save_config(config, override_path)?;

    println!(
        "{}",
        format!(
            "Switched to branch '{}' for repository '{}'.",
            target_branch_name,
            repo_name.cyan()
        ).green()
    );

    Ok(())
}