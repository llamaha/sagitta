//! Utility functions for the Sagitta embedding engine.

use crate::error::{Result, SagittaEmbedError};
use std::path::Path;

/// Validates that a file exists and is readable.
pub fn validate_file_exists<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(SagittaEmbedError::file_not_found(path));
    }
    if !path.is_file() {
        return Err(SagittaEmbedError::file_system(format!(
            "Path exists but is not a file: {}",
            path.display()
        )));
    }
    Ok(())
}

/// Validates that a directory exists and is accessible.
pub fn validate_directory_exists<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(SagittaEmbedError::file_not_found(path));
    }
    if !path.is_dir() {
        return Err(SagittaEmbedError::file_system(format!(
            "Path exists but is not a directory: {}",
            path.display()
        )));
    }
    Ok(())
}

/// Validates that a path exists (can be file or directory).
pub fn validate_path_exists<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(SagittaEmbedError::file_not_found(path));
    }
    Ok(())
}

/// Normalizes text for embedding by trimming whitespace and handling empty strings.
pub fn normalize_text(text: &str) -> String {
    text.trim().to_string()
}

/// Validates that a batch of texts is not empty and contains valid strings.
pub fn validate_text_batch(texts: &[&str]) -> Result<()> {
    if texts.is_empty() {
        return Err(SagittaEmbedError::invalid_input(
            "Text batch cannot be empty"
        ));
    }

    for (i, text) in texts.iter().enumerate() {
        if text.trim().is_empty() {
            return Err(SagittaEmbedError::invalid_input(format!(
                "Text at index {} is empty or whitespace-only",
                i
            )));
        }
    }

    Ok(())
}

/// Validates embedding dimensions match expected values.
pub fn validate_embedding_dimensions(
    embeddings: &[Vec<f32>],
    expected_dimension: usize,
) -> Result<()> {
    for embedding in embeddings.iter() {
        if embedding.len() != expected_dimension {
            return Err(SagittaEmbedError::dimension_mismatch(
                expected_dimension,
                embedding.len(),
            ));
        }
    }
    Ok(())
}

/// Checks if CUDA is available in the current environment.
#[cfg(feature = "cuda")]
pub fn is_cuda_available() -> bool {
    // This is a simplified check - in a real implementation,
    // you might want to actually query CUDA runtime
    true
}

#[cfg(not(feature = "cuda"))]
pub fn is_cuda_available() -> bool {
    false
}

/// Gets the number of available CPU cores for parallel processing.
pub fn get_cpu_count() -> usize {
    num_cpus::get()
}

/// Calculates optimal batch size based on available resources.
pub fn calculate_optimal_batch_size(
    text_count: usize,
    max_batch_size: usize,
    available_memory_mb: Option<usize>,
) -> usize {
    let cpu_cores = get_cpu_count();
    
    // Base calculation on CPU cores
    let cpu_based_batch = std::cmp::min(text_count, cpu_cores * 4);
    
    // Apply memory constraints if provided
    let memory_constrained_batch = if let Some(memory_mb) = available_memory_mb {
        // Rough estimate: 1MB per text in batch (very conservative)
        std::cmp::min(cpu_based_batch, memory_mb)
    } else {
        cpu_based_batch
    };
    
    // Apply maximum batch size limit
    std::cmp::min(memory_constrained_batch, max_batch_size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_validate_file_exists() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        // Test non-existent file
        assert!(validate_file_exists(&file_path).is_err());
        
        // Create file and test again
        fs::write(&file_path, "test content").unwrap();
        assert!(validate_file_exists(&file_path).is_ok());
        
        // Test directory (should fail)
        assert!(validate_file_exists(temp_dir.path()).is_err());
    }

    #[test]
    fn test_validate_directory_exists() {
        let temp_dir = tempdir().unwrap();
        let dir_path = temp_dir.path();
        let file_path = dir_path.join("test.txt");
        
        // Test existing directory
        assert!(validate_directory_exists(dir_path).is_ok());
        
        // Test non-existent directory
        let non_existent = dir_path.join("non_existent");
        assert!(validate_directory_exists(&non_existent).is_err());
        
        // Create file and test (should fail)
        fs::write(&file_path, "test").unwrap();
        assert!(validate_directory_exists(&file_path).is_err());
    }

    #[test]
    fn test_normalize_text() {
        assert_eq!(normalize_text("  hello world  "), "hello world");
        assert_eq!(normalize_text("\t\ntest\t\n"), "test");
        assert_eq!(normalize_text(""), "");
    }

    #[test]
    fn test_validate_text_batch() {
        // Valid batch
        assert!(validate_text_batch(&["hello", "world"]).is_ok());
        
        // Empty batch
        assert!(validate_text_batch(&[]).is_err());
        
        // Batch with empty string
        assert!(validate_text_batch(&["hello", "", "world"]).is_err());
        
        // Batch with whitespace-only string
        assert!(validate_text_batch(&["hello", "   ", "world"]).is_err());
    }

    #[test]
    fn test_validate_embedding_dimensions() {
        let embeddings = vec![
            vec![1.0, 2.0, 3.0],
            vec![4.0, 5.0, 6.0],
        ];
        
        // Correct dimensions
        assert!(validate_embedding_dimensions(&embeddings, 3).is_ok());
        
        // Incorrect dimensions
        assert!(validate_embedding_dimensions(&embeddings, 4).is_err());
    }

    #[test]
    fn test_calculate_optimal_batch_size() {
        // Test basic calculation
        let batch_size = calculate_optimal_batch_size(100, 50, None);
        assert!(batch_size <= 50);
        assert!(batch_size > 0);
        
        // Test with memory constraint
        let batch_size = calculate_optimal_batch_size(100, 50, Some(10));
        assert!(batch_size <= 10);
        
        // Test with small text count
        let batch_size = calculate_optimal_batch_size(5, 50, None);
        assert_eq!(batch_size, 5);
    }

    #[test]
    fn test_get_cpu_count() {
        let count = get_cpu_count();
        assert!(count > 0);
        assert!(count <= 256); // Reasonable upper bound
    }
} 