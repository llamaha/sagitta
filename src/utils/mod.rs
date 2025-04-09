// Utility functions will be added here as needed
pub mod git;

pub fn is_debug_mode() -> bool {
    std::env::var("VECTORDB_DEBUG").is_ok()
}

#[cfg(test)]
mod tests {
}
