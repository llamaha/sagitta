use clap::Args;
use anyhow::{Result, Context, anyhow};
use std::path::PathBuf;
use std::sync::Arc;
use std::collections::HashSet;
use git_manager::GitManager;
use sagitta_search::{AppConfig, save_config, qdrant_client_trait::QdrantClientTrait};
use sagitta_search::repo_helpers::get_branch_sync_metadata;
use colored::*;
use crate::cli::CliArgs;

#[derive(Args, Debug, Clone)]
pub struct CleanupBranchesArgs {
    /// Optional repository name (defaults to active repository)
    pub name: Option<String>,
    
    /// Only show what would be cleaned up without actually cleaning
    #[arg(short = 'n', long)]
    pub dry_run: bool,
    
    /// Force cleanup without confirmation prompts
    #[arg(short = 'f', long)]
    pub force: bool,
    
    /// Clean up collections for branches that no longer exist in Git
    #[arg(short = 'g', long)]
    pub git_cleanup: bool,
    
    /// Clean up collections for branches not in tracked_branches list
    #[arg(short = 't', long)]
    pub tracked_cleanup: bool,
    
    /// Clean up empty collections (0 files indexed)
    #[arg(short = 'e', long)]
    pub empty_cleanup: bool,
    
    /// Clean up all types (equivalent to -g -t -e)
    #[arg(short = 'a', long)]
    pub all: bool,
}

