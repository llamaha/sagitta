// crates/sagitta-code/src/gui/repository/shared_sync_state.rs
use once_cell::sync::Lazy;
use dashmap::DashMap;

use super::types::{SimpleSyncStatus, DisplayableSyncProgress};

/// Simple 0/1 progress + log lines – what the panel already shows
pub static SIMPLE_STATUS: Lazy<DashMap<String, SimpleSyncStatus>> =
    Lazy::new(DashMap::new);

/// Detailed stage/percentage information
pub static DETAILED_STATUS: Lazy<DashMap<String, DisplayableSyncProgress>> =
    Lazy::new(DashMap::new); 