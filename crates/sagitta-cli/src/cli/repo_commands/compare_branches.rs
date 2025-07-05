use clap::Args;
use anyhow::{Result, Context, anyhow};
use std::path::PathBuf;
use std::sync::Arc;
use std::collections::HashMap;
use git_manager::GitManager;
use sagitta_search::{AppConfig, qdrant_client_trait::QdrantClientTrait};
use sagitta_search::repo_helpers::{get_branch_sync_metadata, BranchSyncMetadata};
use colored::*;
use crate::cli::CliArgs;

#[derive(Args, Debug, Clone)]
pub struct CompareBranchesArgs {
    /// Optional repository name (defaults to active repository)
    pub name: Option<String>,
    
    /// First branch to compare
    #[arg(short = 'a', long = "branch-a", value_name = "BRANCH")]
    pub branch_a: Option<String>,
    
    /// Second branch to compare
    #[arg(short = 'b', long = "branch-b", value_name = "BRANCH")]
    pub branch_b: Option<String>,
    
    /// Compare all tracked branches (matrix comparison)
    #[arg(short = 'A', long)]
    pub all: bool,
    
    /// Show detailed file differences
    #[arg(short = 'd', long)]
    pub detailed: bool,
    
    /// Include sync metadata in comparison
    #[arg(short = 's', long)]
    pub sync_status: bool,
}

pub async fn handle_compare_branches<C>(
    args: CompareBranchesArgs,
    config: &AppConfig,
    client: Arc<C>,
    cli_args: &CliArgs,
) -> Result<()>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    let repo_name = args.name.as_ref()
        .or(config.active_repository.as_ref())
        .ok_or_else(|| anyhow!("No repository specified and no active repository set"))?;

    let repo_config = config
        .repositories
        .iter()
        .find(|r| r.name == *repo_name)
        .ok_or_else(|| anyhow!("Repository '{}' not found", repo_name))?;

    let repo_path = PathBuf::from(&repo_config.local_path);
    

    // Initialize git manager
    let mut git_manager = GitManager::new();
    git_manager.initialize_repository(&repo_path).await
        .context("Failed to initialize repository")?;

    if args.all {
        // Compare all tracked branches
        let branches = if repo_config.tracked_branches.is_empty() {
            git_manager.list_branches(&repo_path)?
        } else {
            repo_config.tracked_branches.clone()
        };

        if branches.len() < 2 {
            println!("{}", "Need at least 2 branches for comparison".yellow());
            return Ok(());
        }

        println!("🔍 Branch comparison matrix for repository '{}'", repo_name.cyan().bold());
        println!();

        // Get sync metadata for all branches if requested
        let mut branch_metadata: HashMap<String, Option<BranchSyncMetadata>> = HashMap::new();
        if args.sync_status {
            for branch in &branches {
                let metadata = get_branch_sync_metadata(
                    client.as_ref(),
                    &repo_config.name,
                    branch,
                    config,
                ).await.context("Failed to get branch sync metadata")?;
                branch_metadata.insert(branch.clone(), metadata);
            }
        }

        // Display matrix
        print!("{:>15}", "");
        for branch in &branches {
            print!("{branch:>15}");
        }
        println!();

        for (i, branch_a) in branches.iter().enumerate() {
            print!("{branch_a:>15}");
            for (j, branch_b) in branches.iter().enumerate() {
                if i == j {
                    print!("{:>15}", "●".blue());
                } else if i < j {
                    // Compare branches
                    let comparison = compare_two_branches(
                        &git_manager,
                        &repo_path,
                        branch_a,
                        branch_b,
                        &branch_metadata,
                    ).await?;
                    print!("{:>15}", format_comparison_result(&comparison));
                } else {
                    print!("{:>15}", "");
                }
            }
            println!();
        }

        if args.sync_status {
            println!("\n📊 Sync Status Summary:");
            for branch in &branches {
                if let Some(Some(metadata)) = branch_metadata.get(branch) {
                    println!("  {} {} - {} files indexed", 
                        "●".green(), 
                        branch.cyan(), 
                        metadata.files_count);
                } else {
                    println!("  {} {} - {}", 
                        "○".yellow(), 
                        branch.cyan(), 
                        "not synced".yellow());
                }
            }
        }

    } else {
        // Compare two specific branches
        let branch_a = args.branch_a.as_ref()
            .or(repo_config.active_branch.as_ref())
            .ok_or_else(|| anyhow!("No branch-a specified and no active branch set"))?;

        let branch_b = args.branch_b.as_ref()
            .ok_or_else(|| anyhow!("branch-b must be specified for two-branch comparison"))?;

        println!("🔍 Comparing branches '{}' vs '{}' in repository '{}'", 
            branch_a.cyan(), branch_b.cyan(), repo_name.cyan().bold());

        // Get sync metadata if requested
        let mut branch_metadata: HashMap<String, Option<BranchSyncMetadata>> = HashMap::new();
        if args.sync_status {
            for branch in [branch_a, branch_b] {
                let metadata = get_branch_sync_metadata(
                    client.as_ref(),
                    &repo_config.name,
                    branch,
                    config,
                ).await.context("Failed to get branch sync metadata")?;
                branch_metadata.insert(branch.clone(), metadata);
            }
        }

        let comparison = compare_two_branches(
            &git_manager,
            &repo_path,
            branch_a,
            branch_b,
            &branch_metadata,
        ).await?;

        display_detailed_comparison(branch_a, branch_b, &comparison, args.detailed, args.sync_status);
    }

    Ok(())
}

