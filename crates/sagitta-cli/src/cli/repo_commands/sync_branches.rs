use clap::Args;
use anyhow::{Result, Context, bail, anyhow};
use std::path::PathBuf;
use std::sync::Arc;
use std::collections::HashMap;
use git_manager::GitManager;
use sagitta_search::{AppConfig, save_config, qdrant_client_trait::QdrantClientTrait};
use sagitta_search::sync::{sync_repository, SyncOptions, SyncResult};
use sagitta_search::config::RepositoryConfig;
use sagitta_search::repo_helpers::{get_branch_sync_metadata, BranchSyncMetadata};
use colored::*;
use tokio::task::JoinSet;
use crate::cli::CliArgs;

#[derive(Args, Debug, Clone)]
pub struct SyncBranchesArgs {
    /// Optional repository name (defaults to active repository)
    pub name: Option<String>,
    
    /// Specific branches to sync (defaults to all tracked branches)
    #[arg(short = 'b', long = "branch", value_name = "BRANCH")]
    pub branches: Vec<String>,
    
    /// Force sync even if branches appear up to date
    #[arg(short = 'f', long)]
    pub force: bool,
    
    /// Maximum number of branches to sync in parallel
    #[arg(short = 'j', long, default_value = "3")]
    pub parallel: usize,
    
    /// Only show what would be synced without actually syncing
    #[arg(short = 'n', long)]
    pub dry_run: bool,
    
    /// Include remote branches in sync
    #[arg(short = 'r', long)]
    pub include_remote: bool,
}

