use crate::mcp::types::{ErrorObject, CurrentWorkingDirectoryParams, CurrentWorkingDirectoryResult};
use crate::middleware::auth_middleware::AuthenticatedUser;
use sagitta_search::config::AppConfig;
use sagitta_search::qdrant_client_trait::QdrantClientTrait;
use axum::Extension;
use std::sync::Arc;
use tokio::sync::RwLock;
use super::utils::get_current_repository_path;


/// Handler for getting current working directory information
pub async fn handle_current_working_directory<C: QdrantClientTrait + Send + Sync + 'static>(
    _params: CurrentWorkingDirectoryParams,
    _config: Arc<RwLock<AppConfig>>,
    _qdrant_client: Arc<C>,
    _auth_user_ext: Option<Extension<AuthenticatedUser>>,
) -> Result<CurrentWorkingDirectoryResult, ErrorObject> {
    // Try to get repository context first
    if let Some(repo_path) = get_current_repository_path().await {
        // Extract repository name from path
        let repository_name = repo_path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|s| s.to_string());

        Ok(CurrentWorkingDirectoryResult {
            current_directory: repo_path.to_string_lossy().to_string(),
            is_repository: true,
            repository_name,
        })
    } else {
        // Fall back to system current directory
        match std::env::current_dir() {
            Ok(current_dir) => Ok(CurrentWorkingDirectoryResult {
                current_directory: current_dir.to_string_lossy().to_string(),
                is_repository: false,
                repository_name: None,
            }),
            Err(e) => Err(ErrorObject {
                code: -32603,
                message: format!("Failed to get current directory: {e}"),
                data: None,
            }),
        }
    }
}