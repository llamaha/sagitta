use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use colored::*;
use git2::Repository;
use std::path::PathBuf;

use crate::config::{self, AppConfig};
use crate::cli::repo_commands::helpers;

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

    let repo = Repository::open(&repo_config.local_path)
        .with_context(|| format!("Failed to open repository at {}", repo_config.local_path.display()))?;

    let target_branch_name = &args.name;
    let remote_name = repo_config.remote_name.as_deref().unwrap_or("origin");
    let repo_url = repo_config.url.clone();

    if repo.find_branch(target_branch_name, git2::BranchType::Local).is_err() {
        println!(
            "Local branch '{}' not found. Checking remote '{}'...",
            target_branch_name, remote_name
        );
        
        println!("Fetching from remote '{}' to update refs...", remote_name);
        let mut remote = repo.find_remote(remote_name)?;
        let repo_configs_clone = config.repositories.clone();
        let mut fetch_opts = helpers::create_fetch_options(repo_configs_clone, &repo_url)?;
        remote.fetch(&[] as &[&str], Some(&mut fetch_opts), None)
            .with_context(|| format!("Failed initial fetch from remote '{}' before branch check", remote_name))?;
        println!("Fetch for refs update complete.");

        let remote_branch_ref = format!("{}/{}", remote_name, target_branch_name);
        match repo.find_branch(&remote_branch_ref, git2::BranchType::Remote) {
            Ok(remote_branch) => {
                println!(
                    "Branch '{}' found on remote '{}'. Creating local tracking branch...",
                    target_branch_name, remote_name
                );
                let commit = remote_branch.get().peel_to_commit()
                    .with_context(|| format!("Failed to get commit for remote branch {}", remote_branch_ref))?;
                repo.branch(target_branch_name, &commit, false)
                    .with_context(|| format!("Failed to create local branch '{}'", target_branch_name))?;
                let mut local_branch = repo.find_branch(target_branch_name, git2::BranchType::Local)?;
                local_branch.set_upstream(Some(&remote_branch_ref))
                    .with_context(|| format!("Failed to set upstream for branch '{}' to '{}'", target_branch_name, remote_branch_ref))?;
            }
            Err(_) => {
                bail!(
                    "Branch '{}' not found locally or on remote '{}'.",
                    target_branch_name,
                    remote_name
                );
            }
        }
    }

    println!("Checking out branch '{}'...", target_branch_name);
    let ref_name = format!("refs/heads/{}", target_branch_name);
    repo.set_head(&ref_name)
        .with_context(|| format!("Failed to checkout branch '{}'", target_branch_name))?;
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .with_context(|| format!("Failed to force checkout head for branch '{}'", target_branch_name))?;

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