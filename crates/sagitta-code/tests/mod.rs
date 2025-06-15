pub mod common;

// Initialize test isolation globally when any test module is loaded
#[ctor::ctor]
fn init_test_isolation() {
    common::init_test_isolation();
} 