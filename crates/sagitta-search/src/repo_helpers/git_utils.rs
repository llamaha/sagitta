// Git-related helper functions from repo_helpers.rs will be moved here. 

use std::path::PathBuf;
use git2::{Repository, FetchOptions, Cred, RemoteCallbacks, CredentialType};
use anyhow::Result;
use crate::config::RepositoryConfig;

/// Helper function to check if a file extension is explicitly supported by a parser
pub fn is_supported_extension(extension: &str) -> bool {
    matches!(extension.to_lowercase().as_str(), 
        "rs" | "rb" | "go" | "js" | "jsx" | "ts" | "tsx" | "yaml" | "yml" | "md" | "mdx" | "py"
    )
}

/// Helper to create FetchOptions with SSH credential callback
pub fn create_fetch_options<'a>(
    repo_configs: Vec<RepositoryConfig>,
    repo_url: &'a str,
    ssh_key_path: Option<&'a PathBuf>,
    ssh_key_passphrase: Option<&'a str>,
) -> Result<FetchOptions<'a>> {
    let mut callbacks = RemoteCallbacks::new();
    let relevant_repo_config = repo_configs.iter()
        .find(|r| r.url == repo_url)
        .cloned();
        
    // Check if running in server mode (no interactive prompts allowed)
    let is_server_mode = false;
    
    // Is this an SSH URL? (starts with git@ or ssh://)
    let is_ssh_url = repo_url.starts_with("git@") || repo_url.starts_with("ssh://");
    
    callbacks.credentials(move |_url, username_from_git, allowed_types| {
        log::debug!("Credential callback triggered. URL: {}, Username: {:?}, Allowed: {:?}", _url, username_from_git, allowed_types);
        
        // In server mode, immediately fail for SSH URLs without explicit credentials
        if is_server_mode && is_ssh_url && ssh_key_path.is_none() && 
           !relevant_repo_config.as_ref().and_then(|r| r.ssh_key_path.as_ref()).is_some() {
            log::error!("Server mode detected with SSH URL '{}' but no SSH key configured. Use HTTPS URLs or configure SSH keys explicitly.", _url);
            return Err(git2::Error::from_str("Server mode cannot use interactive authentication. Use HTTPS URLs or configure SSH keys explicitly."));
        }
        
        // First check direct SSH key parameters (for new repositories)
        if allowed_types.contains(CredentialType::SSH_KEY) && ssh_key_path.is_some() {
            let user = username_from_git.unwrap_or("git");
            let key_path = ssh_key_path.unwrap();
            log::debug!("Attempting SSH key authentication from direct parameters. User: '{}', Key Path: {}", user, key_path.display());
            match Cred::ssh_key(user, None, key_path, ssh_key_passphrase) {
                Ok(cred) => {
                    log::info!("SSH key credential created successfully from direct parameters for user '{}'.", user);
                    return Ok(cred);
                }
                Err(e) => {
                    log::error!("Failed to create SSH key credential from direct parameter path {}: {}", key_path.display(), e);
                }
            }
        }
        
        // Then check repository config (for existing repositories)
        if let Some(repo_config) = &relevant_repo_config {
            if allowed_types.contains(CredentialType::SSH_KEY) {
                if let Some(key_path) = &repo_config.ssh_key_path {
                    let user = username_from_git.unwrap_or("git");
                    log::debug!("Attempting SSH key authentication from repo config. User: '{}', Key Path: {}", user, key_path.display());
                    match Cred::ssh_key(user, None, key_path, repo_config.ssh_key_passphrase.as_deref()) {
                        Ok(cred) => {
                            log::info!("SSH key credential created successfully from repo config for user '{}'.", user);
                            return Ok(cred);
                        }
                        Err(e) => {
                            log::error!("Failed to create SSH key credential from repo config path {}: {}", key_path.display(), e);
                        }
                    }
                } else {
                    log::debug!("SSH key requested, but no ssh_key_path configured for repo '{}'", repo_config.name);
                }
            }
        } else {
            log::debug!("No repository configuration found for URL '{}' in credential callback.", _url);
        }
        
        // In server mode, don't try to use default credentials which might prompt for a password
        if is_server_mode && is_ssh_url {
            log::error!("No configured SSH credentials found for URL '{}' in server mode. Unable to authenticate.", _url);
            return Err(git2::Error::from_str("Server mode cannot use interactive authentication. Configure SSH keys explicitly."));
        }
        
        // Finally try default
        if allowed_types.contains(CredentialType::DEFAULT) {
            log::debug!("Attempting default system credentials.");
            match Cred::default() {
                Ok(cred) => {
                    log::info!("Using default system credentials.");
                    return Ok(cred);
                }
                Err(e) => {
                    log::warn!("Failed to get default system credentials: {}", e);
                }
            }
        }
        log::error!("No suitable credentials found or configured for URL '{}', user '{:?}'", _url, username_from_git);
        Err(git2::Error::from_str("Authentication failed: no suitable credentials found"))
    });
    let mut fetch_opts = FetchOptions::new();
    fetch_opts.remote_callbacks(callbacks);
    Ok(fetch_opts)
}

/// Recursively collect files from a Git tree
pub fn collect_files_from_tree(
    repo: &Repository,
    tree: &git2::Tree,
    file_list: &mut Vec<PathBuf>,
    current_path: &PathBuf,
) -> Result<()> {
    for entry in tree.iter() {
        let entry_path = current_path.join(entry.name().unwrap_or(""));
        match entry.kind() {
            Some(git2::ObjectType::Blob) => {
                if entry_path.extension().map_or(false, |ext| is_supported_extension(ext.to_str().unwrap_or(""))) {
                     file_list.push(entry_path);
                 } else {
                    log::trace!("Skipping non-supported file: {}", entry_path.display());
                 }
            }
            Some(git2::ObjectType::Tree) => {
                let subtree = repo.find_tree(entry.id())?;
                collect_files_from_tree(repo, &subtree, file_list, &entry_path)?;
            }
            _ => {}
        }
    }
    Ok(())
} 