use anyhow::Result;
use clap::Args;
use colored::*;
use git_manager::GitManager;
use sagitta_search::AppConfig;
use std::path::PathBuf;

#[derive(Args, Debug, Clone)]
pub struct ListBranchesArgs {
    /// Optional name of the repository (defaults to active repository)
    pub name: Option<String>,
}

pub async fn handle_list_branches(
    args: ListBranchesArgs,
    config: &AppConfig,
) -> Result<()> {
    let repo_name = args.name.as_ref()
        .or(config.active_repository.as_ref())
        .ok_or_else(|| anyhow::anyhow!("No repository specified and no active repository set"))?;
    
    let repo_config = config.repositories.iter()
        .find(|r| r.name == *repo_name)
        .ok_or_else(|| anyhow::anyhow!("Repository '{}' not found", repo_name))?;
    
    let repo_path = PathBuf::from(&repo_config.local_path);
    let git_manager = GitManager::new();
    
    let branches = git_manager.list_branches(&repo_path)?;
    let current_branch = repo_config.active_branch.as_ref()
        .unwrap_or(&repo_config.default_branch);
    
    println!("Branches in repository '{}':", repo_name.cyan());
    for branch in branches {
        if branch == *current_branch {
            println!("  {} {}", "*".green(), branch.green().bold());
        } else {
            println!("    {}", branch);
        }
    }
    
    Ok(())
} 