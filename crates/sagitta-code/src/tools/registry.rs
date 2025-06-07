// Tool registration & lookup will go here

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::tools::types::{BoxedTool, ToolDefinition, ToolCategory};
use crate::utils::errors::SagittaCodeError;
use crate::gui::repository::manager::RepositoryManager;
use sagitta_search::config::AppConfig;
use std::sync::Mutex as SyncMutex;
use crate::tools::repository::*;
use async_trait::async_trait;
use serde_json;

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
    pub async fn register(&self, tool: BoxedTool) -> Result<(), SagittaCodeError> {
        let def = tool.definition();
        let name = def.name.clone();
        
        let mut tools = self.tools.write().await;
        
        if tools.contains_key(&name) {
            return Err(SagittaCodeError::ToolError(format!(
                "Tool with name '{}' is already registered", name
            )));
        }
        
        tools.insert(name, tool);
        Ok(())
    }
    
    /// Unregister a tool from the registry
    pub async fn unregister(&self, name: &str) -> Result<(), SagittaCodeError> {
        let mut tools = self.tools.write().await;
        
        if !tools.contains_key(name) {
            return Err(SagittaCodeError::ToolError(format!(
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
    use crate::tools::types::{BoxedTool, Tool, ToolCategory, ToolDefinition, ToolResult};
    use sagitta_search::config::AppConfig;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use async_trait::async_trait;
    use serde_json::{self, Value};

    // MockTool for testing
    #[derive(Debug, Clone)]
    struct MockTool {
        name: String,
        category: ToolCategory,
        is_required: bool,
        description: String,
    }

    impl MockTool {
        fn new(name: &str, category: ToolCategory, is_required: bool) -> Self {
            Self {
                name: name.to_string(),
                category,
                is_required,
                description: format!("Description for {}", name),
            }
        }
    }

    #[async_trait]
    impl Tool for MockTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                name: self.name.clone(),
                description: self.description.clone(),
                category: self.category.clone(),
                parameters: serde_json::Value::Null,
                is_required: self.is_required,
                metadata: HashMap::new(),
            }
        }

        async fn execute(
            &self,
            _params: Value,
        ) -> Result<ToolResult, SagittaCodeError> {
            Ok(ToolResult::success(serde_json::json!({ "status": format!("{} invoked", self.name) })))
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

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

    #[tokio::test]
    async fn test_register_and_get_tool() {
        let registry = ToolRegistry::new();
        let mock_tool = Arc::new(MockTool::new("TestTool1", ToolCategory::Other, false));
        
        let registration_result = registry.register(mock_tool.clone()).await;
        assert!(registration_result.is_ok(), "Registration should succeed");

        assert!(registry.has_tool("TestTool1").await, "Tool should be present after registration");
        
        let retrieved_tool = registry.get("TestTool1").await;
        assert!(retrieved_tool.is_some(), "Should be able to retrieve the tool");
        if let Some(tool) = retrieved_tool {
            assert_eq!(tool.definition().name, "TestTool1");
            assert_eq!(tool.definition().category, ToolCategory::Other);
            assert!(!tool.definition().is_required);
        }
        
        assert_eq!(registry.count().await, 1, "Count should be 1 after registering one tool");
    }

    #[tokio::test]
    async fn test_register_duplicate_tool_fails() {
        let registry = ToolRegistry::new();
        let mock_tool1 = Arc::new(MockTool::new("DuplicateTool", ToolCategory::Other, false));
        
        registry.register(mock_tool1.clone()).await.unwrap(); // First registration should succeed
        
        let mock_tool2 = Arc::new(MockTool::new("DuplicateTool", ToolCategory::Core, true)); // Same name
        let registration_result = registry.register(mock_tool2).await;
        
        assert!(registration_result.is_err(), "Registering a tool with a duplicate name should fail");
        if let Err(SagittaCodeError::ToolError(msg)) = registration_result {
            assert!(msg.contains("Tool with name 'DuplicateTool' is already registered"));
        } else {
            panic!("Expected SagittaCodeError::ToolError for duplicate registration");
        }
        
        assert_eq!(registry.count().await, 1, "Count should remain 1 after failed duplicate registration");
        // Verify the original tool is still there and unchanged
        let retrieved_tool = registry.get("DuplicateTool").await.unwrap();
        assert_eq!(retrieved_tool.definition().category, ToolCategory::Other);
        assert!(!retrieved_tool.definition().is_required);
    }

    #[tokio::test]
    async fn test_unregister_tool() {
        let registry = ToolRegistry::new();
        let mock_tool = Arc::new(MockTool::new("EphemeralTool", ToolCategory::Other, false));
        
        registry.register(mock_tool.clone()).await.unwrap();
        assert!(registry.has_tool("EphemeralTool").await, "Tool should exist before unregistering");
        assert_eq!(registry.count().await, 1);

        let unregister_result = registry.unregister("EphemeralTool").await;
        assert!(unregister_result.is_ok(), "Unregistration should succeed");
        
        assert!(!registry.has_tool("EphemeralTool").await, "Tool should not exist after unregistering");
        assert!(registry.get("EphemeralTool").await.is_none(), "Getting unregistered tool should return None");
        assert_eq!(registry.count().await, 0, "Count should be 0 after unregistering the only tool");
    }

    #[tokio::test]
    async fn test_unregister_nonexistent_tool_fails() {
        let registry = ToolRegistry::new();
        
        let unregister_result = registry.unregister("NonExistentTool").await;
        assert!(unregister_result.is_err(), "Unregistering a non-existent tool should fail");
        
        if let Err(SagittaCodeError::ToolError(msg)) = unregister_result {
            assert!(msg.contains("Tool with name 'NonExistentTool' is not registered"));
        } else {
            panic!("Expected SagittaCodeError::ToolError for unregistering non-existent tool");
        }
        assert_eq!(registry.count().await, 0);
    }

    #[tokio::test]
    async fn test_get_required_definitions() {
        let registry = ToolRegistry::new();
        
        let required_tool1 = Arc::new(MockTool::new("RequiredTool1", ToolCategory::Core, true));
        let non_required_tool = Arc::new(MockTool::new("NonRequiredTool", ToolCategory::Other, false));
        let required_tool2 = Arc::new(MockTool::new("RequiredTool2", ToolCategory::Repository, true));
        
        registry.register(required_tool1.clone()).await.unwrap();
        registry.register(non_required_tool.clone()).await.unwrap();
        registry.register(required_tool2.clone()).await.unwrap();
        
        let required_defs = registry.get_required_definitions().await;
        assert_eq!(required_defs.len(), 2, "Should retrieve definitions for 2 required tools");
        
        let names: Vec<String> = required_defs.iter().map(|d| d.name.clone()).collect();
        assert!(names.contains(&"RequiredTool1".to_string()));
        assert!(names.contains(&"RequiredTool2".to_string()));
        assert!(!names.contains(&"NonRequiredTool".to_string()));

        for def in &required_defs {
            assert!(def.is_required, "All definitions retrieved should be for required tools");
        }

        // Register another non-required tool and check again
        let non_required_tool2 = Arc::new(MockTool::new("NonRequiredTool2", ToolCategory::Other, false));
        registry.register(non_required_tool2).await.unwrap();
        
        let required_defs_after_add = registry.get_required_definitions().await;
        assert_eq!(required_defs_after_add.len(), 2, "Adding a non-required tool should not change the required definitions list");
         let names_after_add: Vec<String> = required_defs_after_add.iter().map(|d| d.name.clone()).collect();
        assert!(names_after_add.contains(&"RequiredTool1".to_string()));
        assert!(names_after_add.contains(&"RequiredTool2".to_string()));
    }
}

