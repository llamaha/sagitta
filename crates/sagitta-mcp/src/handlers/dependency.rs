use crate::mcp::types::{
    RepositoryDependencyParams, RepositoryDependencyResult, RepositoryListDependenciesParams,
    RepositoryListDependenciesResult, DependencyInfo,
};
use anyhow::Result;
use sagitta_search::config::{save_config, RepositoryDependency};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Adds or updates a dependency for a repository
pub async fn handle_repository_add_dependency(
    params: RepositoryDependencyParams,
    config_mutex: Arc<RwLock<sagitta_search::config::AppConfig>>,
) -> Result<RepositoryDependencyResult> {
    let mut config = config_mutex.write().await;
    
    // Check if dependency repository exists
    let dependency_exists = config.repositories.iter()
        .any(|r| r.name == params.dependency_name);
    
    if !dependency_exists {
        return Err(anyhow::anyhow!(
            "Dependency repository '{}' not found in repository manager. Add it first using repository_add.",
            params.dependency_name
        ));
    }
    
    // Find the main repository and update dependencies
    let main_repo_index = config.repositories.iter()
        .position(|r| r.name == params.repository_name)
        .ok_or_else(|| anyhow::anyhow!("Repository '{}' not found", params.repository_name))?;
    
    let main_repo = &mut config.repositories[main_repo_index];
    
    // Check if dependency already exists
    if let Some(existing) = main_repo.dependencies.iter_mut()
        .find(|d| d.repository_name == params.dependency_name) 
    {
        // Update existing dependency
        existing.target_ref = params.target_ref.clone();
        existing.purpose = params.purpose.clone();
        info!(
            "Updated dependency '{}' for repository '{}'",
            params.dependency_name, params.repository_name
        );
    } else {
        // Add new dependency
        main_repo.dependencies.push(RepositoryDependency {
            repository_name: params.dependency_name.clone(),
            target_ref: params.target_ref.clone(),
            purpose: params.purpose.clone(),
        });
        info!(
            "Added dependency '{}' to repository '{}'",
            params.dependency_name, params.repository_name
        );
    }
    
    // Save configuration only if not in test mode
    #[cfg(not(test))]
    save_config(&config, None)?;
    
    Ok(RepositoryDependencyResult {
        success: true,
        message: format!(
            "Successfully added dependency '{}' to repository '{}'",
            params.dependency_name, params.repository_name
        ),
    })
}

/// Removes a dependency from a repository
pub async fn handle_repository_remove_dependency(
    params: RepositoryDependencyParams,
    config_mutex: Arc<RwLock<sagitta_search::config::AppConfig>>,
) -> Result<RepositoryDependencyResult> {
    let mut config = config_mutex.write().await;
    
    // Find the main repository
    let main_repo = config.repositories.iter_mut()
        .find(|r| r.name == params.repository_name)
        .ok_or_else(|| anyhow::anyhow!("Repository '{}' not found", params.repository_name))?;
    
    // Remove the dependency
    let initial_count = main_repo.dependencies.len();
    main_repo.dependencies.retain(|d| d.repository_name != params.dependency_name);
    
    if main_repo.dependencies.len() < initial_count {
        #[cfg(not(test))]
        save_config(&config, None)?;
        info!(
            "Removed dependency '{}' from repository '{}'",
            params.dependency_name, params.repository_name
        );
        Ok(RepositoryDependencyResult {
            success: true,
            message: format!(
                "Successfully removed dependency '{}' from repository '{}'",
                params.dependency_name, params.repository_name
            ),
        })
    } else {
        Ok(RepositoryDependencyResult {
            success: false,
            message: format!(
                "Dependency '{}' not found in repository '{}'",
                params.dependency_name, params.repository_name
            ),
        })
    }
}

/// Lists all dependencies for a repository
pub async fn handle_repository_list_dependencies(
    params: RepositoryListDependenciesParams,
    config_mutex: Arc<RwLock<sagitta_search::config::AppConfig>>,
) -> Result<RepositoryListDependenciesResult> {
    let config = config_mutex.read().await;
    
    // Find the repository
    let repo = config.repositories.iter()
        .find(|r| r.name == params.repository_name)
        .ok_or_else(|| anyhow::anyhow!("Repository '{}' not found", params.repository_name))?;
    
    // Convert dependencies to DependencyInfo
    let dependencies: Vec<DependencyInfo> = repo.dependencies.iter()
        .map(|dep| {
            // Get additional info about the dependency repository
            let dep_repo = config.repositories.iter()
                .find(|r| r.name == dep.repository_name);
            
            DependencyInfo {
                repository_name: dep.repository_name.clone(),
                target_ref: dep.target_ref.clone(),
                purpose: dep.purpose.clone(),
                is_available: dep_repo.is_some(),
                local_path: dep_repo.map(|r| r.local_path.to_string_lossy().to_string()),
                current_ref: dep_repo.and_then(|r| r.active_branch.clone()),
            }
        })
        .collect();
    
    Ok(RepositoryListDependenciesResult {
        repository_name: params.repository_name,
        dependencies,
    })
}