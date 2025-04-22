use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use colored::*;
use qdrant_client::qdrant::{Filter, PointsSelector, points_selector::PointsSelectorOneOf};
use std::{sync::Arc, path::PathBuf};
use vectordb_core::repo_helpers::{get_collection_name};
use log;
use vectordb_core::{AppConfig, save_config};
use vectordb_core::qdrant_client_trait::QdrantClientTrait;
use std::fmt::Debug;
use std::io::{self, Write};

#[derive(Args, Debug)]
#[derive(Clone)]
pub struct ClearRepoArgs {
    /// Optional name of the repository to clear (defaults to active repository).
    #[arg(short, long)]
    pub name: Option<String>,

    /// Skip confirmation prompt.
    #[arg(short, long, default_value_t = false)]
    pub yes: bool,
}

pub async fn handle_repo_clear<C>(
    args: ClearRepoArgs, 
    config: &mut AppConfig, 
    client: Arc<C>,
    override_path: Option<&PathBuf>,
) -> Result<()>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    let repo_name = match args.name.as_ref().or(config.active_repository.as_ref()) {
        Some(name) => name.clone(),
        None => bail!("No active repository set and no repository name provided."),
    };

    let repo_config_index = config
        .repositories
        .iter()
        .position(|r| r.name == repo_name)
        .ok_or_else(|| anyhow!("Configuration for repository '{}' not found.", repo_name))?;

    // --- Check Qdrant Collection Status (Informational) ---
    let collection_name = get_collection_name(&repo_name);
    let collection_did_exist = match client.collection_exists(collection_name.clone()).await {
        Ok(exists) => exists,
        Err(e) => {
            log::warn!("Failed to check existence of Qdrant collection '{}': {}. Proceeding with config clear anyway.", collection_name, e);
            false // Assume it didn't exist or is inaccessible
        }
    };

    if collection_did_exist {
        println!(
            "{}",
            format!(
                "Preparing to clear the index for repository '{}'. This will remove all indexed data for this repository.",
                repo_name.cyan()
            ).yellow()
        );
        println!("{}", "This action CANNOT be undone.".red().bold());

        if !args.yes {
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

        println!("Deleting all points from collection '{}'...", collection_name.cyan());
        // Clear all points using an empty filter
        let selector = PointsSelector {
            points_selector_one_of: Some(PointsSelectorOneOf::Filter(Filter {
                must: vec![], 
                should: vec![], 
                must_not: vec![], 
                min_should: None,
            })),
        };
        let delete_request = qdrant_client::qdrant::DeletePoints {
            collection_name: collection_name.to_string(),
            wait: Some(true), // Wait for operation to complete
            points: Some(selector),
            ordering: None,
            shard_key_selector: None,
        };
        client.delete_points(delete_request).await
            .with_context(|| format!("Failed to delete points from collection '{}'", collection_name))?;
            
    } else {
         log::warn!("Qdrant Collection '{}' does not exist or could not be verified. Skipping point deletion.", collection_name);
         // No confirmation needed if we are only clearing config state
    }

    // --- Always Clear Config Status and Save --- 
    println!("Clearing repository sync status in configuration...");
    let repo_config_mut = &mut config.repositories[repo_config_index];
    repo_config_mut.last_synced_commits.clear();
    repo_config_mut.indexed_languages = None; // Also clear detected languages

    // Save the modified config
    save_config(config, override_path)
        .context("Failed to save configuration after clearing repository state")?;
    println!("{}", "Repository index cleared and sync status reset.".green());

    Ok(())
} 