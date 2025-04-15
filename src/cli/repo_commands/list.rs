use anyhow::Result;
use colored::*;

use crate::config::AppConfig;

pub(crate) fn list_repositories(config: &AppConfig) -> Result<()> {
    if config.repositories.is_empty() {
        println!("No repositories configured yet. Use 'repo add <url>' to add one.");
        return Ok(());
    }

    println!("{}", "Managed Repositories:".bold());
    for repo in &config.repositories {
        let active_marker = if config.active_repository.as_ref() == Some(&repo.name) {
            "*".green().bold()
        } else {
            " ".normal()
        };
        println!(
            " {} {} ({}) -> {}",
            active_marker,
            repo.name.cyan().bold(),
            repo.url,
            repo.local_path.display()
        );
        println!("     Default Branch: {}", repo.default_branch);
        println!("     Tracked Branches: {:?}", repo.tracked_branches);
        // Display indexed languages if available
        if let Some(langs) = &repo.indexed_languages {
            if !langs.is_empty() {
                let mut sorted_langs = langs.clone();
                sorted_langs.sort();
                println!("     Indexed Languages: {}", sorted_langs.join(", "));
            }
        }
        // Optionally show last sync status here later
    }

    Ok(())
} 