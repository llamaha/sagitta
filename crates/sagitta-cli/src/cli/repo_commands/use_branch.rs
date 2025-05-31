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
    /// This is optional if --target-ref is provided.
    #[arg(group = "ref_specification")]
    pub name: Option<String>,
    
    /// Optional specific Git ref (tag, commit hash, branch name) to check out.
    /// If provided, this ref will be checked out instead of the branch name.
    /// This supports any valid git reference including tags, commits, and remote branches.
    #[arg(long, group = "ref_specification")]
    pub target_ref: Option<String>,
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
    
    // Determine the target reference - either from name or target_ref
    let (target_ref_to_checkout, is_target_ref) = match (&args.name, &args.target_ref) {
        (Some(name), None) => (name.clone(), false),
        (None, Some(target_ref)) => (target_ref.clone(), true),
        (Some(_), Some(_)) => bail!("Cannot specify both branch name and --target-ref. Use one or the other."),
        (None, None) => bail!("Must specify either a branch name or --target-ref."),
    };

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
    let switch_result = git_manager.switch_branch(&repo_path, &target_ref_to_checkout).await
        .context("Failed to switch repository branch/ref")?;

    // Update config with new branch/ref
    let repo_config_mut = &mut config.repositories[repo_config_index];
    
    if is_target_ref {
        // If using target_ref, update the target_ref field and set active_branch to the ref
        repo_config_mut.target_ref = Some(target_ref_to_checkout.clone());
        repo_config_mut.active_branch = Some(target_ref_to_checkout.clone());
    } else {
        // If using branch name, clear target_ref and set active_branch
        repo_config_mut.target_ref = None;
        repo_config_mut.active_branch = Some(target_ref_to_checkout.clone());
        if !repo_config_mut.tracked_branches.contains(&target_ref_to_checkout) {
            repo_config_mut.tracked_branches.push(target_ref_to_checkout.clone());
        }
    }

    save_config(config, override_path)?;

    // Enhanced output with sync information
    let ref_type = if is_target_ref { "ref" } else { "branch" };
    println!(
        "{}",
        format!(
            "Switched to {} '{}' for repository '{}'.",
            ref_type,
            target_ref_to_checkout,
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