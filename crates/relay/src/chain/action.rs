// src/chain/action.rs

use crate::chain::state::ChainState;
use crate::context::AppContext;
use crate::utils::error::Result;
use async_trait::async_trait;
use std::fmt::Debug;

/// Represents a single executable step in a chain.
#[async_trait]
pub trait Action: Debug + Send + Sync {
    /// Returns the name of the action.
    fn name(&self) -> &'static str;

    /// Executes the action logic.
    /// Accesses shared application context and modifies the chain state.
    /// Returns Ok(()) on success, or an error if execution fails.
    async fn execute(&self, context: &AppContext, state: &mut ChainState) -> Result<()>;
}