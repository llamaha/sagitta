use anyhow::{Result, bail};
use clap::Args;
use colored::*;
use git_manager::GitManager;
use sagitta_search::AppConfig;
use std::path::PathBuf;

#[derive(Args, Debug, Clone)]
pub struct DeleteBranchArgs {
    /// Name of the branch to delete
    pub name: String,
    
    /// Optional repository name (defaults to active repository)
    #[arg(short = 'r', long)]
    pub repo: Option<String>,
    
    /// Force deletion even if branch has unmerged changes
    #[arg(short = 'f', long)]
    pub force: bool,
    
    /// Skip confirmation prompt
    #[arg(short = 'y', long)]
    pub yes: bool,
}

pub async fn handle_delete_branch(
    args: DeleteBranchArgs,
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
    
    // Check if trying to delete the current branch
    let current_branch = repo_config.active_branch.as_ref()
        .unwrap_or(&repo_config.default_branch);
    
    if args.name == *current_branch {
        bail!("Cannot delete the currently active branch '{}'. Switch to another branch first.", args.name);
    }
    
    // Check if trying to delete the default branch
    if args.name == repo_config.default_branch {
        bail!("Cannot delete the default branch '{}'. This would require changing the default branch first.", args.name);
    }
    
    let git_manager = GitManager::new();
    
    // Verify the branch exists
    let branches = git_manager.list_branches(&repo_path)?;
    if !branches.contains(&args.name) {
        bail!("Branch '{}' does not exist in repository '{}'", args.name, repo_name);
    }
    
    // Confirmation prompt unless --yes is specified
    if !args.yes {
        println!("Are you sure you want to delete branch '{}' from repository '{}'?", 
            args.name.red(), repo_name.cyan());
        if args.force {
            println!("{}", "Warning: Force deletion will remove the branch even if it has unmerged changes.".yellow());
        }
        print!("Type 'yes' to confirm: ");
        use std::io::{self, Write};
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        if input.trim().to_lowercase() != "yes" {
            println!("Branch deletion cancelled.");
            return Ok(());
        }
    }
    
    // Delete the branch
    git_manager.delete_branch(&repo_path, &args.name)?;
    
    println!(
        "{}",
        format!("Deleted branch '{}' from repository '{}'", args.name, repo_name).green()
    );
    
    // Update config to remove from tracked branches
    let repo_config_mut = &mut config.repositories[repo_config_index];
    repo_config_mut.tracked_branches.retain(|b| b != &args.name);
    
    sagitta_search::save_config(config, override_path)?;
    
    Ok(())
} 