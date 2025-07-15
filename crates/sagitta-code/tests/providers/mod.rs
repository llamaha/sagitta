/// Provider test module
/// 
/// This module contains comprehensive tests for the provider system,
/// including trait implementations, factory patterns, and provider management.

pub mod mock_provider;
pub mod test_provider;
pub mod provider_tests;
pub mod factory_tests;
pub mod manager_tests;
pub mod integration_tests;

// Re-export commonly used test utilities
pub use mock_provider::*;
pub use test_provider::*;
pub use provider_tests::create_user_message;