pub async fn handle_cleanup_branches<C>(
    args: CleanupBranchesArgs,
    config: &mut AppConfig,
    client: Arc<C>,
    cli_args: &CliArgs,
    override_path: Option<&PathBuf>,
) -> Result<()>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    let repo_name = args.name.as_ref()
        .or(config.active_repository.as_ref())
        .ok_or_else(|| anyhow!("No repository specified and no active repository set"))?;

    let repo_config_index = config
        .repositories
        .iter()
        .position(|r| r.name == *repo_name)
        .ok_or_else(|| anyhow!("Repository '{}' not found", repo_name))?;

    let repo_config = config.repositories[repo_config_index].clone();
    let repo_path = PathBuf::from(&repo_config.local_path);
    

    // Determine cleanup types
    let git_cleanup = args.all || args.git_cleanup;
    let tracked_cleanup = args.all || args.tracked_cleanup;
    let empty_cleanup = args.all || args.empty_cleanup;

    if !git_cleanup && !tracked_cleanup && !empty_cleanup {
        println!("{}", "No cleanup type specified. Use --git-cleanup, --tracked-cleanup, --empty-cleanup, or --all".yellow());
        return Ok(());
    }

    println!("🧹 Branch cleanup for repository '{}'", repo_name.cyan().bold());
    
    if args.dry_run {
        println!("{}", "DRY RUN - No actual cleanup will be performed".yellow().bold());
    }

    // Initialize git manager to get current branches
    let mut git_manager = GitManager::new();
    git_manager.initialize_repository(&repo_path).await
        .context("Failed to initialize repository")?;

    let current_git_branches: HashSet<String> = git_manager.list_branches(&repo_path)?
        .into_iter().collect();
    let tracked_branches: HashSet<String> = repo_config.tracked_branches.iter().cloned().collect();

    // Find all collections for this repository
    let collections = find_repository_collections(client.as_ref(), &repo_config.name, config).await?;
    
    println!("📊 Found {} collections for repository '{}'", collections.len(), repo_name);

    let mut collections_to_delete = Vec::new();
    let mut cleanup_reasons = Vec::new();

    // Analyze each collection
    for collection_name in &collections {
        // Extract branch name from collection name
        if let Some(branch_name) = extract_branch_from_collection_name(collection_name, &repo_config.name, config) {
            let mut should_delete = false;
            let mut reasons = Vec::new();

            // Check git cleanup
            if git_cleanup && !current_git_branches.contains(&branch_name) {
                should_delete = true;
                reasons.push(format!("branch '{branch_name}' no longer exists in Git"));
            }

            // Check tracked cleanup
            if tracked_cleanup && !tracked_branches.contains(&branch_name) {
                should_delete = true;
                reasons.push(format!("branch '{branch_name}' not in tracked branches"));
            }

            // Check empty cleanup
            if empty_cleanup {
                if let Ok(Some(metadata)) = get_branch_sync_metadata(
                    client.as_ref(),
                    &repo_config.name,
                    &branch_name,
                    config,
                ).await {
                    if metadata.files_count == 0 {
                        should_delete = true;
                        reasons.push(format!("collection for branch '{branch_name}' is empty"));
                    }
                }
            }

            if should_delete {
                collections_to_delete.push(collection_name.clone());
                cleanup_reasons.push((collection_name.clone(), branch_name, reasons));
            }
        }
    }

    if collections_to_delete.is_empty() {
        println!("\n{}", "No collections need cleanup!".green().bold());
        return Ok(());
    }

    // Display what will be cleaned up
    println!("\n🗑️  Collections to be cleaned up:");
    for (collection_name, branch_name, reasons) in &cleanup_reasons {
        println!("  {} {} ({})", 
            "●".red(), 
            collection_name.yellow(), 
            reasons.join(", "));
    }

    let total_size = collections_to_delete.len();
    println!("\n📈 Cleanup Summary:");
    println!("  Collections to delete: {}", total_size.to_string().red());

    if args.dry_run {
        println!("\n{}", "DRY RUN completed - no collections were deleted".yellow().bold());
        return Ok(());
    }

    // Confirmation prompt
    if !args.force {
        print!("\n❓ Are you sure you want to delete {total_size} collections? [y/N]: ");
        use std::io::{self, Write};
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        io::stdin().read_line(&mut input).context("Failed to read user input")?;
        
        if !input.trim().to_lowercase().starts_with('y') {
            println!("Cleanup cancelled.");
            return Ok(());
        }
    }

    // Perform cleanup
    println!("\n🗑️  Starting cleanup...");
    let mut deleted_count = 0;
    let mut failed_count = 0;

    for collection_name in &collections_to_delete {
        match client.delete_collection(collection_name.clone()).await {
            Ok(_) => {
                println!("  ✅ Deleted collection '{}'", collection_name.green());
                deleted_count += 1;
            }
            Err(e) => {
                println!("  ❌ Failed to delete collection '{}': {}", collection_name.red(), e);
                failed_count += 1;
            }
        }
    }

    // Update tracked branches if we did tracked cleanup
    if tracked_cleanup {
        let repo_config_mut = &mut config.repositories[repo_config_index];
        let original_count = repo_config_mut.tracked_branches.len();
        
        // Remove branches that no longer exist in Git
        repo_config_mut.tracked_branches.retain(|branch| current_git_branches.contains(branch));
        
        let removed_count = original_count - repo_config_mut.tracked_branches.len();
        if removed_count > 0 {
            println!("  🔧 Removed {removed_count} non-existent branches from tracked list");
            save_config(config, override_path)?;
        }
    }

    // Final summary
    println!("\n📊 Cleanup completed!");
    println!("  ✅ Successfully deleted: {}", deleted_count.to_string().green());
    if failed_count > 0 {
        println!("  ❌ Failed to delete: {}", failed_count.to_string().red());
    }

    if deleted_count > 0 {
        println!("  💾 Storage space has been freed up");
    }

    Ok(())
}

async fn find_repository_collections<C>(
    client: &C,
    repo_name: &str,
    config: &AppConfig,
) -> Result<Vec<String>>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    // Get all collections and filter for this repository
    let all_collections = client.list_collections().await
        .context("Failed to list collections")?;

    let prefix = format!("{}{}", 
        config.performance.collection_name_prefix, 
        repo_name);

    let repo_collections: Vec<String> = all_collections
        .into_iter()
        .filter(|name| name.starts_with(&prefix))
        .collect();

    Ok(repo_collections)
}

fn extract_branch_from_collection_name(
    collection_name: &str,
    repo_name: &str,
    config: &AppConfig,
) -> Option<String> {
    let prefix = format!("{}{}_br_", 
        config.performance.collection_name_prefix, 
        repo_name);

    if collection_name.starts_with(&prefix) {
        // For branch-aware collections, we can't easily extract the original branch name
        // from the hash, so we'll return a placeholder
        // In a full implementation, we'd store branch name metadata in the collection
        Some(format!("branch_hash_{}", &collection_name[prefix.len()..]))
    } else {
        // Legacy collection without branch info
        None
    }
} 