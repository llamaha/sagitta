pub mod types;
pub mod filters;
pub mod organization;
pub mod rendering;
pub mod state;
pub mod simple_types;
pub mod simple_rendering;

#[cfg(test)]
pub mod tests;

#[cfg(test)]
mod integration_test;

// Re-export the main sidebar struct and action enum
pub use types::{SidebarAction, ConversationSidebar, OrganizationMode};
// Re-export simple types
pub use simple_types::{SimpleSidebarAction, SimpleConversationSidebar, SimpleConversationItem};
pub use simple_rendering::*; 