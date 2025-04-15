use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use colored::*;
use qdrant_client::{Qdrant, qdrant::{Filter, DeletePointsBuilder, PointsSelector, points_selector::PointsSelectorOneOf}};
use std::{sync::Arc, path::PathBuf};
use super::helpers::{get_collection_name};
use log;
use crate::config::{self, AppConfig};

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

pub async fn handle_repo_clear(
    args: ClearRepoArgs, 
    config: &mut AppConfig, 
    client: Arc<Qdrant>,
    _override_path: Option<&PathBuf>,
) -> Result<()> {
    let repo_name = match args.name.as_ref().or(config.active_repository.as_ref()) {
        Some(name) => name.clone(),
        None => bail!("No active repository set and no repository name provided."),
    };

    let repo_config_index = config
        .repositories
        .iter()
        .position(|r| r.name == repo_name)
        .ok_or_else(|| anyhow!("Configuration for repository '{}' not found.", repo_name))?;

    println!(
        "{}",
        format!(
            "Preparing to clear the index for repository '{}'. This will remove all indexed data for this repository.",
            repo_name.cyan()
        ).yellow()
    );
    println!("{}", "This action CANNOT be undone.".red().bold());

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

    let collection_name = get_collection_name(&repo_name);

    // Check if collection exists before attempting to delete points
    match client.collection_exists(&collection_name).await {
        Ok(exists) => {
            if !exists {
                log::warn!("Collection '{}' does not exist. Nothing to clear.", collection_name);
                // // Optionally, still clear config status even if collection didn't exist?
                // let repo_config_mut = &mut config.repositories[repo_config_index];
                // repo_config_mut.last_synced_commits.clear();
                // repo_config_mut.indexed_languages = None;
                return Ok(()); // Return Ok if collection doesn't exist
            }
        }
        Err(e) => {
            // Propagate error if checking collection existence fails
            return Err(anyhow!("Failed to check existence of collection '{}': {}", collection_name, e));
        }
    }

    println!("Deleting all points from collection '{}'...", collection_name.cyan());

    // Reconstruct the builder and pass it to delete_points
    let selector = PointsSelector {
        points_selector_one_of: Some(PointsSelectorOneOf::Filter(Filter { 
            must: vec![], 
            should: vec![], 
            must_not: vec![], 
            min_should: None
        }))
    };
    let delete_request = DeletePointsBuilder::new(&collection_name)
        .points(selector.points_selector_one_of.unwrap())
        .wait(false);

    client.delete_points(delete_request).await?;

    // Clear the sync status in the config
    let repo_config_mut = &mut config.repositories[repo_config_index];
    repo_config_mut.last_synced_commits.clear();
    repo_config_mut.indexed_languages = None; // Also clear detected languages

    // Save the updated config
    config::save_config(config, _override_path)
        .context("Failed to save config after clearing repository index")?;

    println!(
        "{}",
        format!(
            "Successfully cleared index and sync status for repository '{}'.",
            repo_name.cyan()
        ).green()
    );

    Ok(())
} 