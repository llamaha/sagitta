use clap::Args;
use anyhow::{Result, Context, anyhow};
use std::{fs, path::PathBuf, sync::Arc};
use std::io::{self, Write};
use colored::*;
use log::info;

// Use config types from vectordb_core
use vectordb_core::{AppConfig, save_config};
use vectordb_core::qdrant_client_trait::QdrantClientTrait;
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
    #[arg(long, default_value_t = false)]
    pub delete_local: bool,
}

pub async fn handle_repo_remove<C>(
    args: RemoveRepoArgs, 
    config: &mut AppConfig,
    client: Arc<C>,
    override_path: Option<&PathBuf>,
) -> Result<()>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    let repo_name = args.name.clone();

    let repo_config_index = config
        .repositories
        .iter()
        .position(|r| r.name == args.name)
        .ok_or_else(|| anyhow!("Configuration for repository '{}' not found.", args.name))?;

    if !args.yes {
        println!(
            "{}",
            format!(
                "Preparing to remove repository '{}'.",
                args.name.cyan()
            ).yellow()
        );
        println!("This will remove the repository configuration AND delete the vector index data in Qdrant.");
        if args.delete_local {
             println!("{}", "It will ALSO delete the local clone directory.".bold());
        }
        println!("{}", "This action CANNOT be undone.".red().bold());

        // Replace prompt_for_confirmation call with direct logic
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

    // Get repo config clone before removing
    let repo_config_clone = config.repositories[repo_config_index].clone();

    // Use the core function to delete Qdrant collection
    let collection_name = vectordb_core::repo_helpers::get_collection_name(&repo_name);
    info!("Attempting to delete Qdrant collection '{}'...", collection_name);
    match client.delete_collection(collection_name.clone()).await {
        Ok(deleted) => {
            if deleted {
                info!("Successfully deleted Qdrant collection '{}'.", collection_name);
            } else {
                info!("Qdrant collection '{}' did not exist or was already deleted.", collection_name);
            }
        }
        Err(e) => {
            // Log error but continue, as config removal might still be desired
            log::error!("Failed to delete Qdrant collection '{}': {}. Continuing removal process.", collection_name, e);
        }
    }

    // Remove the repository from the config (CLI-specific part)
    info!("Removing repository configuration for '{}'.", repo_name.cyan());
    config.repositories.remove(repo_config_index);

    // If the removed repository was the active one, unset it
    if config.active_repository.as_ref() == Some(&repo_name) {
        info!("Unsetting active repository as it was removed.");
        config.active_repository = None;
    }

    // Optionally delete local directory
    if args.delete_local {
        let local_path = &repo_config_clone.local_path;
        if local_path.exists() {
            info!("Attempting to remove local clone at {}...", local_path.display());
            match fs::remove_dir_all(local_path) {
                Ok(_) => info!("Successfully removed local directory '{}'.", local_path.display()),
                Err(e) => {
                    log::error!("Failed to remove local directory '{}': {}. Please remove it manually.", local_path.display(), e);
                    // Don't fail the whole operation, just warn
                    println!(
                        "{}",
                        format!("Warning: Failed to remove local directory '{}': {}. Please remove it manually.", local_path.display(), e).yellow()
                    );
                }
            }
        } else {
            info!("Local directory '{}' does not exist. Skipping removal.", local_path.display());
        }
    }

    // Save the modified config
    save_config(config, override_path)
        .context("Failed to save configuration after removing repository")?;

    println!("{}", "Repository configuration removed.".green());
    Ok(())
} 