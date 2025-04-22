use crate::config::RelayConfig;
use crate::llm::AnthropicClient;
use qdrant_client::Qdrant;
use vectordb_core::config::AppConfig as VdbConfig;
use anyhow::Result;
use std::sync::{Arc, Mutex};
use crate::utils::error::RelayError;

/// Holds shared application resources and configurations accessible by actions.
#[derive(Clone)]
pub struct AppContext {
    pub relay_config: Arc<RelayConfig>,
    pub vdb_config: Arc<VdbConfig>,
    pub llm_client: Arc<AnthropicClient>,
    pub qdrant_client: Arc<Qdrant>,
}

impl AppContext {
    // Constructor might be useful later
    // pub fn new(config: Arc<Config>, llm_client: Arc<AnthropicClient>, qdrant_client: Arc<Qdrant>) -> Self {
    //     Self { config, llm_client, qdrant_client }
    // }
} 