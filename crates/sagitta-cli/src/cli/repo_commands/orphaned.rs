use anyhow::Result;
use clap::{Args, Subcommand};
use colored::*;
use sagitta_search::{
    config::{AppConfig, save_config},
    scan_for_orphaned_repositories, reclone_missing_repository,
    add_orphaned_repository, remove_orphaned_repository,
    get_enhanced_repository_list,
};
use std::path::PathBuf;
use std::io::{self, Write};

#[derive(Args, Debug, Clone)]
pub struct OrphanedArgs {
    #[command(subcommand)]
    pub command: OrphanedCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum OrphanedCommand {
    /// List orphaned repositories (on filesystem but not in config) and missing repos (in config but not on filesystem)
    List(ListOrphanedArgs),
    /// Reclone missing repositories
    Reclone(RecloneArgs),
    /// Add orphaned repository to configuration
    Add(AddOrphanedArgs),
    /// Remove orphaned repository from filesystem
    Remove(RemoveOrphanedArgs),
    /// Clean all orphaned repositories
    Clean(CleanOrphanedArgs),
}

#[derive(Args, Debug, Clone)]
pub struct ListOrphanedArgs {
    /// Output in JSON format
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug, Clone)]
pub struct RecloneArgs {
    /// Repository name to reclone (if not specified, reclones all missing)
    pub name: Option<String>,
    /// Skip confirmation prompt
    #[arg(short, long)]
    pub yes: bool,
}

#[derive(Args, Debug, Clone)]
pub struct AddOrphanedArgs {
    /// Name of the orphaned repository directory to add
    pub name: String,
    /// Skip confirmation prompt
    #[arg(short, long)]
    pub yes: bool,
}

#[derive(Args, Debug, Clone)]
pub struct RemoveOrphanedArgs {
    /// Name of the orphaned repository directory to remove
    pub name: String,
    /// Skip confirmation prompt
    #[arg(short, long)]
    pub yes: bool,
}

#[derive(Args, Debug, Clone)]
pub struct CleanOrphanedArgs {
    /// Skip confirmation prompt
    #[arg(short, long)]
    pub yes: bool,
}

pub async fn handle_orphaned_command(
    args: OrphanedArgs,
    config: &mut AppConfig,
    override_path: Option<&PathBuf>,
) -> Result<()> {
    match args.command {
        OrphanedCommand::List(list_args) => {
            handle_list_orphaned(list_args, config).await
        }
        OrphanedCommand::Reclone(reclone_args) => {
            handle_reclone(reclone_args, config, override_path).await
        }
        OrphanedCommand::Add(add_args) => {
            handle_add_orphaned(add_args, config, override_path).await
        }
        OrphanedCommand::Remove(remove_args) => {
            handle_remove_orphaned(remove_args, config).await
        }
        OrphanedCommand::Clean(clean_args) => {
            handle_clean_orphaned(clean_args, config).await
        }
    }
}

