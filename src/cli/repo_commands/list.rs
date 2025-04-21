use anyhow::Result;
use colored::*;
use serde::Serialize;

use crate::config::{AppConfig, RepositoryConfig};

// Define ManagedRepositories struct here if not already defined elsewhere
#[derive(Debug, Serialize)]
pub struct ManagedRepositories {
    pub repositories: Vec<RepositoryConfig>,
    pub active_repository: Option<String>,
}

// Rename function and change signature to accept &AppConfig
pub fn get_managed_repos_from_config(config: &AppConfig) -> ManagedRepositories {
    // Return a structure containing clones of the needed data
    ManagedRepositories {
        repositories: config.repositories.clone(),
        active_repository: config.active_repository.clone(),
    }
}

// Modify function signature to accept json flag
pub fn list_repositories(config: &AppConfig, json_output: bool) -> Result<()> {
    if config.repositories.is_empty() {
        if json_output {
            // Output empty JSON array if requested
            println!("[]");
        } else {
            println!("{}", "No repositories configured yet. Use 'repo add <url>' to add one.".yellow());
        }
        return Ok(());
    }

    if json_output {
        // Prepare data for JSON serialization
        let data = ManagedRepositories {
            repositories: config.repositories.clone(), // Clone data for serialization
            active_repository: config.active_repository.clone(),
        };
        let json_string = serde_json::to_string_pretty(&data)?;
        println!("{}", json_string);
    } else {
        // Original human-readable output
        println!("{}", "Managed Repositories:".bold().underline());
        for repo in &config.repositories {
            let active_marker = if config.active_repository.as_ref() == Some(&repo.name) {
                "* ".green()
            } else {
                "  ".into()
            };
            println!(
                "{} {} ({}) -> {}",
                active_marker,
                repo.name.cyan(),
                repo.url,
                repo.local_path.display()
            );
            // Add more details if needed
            println!("     Default Branch: {}", repo.default_branch);
            if let Some(active_branch) = &repo.active_branch {
                 println!("     Active Branch: {}", active_branch);
             }
            println!("     Tracked Branches: {:?}", repo.tracked_branches);
             if let Some(langs) = &repo.indexed_languages {
                 if !langs.is_empty() {
                     println!("     Indexed Languages: {}", langs.join(", ").blue());
                 }
             }
        }
    }

    Ok(())
} 