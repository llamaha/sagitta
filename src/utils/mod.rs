// Utility functions will be added here as needed 
pub mod git; 

/// Checks if experimental repository features are enabled via the VECTORDB_EXPERIMENTAL_REPO environment variable
pub fn is_repo_features_enabled() -> bool {
    match std::env::var("VECTORDB_EXPERIMENTAL_REPO") {
        Ok(value) => {
            // Check for truthy values
            matches!(value.to_lowercase().as_str(), "1" | "true" | "yes" | "on")
        },
        Err(_) => false, // Default to false if not set
    }
}

/// Helper function for clap to conditionally skip repo commands
pub fn is_repo_features_not_enabled() -> bool {
    !is_repo_features_enabled()
} 