async fn handle_list_orphaned(args: ListOrphanedArgs, config: &AppConfig) -> Result<()> {
    // Get enhanced repository list which includes missing repos
    let enhanced_list = get_enhanced_repository_list(config).await?;
    
    // Get orphaned repositories
    let orphaned_repos = scan_for_orphaned_repositories(config).await?;
    
    if args.json {
        let output = serde_json::json!({
            "orphaned_repositories": orphaned_repos,
            "missing_repositories": enhanced_list.repositories
                .iter()
                .filter(|r| !r.filesystem_status.exists)
                .map(|r| serde_json::json!({
                    "name": r.name,
                    "url": r.url,
                    "local_path": r.local_path,
                    "added_as_local_path": r.added_as_local_path,
                }))
                .collect::<Vec<_>>(),
            "summary": {
                "orphaned_count": orphaned_repos.len(),
                "missing_count": enhanced_list.summary.missing_count,
            }
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        
        // Display missing repositories
        let missing_repos: Vec<_> = enhanced_list.repositories
            .iter()
            .filter(|r| !r.filesystem_status.exists)
            .collect();
        
        if !missing_repos.is_empty() {
            println!("\n{}", "Missing Repositories (in config but not on filesystem):".bold());
            println!("{}", "─".repeat(60).dimmed());
            
            for repo in &missing_repos {
                println!("  {} {}", 
                    "✗".red(),
                    repo.name.red()
                );
                println!("    {} {}", "URL:".dimmed(), repo.url);
                println!("    {} {}", "Path:".dimmed(), repo.local_path.display());
                if repo.added_as_local_path {
                    println!("    {} Added as local path (cannot be recloned)", "Note:".yellow());
                }
            }
        }
        
        // Display orphaned repositories
        if !orphaned_repos.is_empty() {
            println!("\n{}", "Orphaned Repositories (on filesystem but not in config):".bold());
            println!("{}", "─".repeat(60).dimmed());
            
            for orphan in &orphaned_repos {
                println!("  {} {}", 
                    "?".yellow(),
                    orphan.name.yellow()
                );
                println!("    {} {}", "Path:".dimmed(), orphan.local_path.display());
                if orphan.is_git_repository {
                    println!("    {} Git repository", "✓".green());
                    if let Some(url) = &orphan.remote_url {
                        println!("    {} {}", "Remote:".dimmed(), url);
                    }
                }
                if let Some(file_count) = orphan.file_count {
                    println!("    {} {} files", "Files:".dimmed(), file_count);
                }
                if let Some(size) = orphan.size_bytes {
                    let size_mb = size as f64 / 1_048_576.0;
                    println!("    {} {:.2} MB", "Size:".dimmed(), size_mb);
                }
            }
        }
        
        // Summary
        println!("\n{}", "Summary:".bold());
        println!("  {} missing repositories", enhanced_list.summary.missing_count);
        println!("  {} orphaned repositories", orphaned_repos.len());
        
        if !missing_repos.is_empty() && !missing_repos.iter().all(|r| r.added_as_local_path) {
            println!("\n{}", "Run 'sagitta-cli repo orphaned reclone' to reclone missing repositories".dimmed());
        }
        if !orphaned_repos.is_empty() {
            println!("{}", "Run 'sagitta-cli repo orphaned add <name>' to add an orphaned repository".dimmed());
            println!("{}", "Run 'sagitta-cli repo orphaned remove <name>' to delete an orphaned repository".dimmed());
        }
    }
    
    Ok(())
}

async fn handle_reclone(args: RecloneArgs, config: &AppConfig, override_path: Option<&PathBuf>) -> Result<()> {
    let enhanced_list = get_enhanced_repository_list(config).await?;
    
    let missing_repos: Vec<_> = enhanced_list.repositories
        .iter()
        .filter(|r| !r.filesystem_status.exists && !r.added_as_local_path)
        .collect();
    
    if missing_repos.is_empty() {
        println!("No missing repositories to reclone.");
        return Ok(());
    }
    
    let repos_to_reclone = if let Some(name) = &args.name {
        // Reclone specific repository
        missing_repos.into_iter()
            .filter(|r| r.name == *name)
            .collect::<Vec<_>>()
    } else {
        // Reclone all missing
        missing_repos
    };
    
    if repos_to_reclone.is_empty() {
        if args.name.is_some() {
            return Err(anyhow::anyhow!("Repository '{}' not found or not missing", args.name.unwrap()));
        }
        println!("No repositories to reclone.");
        return Ok(());
    }
    
    // Confirm action
    if !args.yes {
        println!("\nRepositories to reclone:");
        for repo in &repos_to_reclone {
            println!("  - {} ({})", repo.name, repo.url);
        }
        
        print!("Reclone {} repositories? (yes/No): ", repos_to_reclone.len());
        io::stdout().flush()?;
        let mut confirmation = String::new();
        io::stdin().read_line(&mut confirmation)?;
        
        if confirmation.trim().to_lowercase() != "yes" {
            println!("Operation cancelled.");
            return Ok(());
        }
    }
    
    // Reclone repositories
    let mut success_count = 0;
    let mut failures = Vec::new();
    
    for repo in repos_to_reclone {
        println!("\nRecloning {} from {}...", repo.name, repo.url);
        
        match reclone_missing_repository(config, &repo.name).await {
            Ok(_) => {
                println!("{} Successfully recloned {}", "✓".green(), repo.name);
                success_count += 1;
            }
            Err(e) => {
                println!("{} Failed to reclone {}: {}", "✗".red(), repo.name, e);
                failures.push((repo.name.clone(), e.to_string()));
            }
        }
    }
    
    // Summary
    println!("\n{}", "Reclone Summary:".bold());
    println!("  {success_count} repositories successfully recloned");
    if !failures.is_empty() {
        println!("  {} repositories failed:", failures.len());
        for (name, error) in failures {
            println!("    - {name}: {error}");
        }
    }
    
    Ok(())
}

async fn handle_add_orphaned(args: AddOrphanedArgs, config: &mut AppConfig, override_path: Option<&PathBuf>) -> Result<()> {
    // Find the orphaned repository
    let orphaned_repos = scan_for_orphaned_repositories(config).await?;
    
    let orphan = orphaned_repos
        .iter()
        .find(|o| o.name == args.name)
        .ok_or_else(|| anyhow::anyhow!("Orphaned repository '{}' not found", args.name))?;
    
    // Confirm action
    if !args.yes {
        println!("\nAdding orphaned repository:");
        println!("  Name: {}", orphan.name);
        println!("  Path: {}", orphan.local_path.display());
        if let Some(url) = &orphan.remote_url {
            println!("  Remote URL: {url}");
        }
        
        print!("Add this repository to configuration? (yes/No): ");
        io::stdout().flush()?;
        let mut confirmation = String::new();
        io::stdin().read_line(&mut confirmation)?;
        
        if confirmation.trim().to_lowercase() != "yes" {
            println!("Operation cancelled.");
            return Ok(());
        }
    }
    
    // Add the repository
    add_orphaned_repository(config, orphan).await?;
    
    // Save configuration
    save_config(config, override_path)?;
    
    println!("{} Successfully added repository '{}'", "✓".green(), orphan.name);
    println!("Run 'sagitta-cli repo sync {}' to index the repository", orphan.name);
    
    Ok(())
}

async fn handle_remove_orphaned(args: RemoveOrphanedArgs, config: &AppConfig) -> Result<()> {
    // Find the orphaned repository
    let orphaned_repos = scan_for_orphaned_repositories(config).await?;
    
    let orphan = orphaned_repos
        .iter()
        .find(|o| o.name == args.name)
        .ok_or_else(|| anyhow::anyhow!("Orphaned repository '{}' not found", args.name))?;
    
    // Confirm action
    if !args.yes {
        println!("\n{}", "WARNING: This will permanently delete the repository from the filesystem!".red().bold());
        println!("\nRemoving orphaned repository:");
        println!("  Name: {}", orphan.name);
        println!("  Path: {}", orphan.local_path.display());
        if let Some(file_count) = orphan.file_count {
            println!("  Files: {file_count}");
        }
        
        print!("Are you sure you want to delete this directory? (yes/No): ");
        io::stdout().flush()?;
        let mut confirmation = String::new();
        io::stdin().read_line(&mut confirmation)?;
        
        if confirmation.trim().to_lowercase() != "yes" {
            println!("Operation cancelled.");
            return Ok(());
        }
    }
    
    // Remove the directory
    remove_orphaned_repository(orphan).await?;
    
    println!("{} Successfully removed orphaned repository '{}'", "✓".green(), orphan.name);
    
    Ok(())
}

async fn handle_clean_orphaned(args: CleanOrphanedArgs, config: &AppConfig) -> Result<()> {
    let orphaned_repos = scan_for_orphaned_repositories(config).await?;
    
    if orphaned_repos.is_empty() {
        println!("No orphaned repositories found.");
        return Ok(());
    }
    
    // Confirm action
    if !args.yes {
        println!("\n{}", "WARNING: This will permanently delete ALL orphaned repositories from the filesystem!".red().bold());
        println!("\nOrphaned repositories to remove:");
        for orphan in &orphaned_repos {
            println!("  - {} ({})", orphan.name, orphan.local_path.display());
        }
        
        print!("Delete {} orphaned repositories? (yes/No): ", orphaned_repos.len());
        io::stdout().flush()?;
        let mut confirmation = String::new();
        io::stdin().read_line(&mut confirmation)?;
        
        if confirmation.trim().to_lowercase() != "yes" {
            println!("Operation cancelled.");
            return Ok(());
        }
    }
    
    // Remove all orphaned repositories
    let mut success_count = 0;
    let mut failures = Vec::new();
    
    for orphan in orphaned_repos {
        match remove_orphaned_repository(&orphan).await {
            Ok(_) => {
                println!("{} Removed {}", "✓".green(), orphan.name);
                success_count += 1;
            }
            Err(e) => {
                println!("{} Failed to remove {}: {}", "✗".red(), orphan.name, e);
                failures.push((orphan.name, e.to_string()));
            }
        }
    }
    
    // Summary
    println!("\n{}", "Clean Summary:".bold());
    println!("  {success_count} orphaned repositories removed");
    if !failures.is_empty() {
        println!("  {} repositories failed:", failures.len());
        for (name, error) in failures {
            println!("    - {name}: {error}");
        }
    }
    
    Ok(())
}