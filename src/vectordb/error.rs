use thiserror::Error;
use std::path::PathBuf;

#[derive(Error, Debug)]
pub enum VectorDBError {
    #[error("Failed to read file {path:?}: {source}")]
    FileReadError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to write file {path:?}: {source}")]
    FileWriteError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to serialize data: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Failed to access file metadata for {path:?}: {source}")]
    MetadataError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Invalid file path: {0}")]
    InvalidPath(String),

    #[error("Failed to create directory {path:?}: {source}")]
    DirectoryCreationError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("Embedding error: {0}")]
    EmbeddingError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),
}

pub type Result<T> = std::result::Result<T, VectorDBError>;

impl From<std::io::Error> for VectorDBError {
    fn from(err: std::io::Error) -> Self {
        VectorDBError::FileReadError {
            path: PathBuf::from("<unknown>"),
            source: err,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;
    use serde::Serialize;

    #[test]
    fn test_file_read_error() {
        let nonexistent = PathBuf::from("/nonexistent/file");
        let err = std::fs::read_to_string(&nonexistent)
            .map_err(|e| VectorDBError::FileReadError {
                path: nonexistent.clone(),
                source: e,
            });
        
        assert!(err.is_err());
        let err_string = err.unwrap_err().to_string();
        assert!(err_string.contains("/nonexistent/file"));
    }

    #[test]
    fn test_file_write_error() {
        let dir = tempdir().unwrap();
        let readonly_path = dir.path().join("readonly.txt");
        File::create(&readonly_path).unwrap();
        
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&readonly_path, std::fs::Permissions::from_mode(0o444)).unwrap();
        }
        
        let err = std::fs::write(&readonly_path, "test")
            .map_err(|e| VectorDBError::FileWriteError {
                path: readonly_path.clone(),
                source: e,
            });
        
        assert!(err.is_err());
        let err_string = err.unwrap_err().to_string();
        assert!(err_string.contains("readonly.txt"));
    }

    #[test]
    fn test_serialization_error() {
        use std::collections::HashMap;
        
        // Create a struct that will fail to serialize
        #[derive(Serialize)]
        struct Test {
            map: HashMap<Vec<u8>, String>, // Vec<u8> cannot be a JSON object key
        }
        
        let mut map = HashMap::new();
        map.insert(vec![1, 2, 3], "test".to_string());
        let test = Test { map };
        
        let err = serde_json::to_string(&test).map_err(VectorDBError::from);
        assert!(err.is_err());
        let err_string = err.unwrap_err().to_string();
        assert!(err_string.contains("serialize"));
    }

    #[test]
    fn test_metadata_error() {
        let nonexistent = PathBuf::from("/nonexistent/file");
        let err = std::fs::metadata(&nonexistent)
            .map_err(|e| VectorDBError::MetadataError {
                path: nonexistent.clone(),
                source: e,
            });
        
        assert!(err.is_err());
        let err_string = err.unwrap_err().to_string();
        assert!(err_string.contains("/nonexistent/file"));
    }
} 