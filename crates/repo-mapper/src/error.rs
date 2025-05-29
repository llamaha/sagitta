use thiserror::Error;

/// Errors that can occur during repository mapping
#[derive(Error, Debug)]
pub enum RepoMapperError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),
    
    #[error("Repository path does not exist: {path}")]
    PathNotFound { path: String },
    
    #[error("No files found matching the specified criteria")]
    NoFilesFound,
    
    #[error("Invalid file extension filter: {extension}")]
    InvalidExtension { extension: String },
    
    #[error("Pattern search error: {message}")]
    PatternSearchError { message: String },
} 