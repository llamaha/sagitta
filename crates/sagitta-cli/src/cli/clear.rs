use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use colored::*;
use qdrant_client::Qdrant;
use std::io::{self, Write}; // Import io for confirmation prompt
use std::sync::Arc;
use sagitta_search::AppConfig; // Added RepositoryConfig
use sagitta_search::repo_helpers::get_branch_aware_collection_name; // Use core helper

#[derive(Args, Debug)]
pub struct ClearArgs {
    /// Optional: Specify the repository name to clear. 
    /// If omitted, clears the active repository.
    #[arg(long)]
    repo_name: Option<String>,

    /// Confirm deletion without prompting.
    #[arg(short, long)]
    yes: bool,
}

pub async fn handle_clear(
    args: &ClearArgs, // Changed to reference
    config: AppConfig, // Keep ownership
    client: Arc<Qdrant>, // Accept client
    _cli_args: &crate::cli::CliArgs, // Added cli_args
) -> Result<()> {

    let repo_name_to_clear = match args.repo_name.as_ref().or(config.active_repository.as_ref()) {
        Some(name) => name.clone(),
        None => bail!("No active repository set and no repository name provided."),
    };

    let repo_config_index = config
        .repositories
        .iter()
        .position(|r| r.name == repo_name_to_clear)
        .ok_or_else(|| anyhow!("Configuration for repository '{}' not found.", repo_name_to_clear))?;

    let repo_config = &config.repositories[repo_config_index];
    let branch_name = repo_config.target_ref.as_deref()
        .or(repo_config.active_branch.as_deref())
        .unwrap_or(&repo_config.default_branch);

    // Use branch-aware collection naming to match the new sync behavior
    let collection_name = get_branch_aware_collection_name(&repo_name_to_clear, branch_name, &config);

    // --- Check Qdrant Collection Status (Informational) ---
    log::info!("Preparing to clear data for repository: '{repo_name_to_clear}', collection: '{collection_name}'");

    // --- Confirmation --- 
    if !args.yes {
        let prompt_message = format!(
            "Are you sure you want to delete ALL indexed data for repository '{}' (collection '{}')?",
            repo_name_to_clear.yellow().bold(),
            collection_name.yellow().bold()
        );
        print!("{prompt_message} (yes/No): ");
        io::stdout().flush().context("Failed to flush stdout")?;
        let mut confirmation = String::new();
        io::stdin().read_line(&mut confirmation)
            .context("Failed to read confirmation input")?;
        if confirmation.trim().to_lowercase() != "yes" {
            println!("Operation cancelled.");
            return Ok(());
        }
    }

    // --- Delete Collection --- 
    // Deleting the collection is simpler than deleting all points for repos
    log::info!("Attempting to delete collection '{collection_name}'...");
    println!("Deleting collection '{collection_name}'...");

    match client.delete_collection(collection_name.clone()).await {
        Ok(op_result) => {
            if op_result.result {
                println!(
                    "{}",
                    format!("Successfully deleted collection '{collection_name}'.").green()
                );
                 log::info!("Collection '{collection_name}' deleted successfully.");
            } else {
                 println!(
                     "{}",
                     format!("Collection '{collection_name}' might not have existed or deletion failed server-side.").yellow()
                 );
                 log::warn!("Delete operation for collection '{collection_name}' returned false.");
            }
        }
        Err(e) => {
             // Check if it's a "not found" type error - treat as success in clearing
             if e.to_string().contains("Not found") || e.to_string().contains("doesn\'t exist") {
                 println!(
                     "{}",
                     format!("Collection '{collection_name}' did not exist.").yellow()
                 );
                 log::warn!("Collection '{collection_name}' not found during delete attempt.");
             } else {
                 // For other errors, report them
                 eprintln!(
                     "{}",
                     format!("Failed to delete collection '{collection_name}': {e}").red()
                 );
                 return Err(e).context(format!("Failed to delete collection '{collection_name}'"));
             }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sagitta_search::RepositoryConfig;
    use std::path::PathBuf;
    use std::collections::HashMap;

    #[test]
    fn test_clear_args_debug() {
        let args = ClearArgs {
            repo_name: Some("test".to_string()),
            yes: true,
        };
        
        // Test that Debug trait is implemented
        let debug_str = format!("{:?}", args);
        assert!(debug_str.contains("ClearArgs"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_branch_resolution() {
        let config = RepositoryConfig {
            name: "test".to_string(),
            url: "https://github.com/test/repo.git".to_string(),
            local_path: PathBuf::from("/test"),
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string()],
            remote_name: Some("origin".to_string()),
            last_synced_commits: HashMap::new(),
            active_branch: Some("feature".to_string()),
            ssh_key_path: None,
            ssh_key_passphrase: None,
            indexed_languages: None,
            added_as_local_path: false,
            target_ref: Some("v1.0".to_string()),
            dependencies: vec![],
            last_synced_commit: None,
        };
        
        let branch = config.target_ref.as_deref()
            .or(config.active_branch.as_deref())
            .unwrap_or(&config.default_branch);
        
        assert_eq!(branch, "v1.0");
    }

    #[test]
    fn test_collection_name_generation() {
        let collection_name = get_branch_aware_collection_name("test-repo", "main", &AppConfig::default());
        assert!(collection_name.contains("test-repo") || collection_name.contains("test_repo"));
    }
}

 
