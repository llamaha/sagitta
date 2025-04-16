use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// VectorDB client configuration
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Server address (host:port)
    pub server_address: String,
    /// Use TLS for connection
    pub use_tls: bool,
    /// API key for authentication
    pub api_key: Option<String>,
    /// CA certificate path for TLS
    pub ca_cert_path: Option<PathBuf>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            server_address: "http://localhost:50051".to_string(),
            use_tls: false,
            api_key: None,
            ca_cert_path: None,
        }
    }
}

impl ClientConfig {
    /// Create a new client configuration
    pub fn new<S: Into<String>>(server_address: S) -> Self {
        Self {
            server_address: server_address.into(),
            ..Default::default()
        }
    }
    
    /// Set TLS mode
    pub fn with_tls(mut self, use_tls: bool) -> Self {
        self.use_tls = use_tls;
        self
    }
    
    /// Set API key
    pub fn with_api_key<S: Into<String>>(mut self, api_key: S) -> Self {
        self.api_key = Some(api_key.into());
        self
    }
    
    /// Set CA certificate path
    pub fn with_ca_cert<P: Into<PathBuf>>(mut self, ca_cert_path: P) -> Self {
        self.ca_cert_path = Some(ca_cert_path.into());
        self
    }
}

/// Serializable client configuration for saving/loading
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableClientConfig {
    /// Server address (host:port)
    pub server_address: String,
    /// Use TLS for connection
    pub use_tls: bool,
    /// API key for authentication
    pub api_key: Option<String>,
    /// CA certificate path for TLS
    pub ca_cert_path: Option<String>,
}

impl From<&ClientConfig> for SerializableClientConfig {
    fn from(config: &ClientConfig) -> Self {
        Self {
            server_address: config.server_address.clone(),
            use_tls: config.use_tls,
            api_key: config.api_key.clone(),
            ca_cert_path: config.ca_cert_path.as_ref().map(|p| p.to_string_lossy().to_string()),
        }
    }
}

impl From<SerializableClientConfig> for ClientConfig {
    fn from(config: SerializableClientConfig) -> Self {
        Self {
            server_address: config.server_address,
            use_tls: config.use_tls,
            api_key: config.api_key,
            ca_cert_path: config.ca_cert_path.map(PathBuf::from),
        }
    }
}

impl SerializableClientConfig {
    /// Load client configuration from a file
    pub fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, std::io::Error> {
        let contents = std::fs::read_to_string(path)?;
        toml::from_str(&contents).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Failed to parse config: {}", e))
        })
    }
    
    /// Save client configuration to a file
    pub fn save_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), std::io::Error> {
        let contents = toml::to_string_pretty(self).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to serialize config: {}", e))
        })?;
        std::fs::write(path, contents)
    }
}

impl ClientConfig {
    /// Load configuration from a file
    pub fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, std::io::Error> {
        let config = SerializableClientConfig::load_from_file(path)?;
        Ok(config.into())
    }
    
    /// Save configuration to a file
    pub fn save_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), std::io::Error> {
        let config = SerializableClientConfig::from(self);
        config.save_to_file(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_client_config_default() {
        let config = ClientConfig::default();
        assert_eq!(config.server_address, "http://localhost:50051");
        assert_eq!(config.use_tls, false);
        assert_eq!(config.api_key, None);
        assert_eq!(config.ca_cert_path, None);
    }
    
    #[test]
    fn test_client_config_new() {
        let config = ClientConfig::new("https://example.com:8080");
        assert_eq!(config.server_address, "https://example.com:8080");
        assert_eq!(config.use_tls, false);
        assert_eq!(config.api_key, None);
        assert_eq!(config.ca_cert_path, None);
    }
    
    #[test]
    fn test_client_config_with_tls() {
        let config = ClientConfig::default().with_tls(true);
        assert_eq!(config.use_tls, true);
    }
    
    #[test]
    fn test_client_config_with_api_key() {
        let config = ClientConfig::default().with_api_key("test-key");
        assert_eq!(config.api_key, Some("test-key".to_string()));
    }
    
    #[test]
    fn test_client_config_with_ca_cert() {
        let path = PathBuf::from("/path/to/cert.pem");
        let config = ClientConfig::default().with_ca_cert(&path);
        assert_eq!(config.ca_cert_path, Some(path));
    }
    
    #[test]
    fn test_serializable_client_config_conversion() {
        let config = ClientConfig {
            server_address: "https://example.com:8080".to_string(),
            use_tls: true,
            api_key: Some("test-key".to_string()),
            ca_cert_path: Some(PathBuf::from("/path/to/cert.pem")),
        };
        
        let serializable: SerializableClientConfig = (&config).into();
        assert_eq!(serializable.server_address, "https://example.com:8080");
        assert_eq!(serializable.use_tls, true);
        assert_eq!(serializable.api_key, Some("test-key".to_string()));
        assert_eq!(serializable.ca_cert_path, Some("/path/to/cert.pem".to_string()));
        
        let back_to_config: ClientConfig = serializable.into();
        assert_eq!(back_to_config.server_address, config.server_address);
        assert_eq!(back_to_config.use_tls, config.use_tls);
        assert_eq!(back_to_config.api_key, config.api_key);
        assert_eq!(back_to_config.ca_cert_path, config.ca_cert_path);
    }
    
    #[test]
    fn test_client_config_save_and_load() -> Result<(), std::io::Error> {
        let config = ClientConfig {
            server_address: "https://example.com:8080".to_string(),
            use_tls: true,
            api_key: Some("test-key".to_string()),
            ca_cert_path: Some(PathBuf::from("/path/to/cert.pem")),
        };
        
        // Create a temporary file
        let mut temp_file = NamedTempFile::new()?;
        let path = temp_file.path().to_owned();
        
        // Save the config
        config.save_to_file(&path)?;
        
        // Read the file contents to verify it's valid TOML
        let contents = std::fs::read_to_string(&path)?;
        assert!(contents.contains("server_address"));
        assert!(contents.contains("use_tls"));
        assert!(contents.contains("api_key"));
        assert!(contents.contains("ca_cert_path"));
        
        // Load the config
        let loaded_config = ClientConfig::load_from_file(&path)?;
        
        // Verify it matches
        assert_eq!(loaded_config.server_address, config.server_address);
        assert_eq!(loaded_config.use_tls, config.use_tls);
        assert_eq!(loaded_config.api_key, config.api_key);
        assert_eq!(loaded_config.ca_cert_path, config.ca_cert_path);
        
        Ok(())
    }
} 