#[derive(Debug)]
struct BranchComparison {
    commits_ahead: usize,
    commits_behind: usize,
    files_different: usize,
    sync_status_a: Option<BranchSyncMetadata>,
    sync_status_b: Option<BranchSyncMetadata>,
    diverged: bool,
}

async fn compare_two_branches(
    git_manager: &GitManager,
    repo_path: &PathBuf,
    branch_a: &str,
    branch_b: &str,
    branch_metadata: &HashMap<String, Option<BranchSyncMetadata>>,
) -> Result<BranchComparison> {
    // For now, we'll do a basic comparison
    // In a full implementation, we'd use git2 to get actual commit differences
    
    // Get sync metadata
    let sync_status_a = branch_metadata.get(branch_a).and_then(|m| m.clone());
    let sync_status_b = branch_metadata.get(branch_b).and_then(|m| m.clone());
    
    // Placeholder comparison logic
    // In reality, we'd use git2 to compare commits and files
    let commits_ahead = 0; // Would calculate actual commits ahead
    let commits_behind = 0; // Would calculate actual commits behind
    let files_different = 0; // Would calculate actual file differences
    let diverged = false; // Would check if branches have diverged
    
    Ok(BranchComparison {
        commits_ahead,
        commits_behind,
        files_different,
        sync_status_a,
        sync_status_b,
        diverged,
    })
}

fn format_comparison_result(comparison: &BranchComparison) -> String {
    if comparison.commits_ahead == 0 && comparison.commits_behind == 0 {
        "≡".green().to_string()
    } else if comparison.diverged {
        "⚡".yellow().to_string()
    } else if comparison.commits_ahead > 0 {
        "↑".blue().to_string()
    } else {
        "↓".red().to_string()
    }
}

fn display_detailed_comparison(
    branch_a: &str,
    branch_b: &str,
    comparison: &BranchComparison,
    detailed: bool,
    sync_status: bool,
) {
    println!();
    
    // Git comparison
    if comparison.commits_ahead == 0 && comparison.commits_behind == 0 {
        println!("📍 Branches are at the same commit");
    } else {
        if comparison.commits_ahead > 0 {
            println!("📈 '{}' is {} commits ahead of '{}'", 
                branch_a.cyan(), comparison.commits_ahead, branch_b.cyan());
        }
        if comparison.commits_behind > 0 {
            println!("📉 '{}' is {} commits behind '{}'", 
                branch_a.cyan(), comparison.commits_behind, branch_b.cyan());
        }
        if comparison.diverged {
            println!("⚡ Branches have diverged");
        }
    }

    if comparison.files_different > 0 {
        println!("📄 {} files differ between branches", comparison.files_different);
    }

    // Sync status comparison
    if sync_status {
        println!("\n📊 Sync Status Comparison:");
        
        match (&comparison.sync_status_a, &comparison.sync_status_b) {
            (Some(meta_a), Some(meta_b)) => {
                println!("  {} '{}': {} files indexed", 
                    "●".green(), branch_a.cyan(), meta_a.files_count);
                println!("  {} '{}': {} files indexed", 
                    "●".green(), branch_b.cyan(), meta_b.files_count);
                
                let diff = meta_a.files_count as i32 - meta_b.files_count as i32;
                if diff > 0 {
                    println!("  📈 '{}' has {} more indexed files", branch_a.cyan(), diff);
                } else if diff < 0 {
                    println!("  📈 '{}' has {} more indexed files", branch_b.cyan(), -diff);
                } else {
                    println!("  ⚖️  Both branches have the same number of indexed files");
                }
            }
            (Some(meta_a), None) => {
                println!("  {} '{}': {} files indexed", 
                    "●".green(), branch_a.cyan(), meta_a.files_count);
                println!("  {} '{}': {}", 
                    "○".yellow(), branch_b.cyan(), "not synced".yellow());
            }
            (None, Some(meta_b)) => {
                println!("  {} '{}': {}", 
                    "○".yellow(), branch_a.cyan(), "not synced".yellow());
                println!("  {} '{}': {} files indexed", 
                    "●".green(), branch_b.cyan(), meta_b.files_count);
            }
            (None, None) => {
                println!("  {} Neither branch has been synced", "○".yellow());
            }
        }
    }

    if detailed {
        println!("\n📋 Detailed Analysis:");
        println!("  (Detailed file-level comparison would be implemented here)");
        // In a full implementation, we'd show:
        // - Files added/removed/modified between branches
        // - Sync timestamp differences
        // - Collection size differences
    }
} 