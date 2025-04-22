use crate::chain::action::Action;
use crate::chain::state::ChainState;
use crate::context::AppContext;
use crate::utils::error::{RelayError, Result};
use tracing::{info, debug, error};
// ... rest of imports ...

/// Responsible for executing a sequence of actions.
#[derive(Default)]
pub struct ChainExecutor {
    actions: Vec<Box<dyn Action>>,
}

impl ChainExecutor {
    // ... new() and add_action() remain the same ...
    pub fn new() -> Self {
        Default::default()
    }

    pub fn add_action(mut self, action: Box<dyn Action>) -> Self {
        self.actions.push(action);
        self
    }

    /// Executes the chain of actions sequentially.
    /// Takes the shared AppContext and an initial state.
    /// Returns the final state or an error.
    pub async fn execute(
        self,
        app_context: &AppContext,
        initial_state: ChainState,
    ) -> Result<ChainState> {
        let mut state = initial_state;
        info!("Starting chain execution with {} actions.", self.actions.len());

        for (index, action) in self.actions.iter().enumerate() {
            debug!(action_name = %action.name(), action_index = index, "Executing action");
            match action.execute(app_context, &mut state).await {
                Ok(_) => {
                    debug!(action_name = %action.name(), action_index = index, "Action executed successfully");
                }
                Err(e) => {
                    error!(action_name = %action.name(), action_index = index, error = %e, "Action execution failed");
                    return Err(RelayError::ChainError(format!(
                        "Error executing action '{}' (index {}): {}",
                        action.name(),
                        index,
                        e
                    )));
                }
            }
        }

        info!("Chain execution completed successfully.");
        Ok(state)
    }
}
