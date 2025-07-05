use anyhow::Result;
use anyhow::Context;
use colored::*;
use clap::Args;

// Use config types and the enhanced list helper from sagitta_search
use sagitta_search::{AppConfig, get_enhanced_repository_list, EnhancedRepositoryList, SyncState};

// Define ListArgs struct
#[derive(Args, Debug, Clone)]
pub struct ListArgs {
    /// Output the list of repositories in JSON format.
    #[arg(long)]
    pub json: bool,
    /// Show detailed information including file extensions and sync status
    #[arg(long)]
    pub detailed: bool,
    /// Show summary statistics only
    #[arg(long)]
    pub summary: bool,
}

// Updated function to use enhanced repository listing
pub async fn list_repositories(config: &AppConfig, args: ListArgs) -> Result<()> {
    let enhanced_data = get_enhanced_repository_list(config).await
        .context("Failed to get enhanced repository list")?;

    if args.json {
        // Serialize the entire enhanced repository list
        let json_output = serde_json::to_string_pretty(&enhanced_data)
            .context("Failed to serialize enhanced repository list to JSON")?;
        println!("{json_output}");
    } else if args.summary {
        print_summary_only(&enhanced_data);
    } else {
        print_enhanced_repository_list(&enhanced_data, args.detailed);
    }

    Ok(())
}

fn print_enhanced_repository_list(data: &EnhancedRepositoryList, detailed: bool) {
        if data.repositories.is_empty() {
            println!("No repositories managed yet. Use `sagitta repo add` to add one.");
        return;
        }

    println!("{}", "Enhanced Repository List:".bold().underline());
    println!();

    for repo in &data.repositories {
        let repo_name = &repo.name;
        let active_marker = if data.active_repository.as_deref() == Some(repo_name) {
            "*".green()
        } else {
            " ".normal()
        };

        // Repository name and basic info
        println!("{} {} -> {}", 
            active_marker, 
            repo_name.cyan().bold(), 
            repo.local_path.display()
        );

        // Filesystem status
        let fs_status = if repo.filesystem_status.exists {
            if repo.filesystem_status.is_git_repository {
                "exists (git)".green()
            } else {
                "exists (no git)".yellow()
            }
        } else {
            "missing".red()
        };
        
        println!("   📁 Status: {fs_status}");

        // Current branch and sync status
        if let Some(branch) = &repo.active_branch {
            let sync_status_color = match repo.sync_status.state {
                SyncState::UpToDate => "up-to-date".green(),
                SyncState::NeedsSync => "needs sync".yellow(),
                SyncState::NeverSynced => "never synced".red(),
                SyncState::Unknown => "unknown".normal(),
            };
            
            println!("   🌿 Branch: {} ({})", branch.bright_blue(), sync_status_color);
        }

        // Git status if available
        if let Some(git_status) = &repo.git_status {
            let clean_status = if git_status.is_clean {
                "clean".green()
            } else {
                "dirty".yellow()
            };
            
            if git_status.is_detached_head {
                println!("   📍 Commit: {} (detached HEAD, {})", 
                    &git_status.current_commit[..8], clean_status);
            } else {
                println!("   📍 Commit: {} ({})", 
                    &git_status.current_commit[..8], clean_status);
            }
        }

        // File statistics
        if let Some(file_count) = repo.filesystem_status.total_files {
            let size_str = if let Some(size) = repo.filesystem_status.size_bytes {
                format!(" ({})", format_bytes(size))
            } else {
                String::new()
            };
            println!("   📊 Files: {file_count}{size_str}");
        }

        // Languages
        if let Some(languages) = &repo.indexed_languages {
            println!("   🔤 Languages: {}", languages.join(", ").bright_cyan());
        }

        if detailed {
            // File extensions (top 5)
            if !repo.file_extensions.is_empty() {
                let top_extensions: Vec<_> = repo.file_extensions.iter().take(5).collect();
                let ext_strs: Vec<String> = top_extensions.iter()
                    .map(|ext| format!("{} ({})", ext.extension, ext.count))
                    .collect();
                println!("   📄 Extensions: {}", ext_strs.join(", "));
            }

            // Tracked branches
            if repo.tracked_branches.len() > 1 {
                println!("   🌿 Tracked: {}", repo.tracked_branches.join(", "));
            }

            // Branches needing sync
            if !repo.sync_status.branches_needing_sync.is_empty() {
                println!("   ⚠️  Need sync: {}", 
                    repo.sync_status.branches_needing_sync.join(", ").yellow()
                );
        }
        }

        println!();
    }

    // Show active repository
    if let Some(active) = &data.active_repository {
        println!("{}: {}", "Active Repository".bold(), active.green());
        } else {
        println!("No active repository set. Use `sagitta repo use <name>` to set one.");
        }

    // Show summary statistics
    println!();
    print_summary_statistics(&data.summary);
}

fn print_summary_only(data: &EnhancedRepositoryList) {
    println!("{}", "Repository Summary:".bold().underline());
    println!();
    print_summary_statistics(&data.summary);
    
    if let Some(active) = &data.active_repository {
        println!();
        println!("{}: {}", "Active Repository".bold(), active.green());
    }
}

fn print_summary_statistics(summary: &sagitta_search::RepositoryListSummary) {
    println!("{}", "Summary Statistics:".bold());
    println!("   📁 Total repositories: {}", summary.existing_count + if summary.existing_count == 0 { 0 } else { 0 });
    println!("   ✅ Existing on filesystem: {}", summary.existing_count);
    println!("   🔄 Need syncing: {}", summary.needs_sync_count);
    println!("   ⚠️  With uncommitted changes: {}", summary.dirty_count);
    println!("   📊 Total files: {}", summary.total_files);
    println!("   💾 Total size: {}", format_bytes(summary.total_size_bytes));
    
    if !summary.common_extensions.is_empty() {
        println!("   📄 Common extensions:");
        for (i, ext) in summary.common_extensions.iter().take(5).enumerate() {
            println!("      {}. {} ({} files, {})", 
                i + 1, 
                ext.extension.bright_cyan(), 
                ext.count, 
                format_bytes(ext.size_bytes)
            );
        }
    }
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
} 