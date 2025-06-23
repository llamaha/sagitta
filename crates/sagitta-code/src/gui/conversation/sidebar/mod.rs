pub mod types;
pub mod filters;
pub mod organization;
pub mod rendering;
pub mod state;

#[cfg(test)]
pub mod tests;

#[cfg(test)]
mod integration_test;

// Re-export the main sidebar struct and action enum
pub use types::{SidebarAction, ConversationSidebar, OrganizationMode}; 