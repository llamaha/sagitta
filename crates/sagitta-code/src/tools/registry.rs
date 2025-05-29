// Tool registration & lookup will go here

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::tools::types::{BoxedTool, ToolDefinition, ToolCategory};
use crate::utils::errors::FredAgentError;
use crate::gui::repository::manager::RepositoryManager;
use sagitta_search::config::AppConfig;
use std::sync::Mutex as SyncMutex;
use crate::tools::repository::*;

/// A registry of tools that can be used by the agent
#[derive(Debug, Clone)]
pub struct ToolRegistry {
    /// Map of tool names to implementations
    tools: Arc<RwLock<HashMap<String, BoxedTool>>>,
}

impl ToolRegistry {
    /// Create a new empty tool registry
    pub fn new() -> Self {
        Self {
            tools: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Register a tool with the registry
    pub async fn register(&self, tool: BoxedTool) -> Result<(), FredAgentError> {
        let def = tool.definition();
        let name = def.name.clone();
        
        let mut tools = self.tools.write().await;
        
        if tools.contains_key(&name) {
            return Err(FredAgentError::ToolError(format!(
                "Tool with name '{}' is already registered", name
            )));
        }
        
        tools.insert(name, tool);
        Ok(())
    }
    
    /// Unregister a tool from the registry
    pub async fn unregister(&self, name: &str) -> Result<(), FredAgentError> {
        let mut tools = self.tools.write().await;
        
        if !tools.contains_key(name) {
            return Err(FredAgentError::ToolError(format!(
                "Tool with name '{}' is not registered", name
            )));
        }
        
        tools.remove(name);
        Ok(())
    }
    
    /// Get a tool by name
    pub async fn get(&self, name: &str) -> Option<BoxedTool> {
        let tools = self.tools.read().await;
        tools.get(name).cloned()
    }
    
    /// Get all tool definitions
    pub async fn get_definitions(&self) -> Vec<ToolDefinition> {
        let tools = self.tools.read().await;
        tools.values().map(|tool| tool.definition()).collect()
    }
    
    /// Get tool definitions by category
    pub async fn get_definitions_by_category(&self, category: ToolCategory) -> Vec<ToolDefinition> {
        let tools = self.tools.read().await;
        tools.values()
            .map(|tool| tool.definition())
            .filter(|def| def.category == category)
            .collect()
    }
    
    /// Get required tool definitions
    pub async fn get_required_definitions(&self) -> Vec<ToolDefinition> {
        let tools = self.tools.read().await;
        tools.values()
            .map(|tool| tool.definition())
            .filter(|def| def.is_required)
            .collect()
    }
    
    /// Check if a tool with the given name exists
    pub async fn has_tool(&self, name: &str) -> bool {
        let tools = self.tools.read().await;
        tools.contains_key(name)
    }
    
    /// Get the number of registered tools
    pub async fn count(&self) -> usize {
        let tools = self.tools.read().await;
        tools.len()
    }
    
    /// Get the number of tools in a category
    pub async fn count_category(&self, category: ToolCategory) -> usize {
        let tools = self.tools.read().await;
        tools.values()
            .filter(|tool| tool.definition().category == category)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::repository::manager::RepositoryManager;
    use crate::tools::repository::*;
    use sagitta_search::config::AppConfig;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn test_tool_registry_basic_functionality() {
        let registry = ToolRegistry::new();
        
        // Initially empty
        let all_definitions = registry.get_definitions().await;
        assert!(all_definitions.is_empty(), "Registry should start empty");
        
        // Test count
        assert_eq!(registry.count().await, 0);
    }

    #[tokio::test]
    async fn test_tool_registry_categories() {
        let registry = ToolRegistry::new();
        let mock_config = Arc::new(Mutex::new(AppConfig::default()));
        let repo_manager = Arc::new(Mutex::new(RepositoryManager::new_for_test(mock_config)));
        
        // Register all expected repository tools
        registry.register(Arc::new(AddRepositoryTool::new(repo_manager.clone()))).await.unwrap();
        registry.register(Arc::new(SyncRepositoryTool::new(repo_manager.clone()))).await.unwrap();
        registry.register(Arc::new(RemoveRepositoryTool::new(repo_manager.clone()))).await.unwrap();
        registry.register(Arc::new(ListRepositoriesTool::new(repo_manager.clone()))).await.unwrap();
        registry.register(Arc::new(SearchFileInRepositoryTool::new(repo_manager.clone()))).await.unwrap();
        registry.register(Arc::new(ViewFileInRepositoryTool::new(repo_manager.clone()))).await.unwrap();
        
        // Test that we have the expected number of repository tools
        let repo_tools = registry.get_definitions_by_category(ToolCategory::Repository).await;
        assert_eq!(repo_tools.len(), 6);
        
        // Test total count
        assert_eq!(registry.count().await, 6);
    }
}

