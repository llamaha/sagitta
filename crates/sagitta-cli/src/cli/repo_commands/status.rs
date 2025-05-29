use anyhow::Result;
use clap::Args;
use colored::*;
use git_manager::GitManager;
use sagitta_search::AppConfig;
use std::path::PathBuf;

#[derive(Args, Debug, Clone)]
pub struct StatusArgs {
    /// Optional repository name (defaults to active repository)
    pub name: Option<String>,
    
    /// Show detailed file status
    #[arg(short = 'd', long)]
    pub detailed: bool,
}

pub async fn handle_status(
    args: StatusArgs,
    config: &AppConfig,
) -> Result<()> {
    let repo_name = args.name.as_ref()
        .or(config.active_repository.as_ref())
        .ok_or_else(|| anyhow::anyhow!("No repository specified and no active repository set"))?;
    
    let repo_config = config.repositories.iter()
        .find(|r| r.name == *repo_name)
        .ok_or_else(|| anyhow::anyhow!("Repository '{}' not found", repo_name))?;
    
    let repo_path = PathBuf::from(&repo_config.local_path);
    let mut git_manager = GitManager::new();
    
    // Initialize and get repository info
    let repo_info = git_manager.initialize_repository(&repo_path).await?;
    
    println!("Repository: {}", repo_name.cyan().bold());
    println!("Path: {}", repo_path.display().to_string().dimmed());
    println!("Current branch: {}", repo_info.current_branch.green());
    println!("Current commit: {}", repo_info.current_commit[..8].yellow());
    
    // Show tracked branches
    if !repo_config.tracked_branches.is_empty() {
        println!("\nTracked branches:");
        for branch in &repo_config.tracked_branches {
            if branch == &repo_info.current_branch {
                println!("  {} {}", "*".green(), branch.green());
            } else {
                println!("    {}", branch);
            }
        }
    }
    
    // Check for uncommitted changes
    let has_changes = git_manager.has_uncommitted_changes(&repo_path)?;
    if has_changes {
        println!("\n{}", "Working directory has uncommitted changes".yellow());
        
        if args.detailed {
            let status_entries = git_manager.get_status(&repo_path)?;
            if !status_entries.is_empty() {
                println!("\nFile status:");
                for (path, status) in status_entries {
                    let status_str = format_git_status(status);
                    println!("  {} {}", status_str, path.display());
                }
            }
        } else {
            println!("  Use --detailed to see file-level changes");
        }
    } else {
        println!("\n{}", "Working directory is clean".green());
    }
    
    // Show sync status if available
    if let Some(active_branch) = &repo_config.active_branch {
        if let Some(last_commit) = repo_config.last_synced_commits.get(active_branch) {
            println!("\nLast synced commit: {}", last_commit[..8].cyan());
            if last_commit != &repo_info.current_commit {
                println!("{}", "Repository has new commits since last sync".yellow());
            } else {
                println!("{}", "Repository is synced with vector database".green());
            }
        } else {
            println!("\n{}", "Repository has not been synced yet".yellow());
        }
    }
    
    Ok(())
}

fn format_git_status(status: git2::Status) -> ColoredString {
    if status.contains(git2::Status::INDEX_NEW) {
        "A ".green()
    } else if status.contains(git2::Status::INDEX_MODIFIED) {
        "M ".green()
    } else if status.contains(git2::Status::INDEX_DELETED) {
        "D ".green()
    } else if status.contains(git2::Status::INDEX_RENAMED) {
        "R ".green()
    } else if status.contains(git2::Status::INDEX_TYPECHANGE) {
        "T ".green()
    } else if status.contains(git2::Status::WT_NEW) {
        "??".red()
    } else if status.contains(git2::Status::WT_MODIFIED) {
        " M".red()
    } else if status.contains(git2::Status::WT_DELETED) {
        " D".red()
    } else if status.contains(git2::Status::WT_TYPECHANGE) {
        " T".red()
    } else if status.contains(git2::Status::WT_RENAMED) {
        " R".red()
    } else {
        "  ".normal()
    }
} 