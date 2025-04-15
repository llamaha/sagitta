use anyhow::{bail, Context, Result};
use clap::Args;
use colored::*;
use qdrant_client::Qdrant;
use std::{fs, path::PathBuf, sync::Arc};
use log;

use crate::config::{self, AppConfig};
use crate::cli::repo_commands::helpers;

#[derive(Args, Debug)]
#[derive(Clone)]
pub struct RemoveRepoArgs {
    /// Name of the repository to remove.
    pub name: String,

    /// Skip confirmation prompt.
    #[arg(short, long, default_value_t = false)]
    pub yes: bool,
}

pub async fn handle_repo_remove(
    args: RemoveRepoArgs, 
    config: &mut AppConfig,
    client: Arc<Qdrant>,
    override_path: Option<&PathBuf>,
) -> Result<()> {
    let repo_name = &args.name;

    println!(
        "{}",
        format!(
            "Preparing to remove repository '{}'. This will delete configuration, Qdrant data, and the local clone.",
            repo_name.cyan()
        ).yellow()
    );
    println!("{}", "This action CANNOT be undone.".red().bold());

    // Add confirmation prompt if -y is not passed
    if !args.yes {
        println!("Are you sure you want to continue? (yes/No)");
        let mut confirmation = String::new();
        std::io::stdin().read_line(&mut confirmation)
            .context("Failed to read confirmation input")?;
        if confirmation.trim().to_lowercase() != "yes" {
            println!("Operation cancelled.");
            return Ok(());
        }
    }

    // Find the repository index
    let repo_config_index = match config.repositories.iter().position(|r| r.name == *repo_name) {
        Some(index) => index,
        None => bail!("Repository '{}' not found.", repo_name),
    };

    // Get details before removing from config
    let repo_config = config.repositories[repo_config_index].clone(); 

    // Attempt to delete the Qdrant collection
    let collection_name = helpers::get_collection_name(repo_name);
    println!("Attempting to delete Qdrant collection '{}'...", collection_name.cyan());
    match client.delete_collection(&collection_name).await {
        Ok(response) => {
            // Check the result field within the response
            if response.result {
                println!("Successfully deleted Qdrant collection '{}'.", collection_name.green());
            } else {
                // This case might indicate the collection didn't exist or another non-error condition
                 println!("Qdrant reported non-success deleting collection '{}'. It might not have existed.", collection_name.yellow());
            }
        }
        Err(e) => {
            // Log error but continue with removing config/local dir
            log::error!(
                "Failed to delete Qdrant collection '{}': {}. Please check Qdrant manually.", 
                collection_name, e
            );
            println!(
                "{}", 
                format!("Warning: Failed to delete Qdrant collection '{}'. Please check Qdrant manually.", collection_name).yellow()
            );
        }
    }

    // Attempt to remove the local repository directory
    println!("Attempting to remove local clone at {}...", repo_config.local_path.display());
    if repo_config.local_path.exists() {
        match fs::remove_dir_all(&repo_config.local_path) {
            Ok(_) => println!("Successfully removed local directory '{}'.", repo_config.local_path.display().to_string().green()),
            Err(e) => {
                // Log error but continue with removing config
                 log::error!("Failed to remove local directory '{}': {}. Please remove it manually.", repo_config.local_path.display(), e);
                 println!(
                    "{}", 
                    format!("Warning: Failed to remove local directory '{}'. Please remove it manually.", repo_config.local_path.display()).yellow()
                );
            }
        }
    } else {
        println!("Local directory '{}' does not exist. Skipping removal.", repo_config.local_path.display().to_string().yellow());
    }

    // Remove the repository from the config
    println!("Removing repository configuration for '{}'.", repo_name.cyan());
    config.repositories.remove(repo_config_index);

    // If the removed repository was the active one, unset it
    if config.active_repository.as_deref() == Some(repo_name) {
        config.active_repository = None;
        println!("Removed repository was active, active repository is now unset.");
        // Optionally, set active to the first remaining repo?
        if let Some(first_repo) = config.repositories.first() {
             config.active_repository = Some(first_repo.name.clone());
             println!("Set active repository to '{}'.", first_repo.name.cyan());
         }
    }

    // Save the updated configuration
    config::save_config(config, override_path)
        .context("Failed to save configuration after removing repository")?;

    println!("{}", format!("Successfully removed repository '{}'.", repo_name.cyan()).green());

    Ok(())
} 