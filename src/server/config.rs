use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Port to listen on
    #[serde(default = "default_port")]
    pub port: u16,
    
    /// Host address to bind to
    #[serde(default = "default_host")]
    pub host: String,
    
    /// Whether to use TLS
    #[serde(default)]
    pub use_tls: bool,
    
    /// Path to TLS certificate file
    pub cert_path: Option<PathBuf>,
    
    /// Path to TLS key file
    pub key_path: Option<PathBuf>,
    
    /// Direct API key for authentication
    pub api_key: Option<String>,
    
    /// Path to API key file for authentication
    pub api_key_file: Option<PathBuf>,
    
    /// Whether authentication is required
    #[serde(default)]
    pub require_auth: bool,
    
    /// Maximum number of concurrent requests
    #[serde(default = "default_max_concurrent_requests")]
    pub max_concurrent_requests: usize,
    
    /// Maximum batch size for indexing
    #[serde(default = "default_max_batch_size")]
    pub max_batch_size: usize,
}

fn default_port() -> u16 {
    50051
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_max_concurrent_requests() -> usize {
    100
}

fn default_max_batch_size() -> usize {
    128
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            host: default_host(),
            use_tls: false,
            cert_path: None,
            key_path: None,
            api_key: None,
            api_key_file: None,
            require_auth: false,
            max_concurrent_requests: default_max_concurrent_requests(),
            max_batch_size: default_max_batch_size(),
        }
    }
}

impl ServerConfig {
    /// Get the socket address for the server
    pub fn socket_addr(&self) -> std::io::Result<SocketAddr> {
        let addr = format!("{}:{}", self.host, self.port);
        addr.parse::<SocketAddr>().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Invalid socket address: {}", e),
            )
        })
    }
    
    /// Check if TLS configuration is valid
    pub fn validate_tls(&self) -> Result<(), String> {
        if self.use_tls {
            if self.cert_path.is_none() {
                return Err("TLS enabled but certificate path is missing".to_string());
            }
            if self.key_path.is_none() {
                return Err("TLS enabled but key path is missing".to_string());
            }
            
            // Check if files exist
            if let Some(cert_path) = &self.cert_path {
                if !cert_path.exists() {
                    return Err(format!("TLS certificate file not found: {:?}", cert_path));
                }
            }
            if let Some(key_path) = &self.key_path {
                if !key_path.exists() {
                    return Err(format!("TLS key file not found: {:?}", key_path));
                }
            }
        }
        Ok(())
    }
    
    /// Validate auth configuration
    pub fn validate_auth(&self) -> Result<(), String> {
        if self.require_auth && self.api_key.is_none() && self.api_key_file.is_none() {
            return Err("Authentication required but neither API key nor API key file is provided".to_string());
        }
        
        if let Some(api_key_file) = &self.api_key_file {
            if !api_key_file.exists() {
                return Err(format!("API key file not found: {:?}", api_key_file));
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_server_config_default() {
        let config = ServerConfig::default();
        assert_eq!(config.port, 50051);
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.use_tls, false);
        assert_eq!(config.cert_path, None);
        assert_eq!(config.key_path, None);
        assert_eq!(config.api_key, None);
        assert_eq!(config.api_key_file, None);
        assert_eq!(config.require_auth, false);
        assert_eq!(config.max_concurrent_requests, 100);
        assert_eq!(config.max_batch_size, 128);
    }

    #[test]
    fn test_server_config_socket_addr() {
        let config = ServerConfig {
            port: 8080,
            host: "127.0.0.1".to_string(),
            ..Default::default()
        };
        
        let addr = config.socket_addr().expect("Should create socket address");
        assert_eq!(addr.to_string(), "127.0.0.1:8080");
    }
    
    #[test]
    fn test_server_config_invalid_socket_addr() {
        let config = ServerConfig {
            port: 8080,
            host: "invalid:host".to_string(),
            ..Default::default()
        };
        
        assert!(config.socket_addr().is_err());
    }
    
    #[test]
    fn test_validate_tls_with_enabled_but_missing_paths() {
        let config = ServerConfig {
            use_tls: true,
            cert_path: None,
            key_path: None,
            ..Default::default()
        };
        
        let result = config.validate_tls();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("certificate path is missing"));
    }
    
    #[test]
    fn test_validate_auth_with_api_key() {
        let config = ServerConfig {
            require_auth: true,
            api_key: Some("test-key".to_string()),
            api_key_file: None,
            ..Default::default()
        };
        
        let result = config.validate_auth();
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_validate_auth_with_missing_credentials() {
        let config = ServerConfig {
            require_auth: true,
            api_key: None,
            api_key_file: None,
            ..Default::default()
        };
        
        let result = config.validate_auth();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("neither API key nor API key file is provided"));
    }
} 