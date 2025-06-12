// TODO: Implement model discovery and caching in Phase 2
// This is a placeholder to make the code compile

use super::api::ModelInfo;

pub struct ModelManager {
    // Placeholder
}

impl ModelManager {
    pub fn new() -> Self {
        Self {}
    }
    
    pub async fn get_available_models(&self) -> Result<Vec<ModelInfo>, super::error::OpenRouterError> {
        // TODO: Implement model discovery
        Ok(vec![])
    }
} 