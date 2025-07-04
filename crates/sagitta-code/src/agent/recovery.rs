use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::broadcast;
use log::{info, warn};
use chrono;

use crate::agent::state::manager::StateManager;
use crate::agent::events::AgentEvent;
use crate::utils::errors::SagittaCodeError;

/// Recovery configuration for the agent
#[derive(Debug, Clone)]
pub struct RecoveryConfig {
    /// Maximum number of retry attempts for network failures
    pub max_retries: u32,
    
    /// Delay between retry attempts (in seconds)
    pub retry_delay_seconds: u64,
    
    /// Maximum time to wait for LLM response before timeout (in seconds)
    pub llm_timeout_seconds: u64,
    
    /// Whether to enable automatic recovery for network failures
    pub enable_auto_recovery: bool,
    
    /// Whether to enable recovery for tool execution failures
    pub enable_tool_recovery: bool,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay_seconds: 2,
            llm_timeout_seconds: 120,
            enable_auto_recovery: true,
            enable_tool_recovery: true,
        }
    }
}

/// Recovery state tracking
#[derive(Debug, Clone)]
pub struct RecoveryState {
    /// Current retry attempt count
    pub retry_count: u32,
    
    /// Last error that triggered recovery
    pub last_error: Option<String>,
    
    /// Whether the agent is currently in recovery mode
    pub in_recovery: bool,
    
    /// Timestamp of last recovery attempt
    pub last_recovery_attempt: Option<chrono::DateTime<chrono::Utc>>,
}

impl Default for RecoveryState {
    fn default() -> Self {
        Self {
            retry_count: 0,
            last_error: None,
            in_recovery: false,
            last_recovery_attempt: None,
        }
    }
}

/// Recovery manager for handling agent error recovery
#[derive(Clone)]
pub struct RecoveryManager {
    /// Recovery configuration
    config: RecoveryConfig,
    
    /// Recovery state tracking
    state: Arc<Mutex<RecoveryState>>,
    
    /// State manager for updating agent state
    state_manager: Arc<StateManager>,
    
    /// Event sender for recovery events
    event_sender: broadcast::Sender<AgentEvent>,
}

impl RecoveryManager {
    /// Create a new recovery manager
    pub fn new(
        config: RecoveryConfig,
        state_manager: Arc<StateManager>,
        event_sender: broadcast::Sender<AgentEvent>,
    ) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(RecoveryState::default())),
            state_manager,
            event_sender,
        }
    }
    
    /// Get the current recovery state
    pub fn get_recovery_state(&self) -> RecoveryState {
        let state = self.state.lock().unwrap();
        state.clone()
    }
    
    /// Reset the recovery state
    pub fn reset_recovery_state(&self) {
        let mut state = self.state.lock().unwrap();
        *state = RecoveryState::default();
        info!("Recovery state reset");
    }
    
    /// Check if the agent should attempt recovery
    pub fn should_attempt_recovery(&self, error: &SagittaCodeError) -> bool {
        let recovery_state = self.state.lock().unwrap();
        
        if !self.config.enable_auto_recovery {
            return false;
        }
        
        if recovery_state.retry_count >= self.config.max_retries {
            warn!("Maximum retry attempts ({}) reached, not attempting recovery", self.config.max_retries);
            return false;
        }
        
        // Check if this is a recoverable error
        match error {
            SagittaCodeError::NetworkError(_) => true,
            SagittaCodeError::LlmError(msg) => {
                let msg_lower = msg.to_lowercase();
                msg_lower.contains("timeout") || 
                msg_lower.contains("connection") || 
                msg_lower.contains("rate limit") || 
                msg_lower.contains("quota") ||
                msg_lower.contains("rate_limit") ||
                msg_lower.contains("too many requests")
            },
            SagittaCodeError::ToolError(msg) => {
                let msg_lower = msg.to_lowercase();
                msg_lower.contains("network") || 
                msg_lower.contains("timeout") ||
                msg_lower.contains("connection")
            },
            _ => false,
        }
    }
    
    /// Attempt recovery from an error
    pub async fn attempt_recovery(&self, error: &SagittaCodeError, context: &str) -> Result<(), SagittaCodeError> {
        let retry_count = {
            let mut recovery_state = self.state.lock().unwrap();
            
            recovery_state.retry_count += 1;
            recovery_state.last_error = Some(error.to_string());
            recovery_state.in_recovery = true;
            recovery_state.last_recovery_attempt = Some(chrono::Utc::now());
            
            recovery_state.retry_count
        }; // MutexGuard is dropped here
        
        warn!("Attempting recovery (attempt {}/{}) for error in {}: {}", 
              retry_count, self.config.max_retries, context, error);
        
        // Set agent state to indicate recovery
        self.state_manager.set_error(
            format!("Recovery attempt {}/{}", retry_count, self.config.max_retries),
            &format!("Recovering from: {}", error)
        ).await?;
        
        // Emit recovery event
        let _ = self.event_sender.send(AgentEvent::Error(format!(
            "Recovery attempt {}/{}: {}", retry_count, self.config.max_retries, error
        )));
        
        // Wait before retry
        let delay = Duration::from_secs(self.config.retry_delay_seconds);
        info!("Waiting {} seconds before retry attempt {}", self.config.retry_delay_seconds, retry_count);
        tokio::time::sleep(delay).await;
        
        Ok(())
    }
    
    /// Complete recovery (called on successful operation after recovery)
    pub fn complete_recovery(&self) {
        let mut recovery_state = self.state.lock().unwrap();
        if recovery_state.in_recovery {
            info!("Recovery completed successfully after {} attempts", recovery_state.retry_count);
            *recovery_state = RecoveryState::default();
        }
    }
    
    /// Get the LLM timeout duration
    pub fn get_llm_timeout(&self) -> Duration {
        Duration::from_secs(self.config.llm_timeout_seconds)
    }
    
    /// Update recovery configuration
    pub fn update_config(&mut self, config: RecoveryConfig) {
        self.config = config;
    }
    
    /// Get recovery configuration
    pub fn get_config(&self) -> &RecoveryConfig {
        &self.config
    }
} 