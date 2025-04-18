use anyhow::Result;
use git2::{Repository, FetchOptions, RemoteCallbacks, AutotagOption};
use log;
use std::path::PathBuf;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::task;

use crate::config::{RepositoryConfig, AppConfig};
use crate::cli::repo_commands::helpers;
use crate::cli::commands::CliArgs;
use crate::cli::commands::FIELD_LANGUAGE;
use crate::vectordb::qdrant_client_trait::QdrantClientTrait;

pub struct SyncResult {
    pub success: bool,
    pub message: String,
    pub indexed_languages: Vec<String>,
}

/// Shared implementation of repository sync that can be used by both CLI and server
pub async fn sync_repository<C>(
    client: Arc<C>,
    repo_config: RepositoryConfig,
    options: SyncOptions,
    cli_args: &CliArgs,
    app_config: &AppConfig,
) -> Result<SyncResult>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    log::info!("Synchronizing repository: {}", repo_config.name);
    
    // Clone all values we'll need before moving into spawn_blocking
    let repo_name = repo_config.name.clone();
    let repo_path = repo_config.local_path.clone();
    let active_branch = match &repo_config.active_branch {
        Some(branch) => branch.clone(),
        None => "main".to_string(),
    };
    let remote_name = repo_config.remote_name.clone().unwrap_or_else(|| "origin".to_string());
    let last_synced_commits = repo_config.last_synced_commits.clone();
    let force_sync = options.force;
    let extensions_filter = options.extensions.clone();
    
    log::info!("Starting sync for repository '{}' on branch '{}'", repo_name, active_branch);
    
    // Make copies of values that will be needed after the closure
    let repo_name_outside = repo_name.clone();
    let repo_path_outside = repo_path.clone();
    let active_branch_outside = active_branch.clone();
    
    // Step 1: Perform ALL git operations in a blocking thread since git2 isn't Send
    let git_result = task::spawn_blocking(move || -> Result<(String, Vec<PathBuf>, bool)> {
        log::info!("Using branch '{}' for repository '{}'", active_branch, repo_name);
        
        // Open the repository
        let repo = Repository::open(&repo_path)?;
        
        // Set up the fetch options
        let mut fetch_opts = FetchOptions::new();
        let mut callbacks = RemoteCallbacks::new();
        callbacks.update_tips(|name, old, new| {
            log::info!("[{}] {} -> {}", name, old, new);
            true
        });
        fetch_opts.remote_callbacks(callbacks);
        fetch_opts.download_tags(AutotagOption::All);
        
        // Fetch updates from the remote
        let mut remote = repo.find_remote(&remote_name)?;
        remote.fetch(&[&active_branch], Some(&mut fetch_opts), None)?;
        
        // Find the branch reference
        let branch_ref_name = format!("refs/remotes/{}/{}", remote_name, active_branch);
        let branch_ref = repo.find_reference(&branch_ref_name)?;
        let branch_commit = branch_ref.peel_to_commit()?;
        let commit_oid = branch_commit.id();
        let commit_oid_str = commit_oid.to_string();
        
        // Check if we need a full sync
        let mut full_sync_needed = force_sync;
        if !force_sync {
            // Direct HashMap access
            if let Some(last_commit) = last_synced_commits.get(&active_branch) {
                if last_commit == &commit_oid_str {
                    log::info!("Repository already synced to commit: {}", commit_oid_str);
                    // Return early with empty file list if no sync needed
                    return Ok((commit_oid_str, Vec::new(), false));
                }
                log::info!("Repository needs syncing from {} to {}", last_commit, commit_oid_str);
            }
            full_sync_needed = true;
        }
        
        if full_sync_needed {
            log::info!("Performing full sync of repository");
            
            // Get the tree for the commit
            let tree = branch_commit.tree()?;
            
            // Collect all files from tree
            let mut files_to_index = Vec::new();
            tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
                if let Some(name) = entry.name() {
                    if let Some(kind) = entry.kind() {
                        if kind == git2::ObjectType::Blob {
                            let path = if dir.is_empty() {
                                PathBuf::from(name)
                            } else {
                                PathBuf::from(dir).join(name)
                            };
                            files_to_index.push(path);
                        }
                    }
                }
                git2::TreeWalkResult::Ok
            })?;
            
            Ok((commit_oid_str, files_to_index, true))
        } else {
            Ok((commit_oid_str, Vec::new(), false))
        }
    }).await??;  // Handle both the task JoinError and the Result
    
    let (commit_oid_str, mut files_to_index, sync_performed) = git_result;
    
    // Return early if no files to index (no changes)
    if !sync_performed || files_to_index.is_empty() {
        return Ok(SyncResult {
            success: true,
            message: format!("Repository '{}' already synced to the latest commit on branch '{}'", 
                repo_name_outside, active_branch_outside),
            indexed_languages: Vec::new(),
        });
    }
    
    // Filter files by extension if specified
    if let Some(exts) = &extensions_filter {
        log::info!("Filtering files by extensions: {:?}", exts);
        files_to_index = files_to_index.into_iter()
            .filter(|path| {
                if let Some(ext) = path.extension() {
                    if let Some(ext_str) = ext.to_str() {
                        return exts.iter().any(|e| e == ext_str);
                    }
                }
                false
            })
            .collect::<Vec<_>>();
    }
    
    log::info!("Found {} files to index", files_to_index.len());
    
    // Create collection name
    let collection_name = helpers::get_collection_name(&repo_name_outside);
    
    // Ensure the collection exists before indexing
    let default_dimension = helpers::DEFAULT_VECTOR_DIMENSION;
    helpers::ensure_repository_collection_exists(client.as_ref(), &collection_name, default_dimension).await?;
    
    // Perform indexing using the passed cli_args and app_config
    helpers::index_files(
        client.as_ref(),
        cli_args,
        app_config,
        &repo_path_outside,
        &files_to_index,
        &collection_name,
        &active_branch_outside,
        &commit_oid_str,
    ).await?;
    
    log::info!("Repository '{}' synced successfully to commit {}", repo_name_outside, commit_oid_str);
    
    // Query for indexed languages
    let mut indexed_languages = HashSet::new();
    let scroll_filter = helpers::create_branch_filter(&active_branch_outside);
    
    // Create scroll request manually using Qdrant client libraries
    use qdrant_client::qdrant::{ScrollPointsBuilder, PayloadIncludeSelector};
    let mut scroll_request = ScrollPointsBuilder::new(&collection_name)
        .filter(scroll_filter.clone())
        .limit(100)
        .with_payload(PayloadIncludeSelector { fields: vec![FIELD_LANGUAGE.to_string()] })
        .with_vectors(false);
    
    // Loop through all points to collect languages
    loop {
        let response = client.scroll(scroll_request.into()).await?;
        let points = response.result;
        
        if points.is_empty() {
            break;
        }
        
        // Collect languages from each point
        for point in &points {
            // payload is directly accessible, not wrapped in Option
            for (key, value) in &point.payload {
                if key == FIELD_LANGUAGE {
                    if let Some(kind) = &value.kind {
                        if let qdrant_client::qdrant::value::Kind::StringValue(lang) = kind {
                            indexed_languages.insert(lang.clone());
                        }
                    }
                }
            }
        }
        
        // Update the scroll request for the next batch
        if let Some(next_offset) = response.next_page_offset {
            let scroll_filter_clone = scroll_filter.clone();
            scroll_request = ScrollPointsBuilder::new(&collection_name)
                .filter(scroll_filter_clone)
                .limit(100)
                .with_payload(PayloadIncludeSelector { fields: vec![FIELD_LANGUAGE.to_string()] })
                .with_vectors(false)
                .offset(next_offset);
        } else {
            break;  // No more results
        }
    }
    
    // Sort languages for consistent output
    let mut indexed_languages: Vec<String> = indexed_languages.into_iter().collect();
    indexed_languages.sort();
    
    if !indexed_languages.is_empty() {
        log::info!("Indexed languages: {:?}", indexed_languages);
    }
    
    Ok(SyncResult {
        success: true,
        message: format!("Successfully synced repository '{}' on branch '{}'", 
            repo_name_outside, active_branch_outside),
        indexed_languages,
    })
}

/// Options for repository sync
#[derive(Debug)]
pub struct SyncOptions {
    pub force: bool,
    pub extensions: Option<Vec<String>>,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            force: false,
            extensions: None,
        }
    }
} 