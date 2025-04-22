use crate::chain::action::Action;
use crate::chain::state::ChainState;
use crate::context::AppContext;
use crate::utils::error::{RelayError, Result};
use crate::utils::prompt_user_confirmation;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};
use tokio::process::Command;
use vectordb_core::repo_add::{self, AddRepoArgs};
use vectordb_core::config::{self as vdb_config, AppConfig};
use vectordb_lib::cli::repo_commands::r#use::{self as use_repo, UseRepoArgs};
use vectordb_lib::cli::repo_commands::list;
use vectordb_core::qdrant_client_trait::QdrantClientTrait;
use git2::Repository;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

pub mod actions;

// --- Init Repo Action ---
// Note: Uses git2 library to initialize a new repository.

// --- Add Repo Action ---
// Corresponds to `vectordb-cli repo add`

// --- Use Repo Action ---
// Corresponds to `vectordb-cli repo use` 