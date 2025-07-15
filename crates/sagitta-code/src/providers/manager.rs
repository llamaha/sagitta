//! Provider management and state handling

use super::{ProviderType, ProviderConfig, Provider};
use crate::utils::errors::SagittaCodeError;
use crate::llm::client::LlmClient;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// State information for a provider
#[derive(Debug, Clone)]
pub struct ProviderState {
    /// The provider configuration
    pub config: ProviderConfig,
    /// Whether the provider is currently active
    pub is_active: bool,
    /// Last known availability status
    pub is_available: bool,
    /// Optional error message if the provider is not available
    pub error_message: Option<String>,
}

impl ProviderState {
    /// Creates a new provider state
    pub fn new(config: ProviderConfig) -> Self {
        Self {
            config,
            is_active: false,
            is_available: true,
            error_message: None,
        }
    }
    
    /// Marks the provider as active
    pub fn set_active(&mut self, active: bool) {
        self.is_active = active;
    }
    
    /// Updates the availability status
    pub fn set_availability(&mut self, available: bool, error_message: Option<String>) {
        self.is_available = available;
        self.error_message = error_message;
    }
}

/// Manages multiple providers and handles provider switching
pub struct ProviderManager {
    /// Registry of available providers
    providers: HashMap<ProviderType, Box<dyn Provider>>,
    /// Current state of each provider
    provider_states: Arc<RwLock<HashMap<ProviderType, ProviderState>>>,
    /// Currently active provider type
    active_provider_type: Arc<RwLock<Option<ProviderType>>>,
}

