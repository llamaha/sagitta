// Git-related helper functions from repo_helpers.rs will be moved here. 

use std::path::{Path, PathBuf};
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
        log::debug!("Credential callback triggered. URL: {_url}, Username: {username_from_git:?}, Allowed: {allowed_types:?}");
        
        // In server mode, immediately fail for SSH URLs without explicit credentials
        if is_server_mode && is_ssh_url && ssh_key_path.is_none() && 
           relevant_repo_config.as_ref().and_then(|r| r.ssh_key_path.as_ref()).is_none() {
            log::error!("Server mode detected with SSH URL '{_url}' but no SSH key configured. Use HTTPS URLs or configure SSH keys explicitly.");
            return Err(git2::Error::from_str("Server mode cannot use interactive authentication. Use HTTPS URLs or configure SSH keys explicitly."));
        }
        
        // First check direct SSH key parameters (for new repositories)
        if allowed_types.contains(CredentialType::SSH_KEY) && ssh_key_path.is_some() {
            let user = username_from_git.unwrap_or("git");
            let key_path = ssh_key_path.unwrap();
            log::debug!("Attempting SSH key authentication from direct parameters. User: '{}', Key Path: {}", user, key_path.display());
            match Cred::ssh_key(user, None, key_path, ssh_key_passphrase) {
                Ok(cred) => {
                    log::info!("SSH key credential created successfully from direct parameters for user '{user}'.");
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
                            log::info!("SSH key credential created successfully from repo config for user '{user}'.");
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
            log::debug!("No repository configuration found for URL '{_url}' in credential callback.");
        }
        
        // In server mode, don't try to use default credentials which might prompt for a password
        if is_server_mode && is_ssh_url {
            log::error!("No configured SSH credentials found for URL '{_url}' in server mode. Unable to authenticate.");
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
                    log::warn!("Failed to get default system credentials: {e}");
                }
            }
        }
        log::error!("No suitable credentials found or configured for URL '{_url}', user '{username_from_git:?}'");
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
    current_path: &Path,
) -> Result<()> {
    for entry in tree.iter() {
        let entry_path = current_path.join(entry.name().unwrap_or(""));
        match entry.kind() {
            Some(git2::ObjectType::Blob) => {
                if entry_path.extension().is_some_and(|ext| is_supported_extension(ext.to_str().unwrap_or(""))) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_is_supported_extension() {
        // Test supported extensions
        assert!(is_supported_extension("rs"));
        assert!(is_supported_extension("rb"));
        assert!(is_supported_extension("go"));
        assert!(is_supported_extension("js"));
        assert!(is_supported_extension("jsx"));
        assert!(is_supported_extension("ts"));
        assert!(is_supported_extension("tsx"));
        assert!(is_supported_extension("yaml"));
        assert!(is_supported_extension("yml"));
        assert!(is_supported_extension("md"));
        assert!(is_supported_extension("mdx"));
        assert!(is_supported_extension("py"));
        
        // Test uppercase
        assert!(is_supported_extension("RS"));
        assert!(is_supported_extension("PY"));
        assert!(is_supported_extension("YAML"));
        
        // Test unsupported extensions
        assert!(!is_supported_extension("txt"));
        assert!(!is_supported_extension("pdf"));
        assert!(!is_supported_extension("exe"));
        assert!(!is_supported_extension(""));
        assert!(!is_supported_extension("unknown"));
    }

    #[test]
    fn test_create_fetch_options_no_ssh() {
        let repo_configs = vec![];
        let repo_url = "https://github.com/example/repo.git";
        
        let result = create_fetch_options(repo_configs, repo_url, None, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_fetch_options_with_ssh_key() {
        let temp_dir = TempDir::new().unwrap();
        let ssh_key_path = temp_dir.path().join("test_key");
        fs::write(&ssh_key_path, "dummy key content").unwrap();
        
        let repo_configs = vec![];
        let repo_url = "git@github.com:example/repo.git";
        
        let result = create_fetch_options(
            repo_configs, 
            repo_url, 
            Some(&ssh_key_path), 
            Some("passphrase")
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_fetch_options_with_repo_config() {
        let temp_dir = TempDir::new().unwrap();
        let ssh_key_path = temp_dir.path().join("repo_key");
        fs::write(&ssh_key_path, "dummy key content").unwrap();
        
        let repo_config = RepositoryConfig {
            name: "test_repo".to_string(),
            url: "git@github.com:example/repo.git".to_string(),
            local_path: PathBuf::from("/tmp/test"),
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string()],
            remote_name: Some("origin".to_string()),
            last_synced_commits: std::collections::HashMap::new(),
            active_branch: Some("main".to_string()),
            ssh_key_path: Some(ssh_key_path.clone()),
            ssh_key_passphrase: Some("repo_passphrase".to_string()),
            indexed_languages: None,
            added_as_local_path: false,
            target_ref: None,
            dependencies: vec![],
            last_synced_commit: None,
        };
        
        let repo_configs = vec![repo_config];
        let repo_url = "git@github.com:example/repo.git";
        
        let result = create_fetch_options(repo_configs, repo_url, None, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_collect_files_from_tree() {
        // Create a test repository
        let temp_dir = TempDir::new().unwrap();
        let repo = Repository::init(&temp_dir).unwrap();
        
        // Create test files
        let test_rs = temp_dir.path().join("test.rs");
        fs::write(&test_rs, "fn main() {}").unwrap();
        
        let test_py = temp_dir.path().join("test.py");
        fs::write(&test_py, "print('hello')").unwrap();
        
        let test_txt = temp_dir.path().join("test.txt");
        fs::write(&test_txt, "not supported").unwrap();
        
        // Create subdirectory with file
        let sub_dir = temp_dir.path().join("src");
        fs::create_dir(&sub_dir).unwrap();
        let sub_file = sub_dir.join("lib.rs");
        fs::write(&sub_file, "pub fn lib() {}").unwrap();
        
        // Add files to index and commit
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("test.rs")).unwrap();
        index.add_path(Path::new("test.py")).unwrap();
        index.add_path(Path::new("test.txt")).unwrap();
        index.add_path(Path::new("src/lib.rs")).unwrap();
        index.write().unwrap();
        
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        
        // Test collecting files
        let mut file_list = Vec::new();
        let result = collect_files_from_tree(&repo, &tree, &mut file_list, Path::new(""));
        
        assert!(result.is_ok());
        assert_eq!(file_list.len(), 3); // Should have test.rs, test.py, and src/lib.rs
        
        // Check that correct files were collected
        let file_names: Vec<String> = file_list.iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        
        assert!(file_names.contains(&"test.rs".to_string()));
        assert!(file_names.contains(&"test.py".to_string()));
        assert!(file_names.contains(&"src/lib.rs".to_string()));
        assert!(!file_names.contains(&"test.txt".to_string())); // Should not include .txt
    }

    #[test]
    fn test_collect_files_empty_tree() {
        let temp_dir = TempDir::new().unwrap();
        let repo = Repository::init(&temp_dir).unwrap();
        
        // Create empty tree
        let mut index = repo.index().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        
        let mut file_list = Vec::new();
        let result = collect_files_from_tree(&repo, &tree, &mut file_list, Path::new(""));
        
        assert!(result.is_ok());
        assert!(file_list.is_empty());
    }
} 