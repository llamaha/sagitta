use std::path::PathBuf;

/// Get the current repository working directory from state file
pub async fn get_current_repository_path() -> Option<PathBuf> {
    // Try to read from a state file in the config directory
    let mut state_path = dirs::config_dir()?;
    state_path.push("sagitta-code");
    state_path.push("current_repository.txt");
    
    match tokio::fs::read_to_string(&state_path).await {
        Ok(content) => {
            let path_str = content.trim();
            if !path_str.is_empty() {
                let path = PathBuf::from(path_str);
                if path.exists() && path.is_dir() {
                    log::debug!("Read current repository path from state file: {}", path.display());
                    return Some(path);
                }
            }
        }
        Err(e) => {
            log::trace!("Could not read repository state file: {e}");
        }
    }
    
    // Fallback to environment variable
    if let Ok(repo_path) = std::env::var("SAGITTA_CURRENT_REPO_PATH") {
        let path = PathBuf::from(repo_path);
        if path.exists() && path.is_dir() {
            log::debug!("Using repository path from environment: {}", path.display());
            return Some(path);
        }
    }
    
    None
}