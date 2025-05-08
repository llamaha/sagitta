use anyhow::{anyhow, bail, Result};
use vectordb_core::config::{AppConfig, RepositoryConfig};

/// Retrieves the configuration for the target repository.
/// 
/// Priority:
/// 1. If `name_override` is provided, use that repository.
/// 2. If `name_override` is None, use the `active_repository` from the config.
/// 3. If neither is available, return an error.
pub fn get_active_repo_config<'a>(
    config: &'a AppConfig,
    name_override: Option<&str>,
) -> Result<&'a RepositoryConfig> {
    let repo_name = match name_override {
        Some(name) => name.to_string(),
        None => config.active_repository.clone().ok_or_else(|| {
            anyhow!("No active repository set and no repository name provided. Use 'repo use <name>' or specify --name.")
        })?,
    };

    config
        .repositories
        .iter()
        .find(|r| r.name == repo_name)
        .ok_or_else(|| anyhow!("Repository '{}' not found in configuration.", repo_name))
} 