use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use colored::*;
use std::{sync::Arc, path::PathBuf};
use sagitta_search::repo_helpers::{get_collection_name};
use log;
use sagitta_search::{AppConfig, save_config};
use sagitta_search::qdrant_client_trait::QdrantClientTrait;
use sagitta_search::qdrant_ops::{delete_collection_by_name, delete_all_points};
use std::fmt::Debug;
use std::io::{self, Write};
use git_manager::GitManager;

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
    cli_args: &crate::cli::CliArgs,
    override_path: Option<&PathBuf>,
) -> Result<()>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    let cli_tenant_id = match cli_args.tenant_id.as_deref() {
        Some(id) => id,
        None => {
            config.tenant_id.as_deref().ok_or_else(|| anyhow!("--tenant-id is required (or set tenant_id in config) to clear a repository."))?
        }
    };

    let repo_name_to_clear = match args.name.as_ref().or(config.active_repository.as_ref()) {
        Some(name) => name.clone(),
        None => bail!("No active repository set and no repository name provided for tenant '{}'.", cli_tenant_id),
    };

    let repo_config_index = config
        .repositories
        .iter()
        .position(|r| r.name == repo_name_to_clear && r.tenant_id.as_deref() == Some(cli_tenant_id))
        .ok_or_else(|| anyhow!("Configuration for repository '{}' under tenant '{}' not found.", repo_name_to_clear, cli_tenant_id))?;

    let collection_name = get_collection_name(cli_tenant_id, &repo_name_to_clear, &config);
    let collection_existed_before_clear = match client.collection_exists(collection_name.clone()).await {
        Ok(exists) => exists,
        Err(e) => {
            log::warn!("Failed to check existence of Qdrant collection '{}': {}. Proceeding with config clear anyway.", collection_name, e);
            false 
        }
    };

    if collection_existed_before_clear {
        println!(
            "{}",
            format!(
                "Preparing to clear repository '{}' (Tenant: '{}'). This will DELETE the Qdrant collection '{}'.",
                repo_name_to_clear.cyan(),
                cli_tenant_id.cyan(),
                collection_name.cyan()
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

        println!("Deleting Qdrant collection '{}'...", collection_name.cyan());
        match delete_collection_by_name(client.clone(), &collection_name).await {
            Ok(_) => {
                log::info!("Successfully initiated deletion of collection '{}' for repository '{}'.", collection_name, repo_name_to_clear);
            }
            Err(e) => {
                log::error!("Failed to delete collection '{}' for repository '{}': {}", collection_name, repo_name_to_clear, e);
                eprintln!("{}", format!("Warning: Failed to delete Qdrant collection '{}'. Error: {}. Proceeding to clear local sync state.", collection_name, e).red());
            }
        }
    } else {
         log::info!("Qdrant Collection '{}' did not exist. Only clearing local sync state.", collection_name);
         println!("Qdrant Collection '{}' does not exist. Only clearing local sync state.", collection_name.cyan());
    }

    println!("Clearing repository sync status in configuration...");
    let repo_config_mut = &mut config.repositories[repo_config_index];
    repo_config_mut.last_synced_commits.clear();
    repo_config_mut.indexed_languages = None;

    // Clear git-manager state to ensure sync detection works correctly after clear
    let repo_path = PathBuf::from(&repo_config_mut.local_path);
    let mut git_manager = GitManager::new();
    git_manager.state_manager_mut().clear_repository_state(&repo_path);
    log::info!("Cleared git-manager state for repository at path: {}", repo_path.display());

    save_config(config, override_path)
        .context("Failed to save configuration after clearing repository state")?;
    
    if collection_existed_before_clear {
        println!("{}", format!("Qdrant collection for repository '{}' marked for deletion and local sync status reset.", repo_name_to_clear).green());
    } else {
        println!("{}", format!("Local sync status for repository '{}' reset (Qdrant collection was not present).", repo_name_to_clear).green());
    }

    Ok(())
} 