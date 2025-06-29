pub mod types;
pub mod loader;
pub mod paths;

// Re-export commonly used items
pub use types::{SagittaCodeConfig, SidebarPersistentConfig, SidebarFiltersConfig, SidebarPerformanceConfig};
pub use loader::{load_config, load_merged_config, load_config_from_path, save_config, save_config_to_path, load_all_configs};
pub use paths::{get_sagitta_code_app_config_path, get_conversations_path, get_logs_path, migrate_old_config};
