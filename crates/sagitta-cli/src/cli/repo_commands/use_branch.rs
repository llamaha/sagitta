use clap::Args;
use anyhow::{Result, Context, bail, anyhow};
use std::path::PathBuf;
use std::sync::Arc;
use git_manager::GitManager;
use sagitta_search::{AppConfig, save_config, qdrant_client_trait::QdrantClientTrait};
use sagitta_search::sync::SagittaSync;
use colored::*;

#[derive(Args, Debug)]
#[derive(Clone)]
pub struct UseBranchArgs {
    /// Name of the branch to checkout and set active.
    pub name: String,
}

pub async fn handle_use_branch<C>(
    args: UseBranchArgs, 
    config: &mut AppConfig, 
    client: Arc<C>,
    override_path: Option<&PathBuf>
) -> Result<()> 
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    let repo_name = match config.active_repository.clone() {
        Some(name) => name,
        None => bail!("No active repository set. Use 'repo use <n>' first."),
    };

    let repo_config_index = config
        .repositories
        .iter()
        .position(|r| r.name == repo_name)
        .ok_or_else(|| anyhow!("Active repository '{}' configuration not found.", repo_name))?;

    let repo_name_clone = config.repositories[repo_config_index].name.clone();
    let target_branch_name = &args.name;

    // Get repository path and config for git-manager
    let repo_config = config.repositories[repo_config_index].clone();
    let repo_path = PathBuf::from(&repo_config.local_path);

    // Create SagittaSync implementation
    let vector_sync = Arc::new(SagittaSync::new(
        client,
        repo_config.clone(),
        config.clone(),
    ));

    // Create git manager with real sync capabilities
    let mut git_manager = GitManager::with_sync(vector_sync);

    // Initialize repository
    git_manager.initialize_repository(&repo_path).await
        .context("Failed to initialize repository")?;

    // Switch branch with automatic resync detection and real sync
    let switch_result = git_manager.switch_branch(&repo_path, target_branch_name).await
        .context("Failed to switch repository branch")?;

    // Update config with new branch
    let repo_config_mut = &mut config.repositories[repo_config_index];
    repo_config_mut.active_branch = Some(target_branch_name.to_string());
    if !repo_config_mut.tracked_branches.contains(target_branch_name) {
        repo_config_mut.tracked_branches.push(target_branch_name.to_string());
    }

    save_config(config, override_path)?;

    // Enhanced output with sync information
    println!(
        "{}",
        format!(
            "Switched to branch '{}' for repository '{}'.",
            target_branch_name,
            repo_name_clone.cyan()
        ).green()
    );

    if let Some(sync_result) = switch_result.sync_result {
        if sync_result.success {
            println!("🔄 Automatic resync completed: {} files updated, {} files added, {} files removed",
                sync_result.files_updated, sync_result.files_added, sync_result.files_removed);
        } else {
            println!("⚠️  Automatic resync failed: {}", 
                sync_result.error_message.unwrap_or_else(|| "Unknown error".to_string()).yellow());
        }
    } else {
        println!("✅ No resync needed - repository is up to date");
    }

    Ok(())
}