impl ProviderManager {
    /// Creates a new provider manager
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            provider_states: Arc::new(RwLock::new(HashMap::new())),
            active_provider_type: Arc::new(RwLock::new(None)),
        }
    }
    
    /// Registers a new provider
    pub fn register_provider(&mut self, provider: Box<dyn Provider>) {
        let provider_type = provider.provider_type();
        let default_config = provider.default_config();
        let state = ProviderState::new(default_config);
        
        // Store the provider and its state
        self.providers.insert(provider_type.clone(), provider);
        self.provider_states.write().unwrap().insert(provider_type, state);
    }
    
    /// Gets all registered provider types
    pub fn get_provider_types(&self) -> Vec<ProviderType> {
        self.providers.keys().cloned().collect()
    }
    
    /// Gets all enabled provider types
    pub fn get_enabled_provider_types(&self) -> Vec<ProviderType> {
        let states = self.provider_states.read().unwrap();
        states
            .iter()
            .filter(|(_, state)| state.config.enabled)
            .map(|(provider_type, _)| provider_type.clone())
            .collect()
    }
    
    /// Gets the current active provider type
    pub fn get_active_provider_type(&self) -> Option<ProviderType> {
        self.active_provider_type.read().unwrap().clone()
    }
    
    /// Sets the active provider type
    pub fn set_active_provider(&self, provider_type: ProviderType) -> Result<(), SagittaCodeError> {
        // Validate that the provider exists and is enabled
        let states = self.provider_states.read().unwrap();
        let state = states.get(&provider_type)
            .ok_or_else(|| SagittaCodeError::ConfigError(
                format!("Provider {:?} is not registered", provider_type)
            ))?;
            
        if !state.config.enabled {
            return Err(SagittaCodeError::ConfigError(
                format!("Provider {:?} is disabled", provider_type)
            ));
        }
        
        if !state.is_available {
            return Err(SagittaCodeError::LlmError(
                format!("Provider {:?} is not available: {}", 
                    provider_type, 
                    state.error_message.as_deref().unwrap_or("Unknown error")
                )
            ));
        }
        
        // Update active states
        drop(states);
        self.update_active_states(&provider_type);
        
        // Set the new active provider
        *self.active_provider_type.write().unwrap() = Some(provider_type);
        
        Ok(())
    }
    
    /// Creates an LLM client for the currently active provider
    pub fn create_active_client(
        &self,
        mcp_integration: std::sync::Arc<crate::providers::claude_code::mcp_integration::McpIntegration>
    ) -> Result<Box<dyn LlmClient>, SagittaCodeError> {
        let provider_type = self.get_active_provider_type()
            .ok_or_else(|| SagittaCodeError::ConfigError(
                "No active provider set".to_string()
            ))?;
            
        self.create_client_for_provider(&provider_type, mcp_integration)
    }
    
    /// Creates an LLM client for a specific provider
    pub fn create_client_for_provider(
        &self, 
        provider_type: &ProviderType,
        mcp_integration: std::sync::Arc<crate::providers::claude_code::mcp_integration::McpIntegration>
    ) -> Result<Box<dyn LlmClient>, SagittaCodeError> {
        let provider = self.providers.get(provider_type)
            .ok_or_else(|| SagittaCodeError::ConfigError(
                format!("Provider {:?} is not registered", provider_type)
            ))?;
            
        let states = self.provider_states.read().unwrap();
        let state = states.get(provider_type)
            .ok_or_else(|| SagittaCodeError::ConfigError(
                format!("No state found for provider {:?}", provider_type)
            ))?;
            
        provider.create_client(&state.config, mcp_integration)
    }
    
    /// Updates the configuration for a provider
    pub fn update_provider_config(&self, provider_type: &ProviderType, config: ProviderConfig) -> Result<(), SagittaCodeError> {
        // Validate the configuration
        let provider = self.providers.get(provider_type)
            .ok_or_else(|| SagittaCodeError::ConfigError(
                format!("Provider {:?} is not registered", provider_type)
            ))?;
            
        provider.validate_config(&config)?;
        
        // Update the state
        let mut states = self.provider_states.write().unwrap();
        if let Some(state) = states.get_mut(provider_type) {
            state.config = config;
        }
        
        Ok(())
    }
    
    /// Gets the current state of a provider
    pub fn get_provider_state(&self, provider_type: &ProviderType) -> Option<ProviderState> {
        self.provider_states.read().unwrap().get(provider_type).cloned()
    }
    
    /// Gets the current state of all providers
    pub fn get_all_provider_states(&self) -> HashMap<ProviderType, ProviderState> {
        self.provider_states.read().unwrap().clone()
    }
    
    /// Checks availability of all providers
    pub fn check_provider_availability(&self) {
        for (provider_type, provider) in &self.providers {
            let is_available = provider.is_available();
            let error_message = if is_available {
                None
            } else {
                Some("Provider is not available".to_string())
            };
            
            if let Some(state) = self.provider_states.write().unwrap().get_mut(provider_type) {
                state.set_availability(is_available, error_message);
            }
        }
    }
    
    /// Validates that at least one provider is available
    pub fn validate_providers(&self) -> Result<(), SagittaCodeError> {
        let states = self.provider_states.read().unwrap();
        let has_available = states
            .values()
            .any(|state| state.config.enabled && state.is_available);
            
        if !has_available {
            return Err(SagittaCodeError::ConfigError(
                "No providers are available. Please configure at least one provider.".to_string()
            ));
        }
        
        Ok(())
    }
    
    /// Gets the default provider (first available enabled provider)
    pub fn get_default_provider(&self) -> Option<ProviderType> {
        let states = self.provider_states.read().unwrap();
        
        // Try to find Claude Code first (original default)
        if let Some(state) = states.get(&ProviderType::ClaudeCode) {
            if state.config.enabled && state.is_available {
                return Some(ProviderType::ClaudeCode);
            }
        }
        
        // Otherwise, return the first available enabled provider
        states
            .iter()
            .find(|(_, state)| state.config.enabled && state.is_available)
            .map(|(provider_type, _)| provider_type.clone())
    }
    
    /// Updates the active state for all providers
    fn update_active_states(&self, active_provider_type: &ProviderType) {
        let mut states = self.provider_states.write().unwrap();
        for (provider_type, state) in states.iter_mut() {
            state.set_active(provider_type == active_provider_type);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{ClaudeCodeProvider};
    use crate::providers::mistral_rs::MistralRsProvider;
    
    fn create_test_manager() -> ProviderManager {
        ProviderManager::new()
    }
    
    #[test]
    fn test_provider_registration() {
        let mut manager = create_test_manager();
        let test_provider = ClaudeCodeProvider::new();
        
        manager.register_provider(Box::new(test_provider));
        
        let provider_types = manager.get_provider_types();
        assert_eq!(provider_types.len(), 1);
        assert_eq!(provider_types[0], ProviderType::ClaudeCode);
    }
    
    #[test]
    fn test_provider_activation() {
        let mut manager = create_test_manager();
        let test_provider = ClaudeCodeProvider::new();
        
        manager.register_provider(Box::new(test_provider));
        
        // Should be able to activate the provider
        assert!(manager.set_active_provider(ProviderType::ClaudeCode).is_ok());
        assert_eq!(manager.get_active_provider_type(), Some(ProviderType::ClaudeCode));
    }
    
    #[tokio::test]
    async fn test_provider_state_tracking() {
        let mut manager = create_test_manager();
        let test_provider = ClaudeCodeProvider::new();
        
        manager.register_provider(Box::new(test_provider));
        
        // Check initial state
        let state = manager.get_provider_state(&ProviderType::ClaudeCode).unwrap();
        assert!(state.is_available);
        assert_eq!(state.config.enabled, true);
        assert!(state.error_message.is_none());
    }
    
    #[test]
    fn test_get_provider() {
        let mut manager = create_test_manager();
        let test_provider = ClaudeCodeProvider::new();
        
        manager.register_provider(Box::new(test_provider));
        
        // Get the provider state
        let state = manager.get_provider_state(&ProviderType::ClaudeCode);
        assert!(state.is_some());
        let state = state.unwrap();
        assert!(state.is_available);
    }
}