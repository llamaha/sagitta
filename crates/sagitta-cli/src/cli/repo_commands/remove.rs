use clap::Args;
use anyhow::{Result, Context, anyhow};
use std::{fs, path::PathBuf, sync::Arc};
use std::io::{self, Write};
use colored::*;
use log::info;

// Use config types from sagitta_search
use sagitta_search::{AppConfig, save_config};
use sagitta_search::qdrant_client_trait::QdrantClientTrait;
// use crate::utils::prompt_for_confirmation;

#[derive(Args, Debug)]
#[derive(Clone)]
pub struct RemoveRepoArgs {
    /// Name of the repository to remove.
    #[arg(required = true)]
    pub name: String,

    /// Skip confirmation prompt.
    #[arg(short, long, default_value_t = false)]
    pub yes: bool,
    
    /// Also delete the local repository clone directory.
    #[arg(long, default_value_t = true)]
    pub delete_local: bool,
}

pub async fn handle_repo_remove<C>(
    args: RemoveRepoArgs, 
    config: &mut AppConfig,
    client: Arc<C>,
    cli_args: &crate::cli::CliArgs,
    override_path: Option<&PathBuf>,
) -> Result<()>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    let repo_name_to_remove = args.name.clone();


    let repo_config_index = config
        .repositories
        .iter()
        .position(|r| r.name == repo_name_to_remove)
        .ok_or_else(|| anyhow!("Repository '{}' not found.", repo_name_to_remove))?;

    if !args.yes {
        println!(
            "{}",
            format!(
                "Preparing to remove repository '{}'.",
                repo_name_to_remove.cyan()
            ).yellow()
        );
        println!("This will remove the repository configuration AND delete the vector index data in Qdrant.");
        if args.delete_local {
             println!("{}", "It will ALSO delete the local clone directory.".bold());
        }
        println!("{}", "This action CANNOT be undone.".red().bold());

        print!("Are you sure you want to continue? (yes/No): ");
        io::stdout().flush().context("Failed to flush stdout")?;
        let mut confirmation = String::new();
        io::stdin().read_line(&mut confirmation)
            .context("Failed to read confirmation input")?;
        if confirmation.trim().to_lowercase() != "yes" {
            println!("Operation cancelled.");
            return Ok(());
        }
    }

    let repo_config_clone = config.repositories[repo_config_index].clone();

    if args.delete_local {
        info!("Calling core delete_repository_data (includes local files) for repo: {}", repo_name_to_remove);
        sagitta_search::repo_helpers::delete_repository_data(&repo_config_clone, client.clone(), config).await
            .with_context(|| format!("Failed to delete repository data for '{}'", repo_name_to_remove))?;
    } else {
        info!("Deleting Qdrant collection(s) for this repository");
        let repo_config = &config.repositories[repo_config_index];
        let current_branch = repo_config.target_ref.as_deref()
            .or(repo_config.active_branch.as_deref())
            .unwrap_or(&repo_config.default_branch);
        
        // Try to delete current branch collection
        let current_collection_name = sagitta_search::repo_helpers::get_branch_aware_collection_name(&repo_name_to_remove, current_branch, config);
        match client.delete_collection(current_collection_name.clone()).await {
            Ok(_) => {
                println!("✓ Deleted Qdrant collection: {}", current_collection_name);
            }
            Err(e) => {
                eprintln!("⚠ Warning: Failed to delete current branch collection '{}': {}", current_collection_name, e);
            }
        }
        
        // Also try to delete legacy collection for backward compatibility
        let legacy_collection_name = sagitta_search::repo_helpers::get_collection_name(&repo_name_to_remove, config);
        info!("Deleting Qdrant collection '{}' (delete_local is false).", legacy_collection_name);
        match client.delete_collection(legacy_collection_name.clone()).await {
            Ok(deleted) => {
                if deleted { info!("Successfully deleted Qdrant collection '{}'.", legacy_collection_name); }
                else { info!("Qdrant collection '{}' did not exist or was already deleted.", legacy_collection_name); }
            }
            Err(e) => log::error!("Failed to delete Qdrant collection '{}': {}. Manual cleanup may be needed.", legacy_collection_name, e),
        }
        info!("Local files for '{}' will NOT be deleted as per --delete-local=false.", repo_name_to_remove);
    }

    info!("Removing repository configuration for '{}'.", repo_name_to_remove.cyan());
    config.repositories.remove(repo_config_index);

    if config.active_repository.as_ref() == Some(&repo_name_to_remove) {
        info!("Unsetting active repository as it was removed.");
        config.active_repository = None;
    }

    save_config(config, override_path)
        .context("Failed to save configuration after removing repository")?;

    println!("{}", "Successfully removed repository".green());
    Ok(())
} 