use std::fmt;

#[derive(Debug)]
pub enum OpenRouterError {
    /// HTTP request failed
    HttpError(String),
    /// API key is missing or invalid
    AuthenticationError(String),
    /// Rate limit exceeded
    RateLimitError(String),
    /// Model not found or not available
    ModelError(String),
    /// JSON parsing error
    SerializationError(String),
    /// Streaming error
    StreamingError(String),
    /// Network connectivity error
    NetworkError(String),
    /// Configuration error
    ConfigError(String),
    /// Unknown error
    Unknown(String),
}

impl fmt::Display for OpenRouterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HttpError(msg) => write!(f, "HTTP error: {}", msg),
            Self::AuthenticationError(msg) => write!(f, "Authentication error: {}", msg),
            Self::RateLimitError(msg) => write!(f, "Rate limit error: {}", msg),
            Self::ModelError(msg) => write!(f, "Model error: {}", msg),
            Self::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            Self::StreamingError(msg) => write!(f, "Streaming error: {}", msg),
            Self::NetworkError(msg) => write!(f, "Network error: {}", msg),
            Self::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            Self::Unknown(msg) => write!(f, "Unknown error: {}", msg),
        }
    }
}

impl std::error::Error for OpenRouterError {}

impl From<reqwest::Error> for OpenRouterError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            Self::NetworkError(format!("Request timeout: {}", err))
        } else if err.is_connect() {
            Self::NetworkError(format!("Connection error: {}", err))
        } else {
            Self::HttpError(err.to_string())
        }
    }
}

impl From<serde_json::Error> for OpenRouterError {
    fn from(err: serde_json::Error) -> Self {
        Self::SerializationError(err.to_string())
    }
} 