pub async fn handle_sync_branches<C>(
    args: SyncBranchesArgs,
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
    
    // Get tenant ID
    let tenant_id = cli_args.tenant_id.as_deref()
        .ok_or_else(|| anyhow!("--tenant-id is required for sync operations"))?;

    // Initialize git manager to get available branches
    let mut git_manager = GitManager::new();
    git_manager.initialize_repository(&repo_path).await
        .context("Failed to initialize repository")?;

    // Determine which branches to sync
    let branches_to_sync = if args.branches.is_empty() {
        // Use tracked branches or discover branches
        let mut branches = repo_config.tracked_branches.clone();
        
        if args.include_remote {
            let all_branches = git_manager.list_branches(&repo_path)?;
            for branch in all_branches {
                if !branches.contains(&branch) {
                    branches.push(branch);
                }
            }
        }
        
        if branches.is_empty() {
            // Fallback to current branch
            let repo_info = git_manager.get_repository_info(&repo_path)?;
            branches.push(repo_info.current_branch);
        }
        
        branches
    } else {
        args.branches.clone()
    };

    if branches_to_sync.is_empty() {
        bail!("No branches to sync. Use --branch to specify branches or ensure repository has tracked branches.");
    }

    println!("🔄 Multi-branch sync for repository '{}'", repo_name.cyan().bold());
    println!("Branches to sync: {}", branches_to_sync.join(", ").yellow());
    
    if args.dry_run {
        println!("{}", "DRY RUN - No actual syncing will be performed".yellow().bold());
    }

    // Check sync status for each branch
    let mut branch_metadata: HashMap<String, Option<BranchSyncMetadata>> = HashMap::new();
    let mut branches_needing_sync = Vec::new();

    println!("\n📊 Checking sync status...");
    for branch in &branches_to_sync {
        let metadata = get_branch_sync_metadata(
            client.as_ref(),
            tenant_id,
            &repo_config.name,
            branch,
            config,
        ).await.context("Failed to get branch sync metadata")?;
        
        let needs_sync = if args.force {
            true
        } else {
            // Check if branch needs sync by comparing with current commit
            // For now, we'll assume it needs sync if no metadata exists
            metadata.is_none() || metadata.as_ref().unwrap().files_count == 0
        };
        
        if needs_sync {
            branches_needing_sync.push(branch.clone());
            println!("  {} {} - {}", "●".yellow(), branch, "needs sync".yellow());
        } else {
            println!("  {} {} - {}", "✓".green(), branch, "up to date".green());
        }
        
        branch_metadata.insert(branch.clone(), metadata);
    }

    if branches_needing_sync.is_empty() {
        println!("\n{}", "All branches are up to date!".green().bold());
        return Ok(());
    }

    if args.dry_run {
        println!("\n{} branches would be synced: {}", 
            branches_needing_sync.len(), 
            branches_needing_sync.join(", "));
        return Ok(());
    }

    // Perform parallel sync
    println!("\n🚀 Starting parallel sync of {} branches (max {} concurrent)...", 
        branches_needing_sync.len(), args.parallel);

    let mut join_set = JoinSet::new();
    let mut active_syncs = 0;
    let mut branch_iter = branches_needing_sync.into_iter();
    let mut results: HashMap<String, Result<SyncResult>> = HashMap::new();

    // Start initial batch of syncs
    while active_syncs < args.parallel {
        if let Some(branch) = branch_iter.next() {
            let client_clone = Arc::clone(&client);
            let config_clone = config.clone();
            let mut repo_config_clone = repo_config.clone();
            
            // Set the branch as active for this sync
            repo_config_clone.active_branch = Some(branch.clone());
            repo_config_clone.target_ref = None; // Clear target_ref when syncing branches
            
            let sync_options = SyncOptions {
                force: args.force,
                extensions: None,
            };
            
            println!("  🔄 Starting sync for branch '{}'...", branch.cyan());
            
            join_set.spawn(async move {
                let result = sync_repository(
                    client_clone,
                    &repo_config_clone,
                    sync_options,
                    &config_clone,
                    None, // No progress reporter for now
                ).await;
                (branch, result)
            });
            
            active_syncs += 1;
        } else {
            break;
        }
    }

    // Process completed syncs and start new ones
    while !join_set.is_empty() {
        if let Some(result) = join_set.join_next().await {
            match result {
                Ok((branch, sync_result)) => {
                    match &sync_result {
                        Ok(result) => {
                            if result.success {
                                println!("  ✅ {} - {} files indexed", 
                                    branch.green(), result.files_indexed);
                            } else {
                                println!("  ❌ {} - {}", 
                                    branch.red(), result.message);
                            }
                        }
                        Err(e) => {
                            println!("  ❌ {} - Error: {}", branch.red(), e);
                        }
                    }
                    results.insert(branch, sync_result.map_err(|e| anyhow::Error::from(e)));
                }
                Err(e) => {
                    println!("  ❌ Task error: {}", e);
                }
            }
            
            active_syncs -= 1;
            
            // Start next sync if available
            if let Some(branch) = branch_iter.next() {
                let client_clone = Arc::clone(&client);
                let config_clone = config.clone();
                let mut repo_config_clone = repo_config.clone();
                
                repo_config_clone.active_branch = Some(branch.clone());
                repo_config_clone.target_ref = None;
                
                let sync_options = SyncOptions {
                    force: args.force,
                    extensions: None,
                };
                
                println!("  🔄 Starting sync for branch '{}'...", branch.cyan());
                
                join_set.spawn(async move {
                    let result = sync_repository(
                        client_clone,
                        &repo_config_clone,
                        sync_options,
                        &config_clone,
                        None,
                    ).await;
                    (branch, result)
                });
                
                active_syncs += 1;
            }
        }
    }

    // Summary
    let successful_syncs = results.values().filter(|r| r.is_ok() && r.as_ref().unwrap().success).count();
    let failed_syncs = results.len() - successful_syncs;
    
    println!("\n📈 Multi-branch sync completed!");
    println!("  ✅ Successful: {}", successful_syncs.to_string().green());
    if failed_syncs > 0 {
        println!("  ❌ Failed: {}", failed_syncs.to_string().red());
    }
    
    // Update config with any new tracked branches
    let repo_config_mut = &mut config.repositories[repo_config_index];
    for branch in &branches_to_sync {
        if !repo_config_mut.tracked_branches.contains(branch) {
            repo_config_mut.tracked_branches.push(branch.clone());
        }
    }
    
    save_config(config, override_path)?;

    Ok(())
} 