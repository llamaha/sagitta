pub mod types;
pub mod loader;
pub mod paths;

// Re-export commonly used items
pub use types::{SagittaCodeConfig, GeminiConfig};
pub use loader::{load_config, load_merged_config, load_config_from_path, save_config};
pub use paths::{get_sagitta_code_core_config_path, get_sagitta_code_app_config_path};
