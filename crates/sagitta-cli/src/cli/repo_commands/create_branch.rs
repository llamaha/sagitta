use anyhow::Result;
use clap::Args;
use colored::*;
use git_manager::GitManager;
use sagitta_search::AppConfig;
use std::path::PathBuf;

#[derive(Args, Debug, Clone)]
pub struct CreateBranchArgs {
    /// Name of the new branch to create
    pub name: String,
    
    /// Optional repository name (defaults to active repository)
    #[arg(short = 'r', long)]
    pub repo: Option<String>,
    
    /// Optional starting point (commit, branch, or tag)
    #[arg(short = 's', long)]
    pub start_point: Option<String>,
    
    /// Switch to the new branch after creating it
    #[arg(short = 'c', long)]
    pub checkout: bool,
}

pub async fn handle_create_branch(
    args: CreateBranchArgs,
    config: &mut AppConfig,
    override_path: Option<&PathBuf>,
) -> Result<()> {
    let repo_name = args.repo.as_ref()
        .or(config.active_repository.as_ref())
        .ok_or_else(|| anyhow::anyhow!("No repository specified and no active repository set"))?;
    
    let repo_config_index = config.repositories.iter()
        .position(|r| r.name == *repo_name)
        .ok_or_else(|| anyhow::anyhow!("Repository '{}' not found", repo_name))?;
    
    let repo_config = &config.repositories[repo_config_index];
    let repo_path = PathBuf::from(&repo_config.local_path);
    
    let mut git_manager = GitManager::new();
    git_manager.initialize_repository(&repo_path).await?;
    
    // Create the branch
    git_manager.create_branch(&repo_path, &args.name, args.start_point.as_deref())?;
    
    println!(
        "{}",
        format!("Created branch '{}' in repository '{}'", args.name, repo_name).green()
    );
    
    if let Some(start_point) = &args.start_point {
        println!("  Starting from: {}", start_point.cyan());
    }
    
    // Update config to track the new branch
    let repo_config_mut = &mut config.repositories[repo_config_index];
    if !repo_config_mut.tracked_branches.contains(&args.name) {
        repo_config_mut.tracked_branches.push(args.name.clone());
    }
    
    // Switch to the new branch if requested
    if args.checkout {
        let switch_result = git_manager.switch_branch(&repo_path, &args.name).await?;
        repo_config_mut.active_branch = Some(args.name.clone());
        
        println!(
            "{}",
            format!("Switched to branch '{}'", args.name).green()
        );
        
        if let Some(sync_result) = switch_result.sync_result {
            if sync_result.success {
                println!("🔄 Automatic resync completed: {} files updated, {} files added, {} files removed",
                    sync_result.files_updated, sync_result.files_added, sync_result.files_removed);
            }
        }
    }
    
    sagitta_search::save_config(config, override_path)?;
    
    Ok(())
} 