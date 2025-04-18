use anyhow::{bail, Context, Result};
use clap::Args;
use colored::*;
use std::{path::PathBuf, sync::Arc, fmt::Debug};

use crate::config::{self, AppConfig};
use crate::vectordb::qdrant_client_trait::QdrantClientTrait;
use crate::cli::repo_commands::helpers::delete_repository_data;

#[derive(Args, Debug)]
#[derive(Clone)]
pub struct RemoveRepoArgs {
    /// Name of the repository to remove.
    pub name: String,

    /// Skip confirmation prompt.
    #[arg(short, long, default_value_t = false)]
    pub yes: bool,
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
    let repo_name = &args.name;

    // Add confirmation prompt if -y is not passed
    if !args.yes {
         println!(
            "{}",
            format!(
                "Preparing to remove repository '{}'. This will delete configuration, Qdrant data, and the local clone.",
                repo_name.cyan()
            ).yellow()
        );
        println!("{}", "This action CANNOT be undone.".red().bold());
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

    // Attempt to delete the Qdrant collection and local data using the helper
    println!("Removing associated data for '{}'...", repo_name.cyan());
    delete_repository_data(&repo_config, client.clone()).await
        .context("Failed during repository data deletion (Qdrant collection / local files)")?;
    println!("Data removal process completed.");

    // Remove the repository from the config (CLI-specific part)
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