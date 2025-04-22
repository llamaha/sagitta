use anyhow::Result;
use anyhow::Context;
use colored::*;

// Use config types and the list helper from vectordb_core
use vectordb_core::{AppConfig, get_managed_repos_from_config};

// Modify function signature to accept json flag
pub fn list_repositories(config: &AppConfig, json_output: bool) -> Result<()> {
    // Use the function from vectordb_core
    let data = get_managed_repos_from_config(config);

    if json_output {
        // Serialize the entire ManagedRepositories struct
        let json_output = serde_json::to_string_pretty(&data)
            .context("Failed to serialize repository list to JSON")?;
        println!("{}", json_output);
    } else {
        // Original pretty print logic (uses data.repositories and data.active_repository)
        if data.repositories.is_empty() {
            println!("No repositories managed yet. Use `vectordb repo add` to add one.");
            return Ok(());
        }

        println!("{}", "Managed Repositories:".bold().underline());
        for repo in data.repositories {
            let repo_name = repo.name.as_str();
            let active_marker = if data.active_repository.as_deref() == Some(repo_name) {
                "*" 
            } else {
                " "
            };
            let local_path = repo.local_path.display();
            println!(" {} {} -> {}", active_marker.green(), repo_name.cyan(), local_path);
        }
        
        if let Some(active) = data.active_repository {
            println!("\n{}: {}", "Active Repository".bold(), active.green());
        } else {
            println!("\nNo active repository set. Use `vectordb repo use <name>` to set one.");
        }
    }

    Ok(())
} 