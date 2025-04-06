use crate::vectordb::error::VectorDBError;
use anyhow::Result;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Creates a set of test files in the given directory
// Remove #[cfg(test)] to make the function accessible outside lib tests
pub fn create_test_files(dir_path: &str, count: usize) -> Result<()> {
    for i in 0..count {
        let test_file = PathBuf::from(dir_path).join(format!("test_{}.txt", i));
        let mut file = fs::File::create(&test_file).map_err(|e| VectorDBError::FileWriteError {
            path: test_file.clone(),
            source: e,
        })?;
        writeln!(file, "Test file content {}", i).map_err(|e| VectorDBError::FileWriteError {
            path: test_file,
            source: e,
        })?;
    }
    Ok(())
} 