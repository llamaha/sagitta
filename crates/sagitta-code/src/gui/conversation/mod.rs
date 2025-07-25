pub mod simple_types;
pub mod simple_persistence;
pub mod simple_manager;
pub mod panel;

#[cfg(test)]
mod panel_tests;

// Re-export key types
pub use simple_types::*;
pub use simple_persistence::*;
pub use simple_manager::*;
pub use panel::*; 