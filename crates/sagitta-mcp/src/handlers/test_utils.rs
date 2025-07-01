// Shared test utilities for handlers

use std::sync::Mutex;

// Global mutex for todo tests to prevent concurrent directory changes
pub static TODO_TEST_MUTEX: Mutex<()> = Mutex::new(());