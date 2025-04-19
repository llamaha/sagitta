use std::collections::HashSet;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;
use tracing::{debug, error, warn};
use crate::server::error::ServerError;
use tonic::{Request, Status};

const API_KEY_HEADER: &str = "x-api-key";

/// API key authenticator
#[derive(Debug, Clone)]
pub struct ApiKeyAuthenticator {
    api_keys: HashSet<String>,
    require_auth: bool,
}

impl ApiKeyAuthenticator {
    /// Create a new API key authenticator
    pub fn new(api_key_file: Option<&Path>, require_auth: bool) -> Result<Self, io::Error> {
        let mut api_keys = HashSet::new();
        
        if let Some(path) = api_key_file {
            debug!("Loading API keys from {:?}", path);
            let file = File::open(path)?;
            let reader = BufReader::new(file);
            
            for line in reader.lines() {
                let line = line?;
                let key = line.trim();
                if !key.is_empty() && !key.starts_with('#') {
                    api_keys.insert(key.to_string());
                }
            }
            debug!("Loaded {} API keys", api_keys.len());
        } else if require_auth {
            warn!("Authentication required but no API key file provided");
        }
        
        Ok(Self { api_keys, require_auth })
    }
    
    /// Authenticate a request
    pub fn authenticate<T>(&self, request: &Request<T>) -> Result<(), ServerError> {
        if !self.require_auth {
            return Ok(());
        }
        
        let metadata = request.metadata();
        
        // Get API key from headers
        if let Some(api_key) = metadata.get(API_KEY_HEADER) {
            if let Ok(key) = api_key.to_str() {
                if self.api_keys.contains(key) {
                    return Ok(());
                }
                debug!("Invalid API key provided");
                return Err(ServerError::Authentication("Invalid API key".to_string()));
            }
        }
        
        error!("Missing API key header");
        Err(ServerError::Authentication("Missing API key".to_string()))
    }
}

/// Middleware for authenticating gRPC requests
pub fn authenticate_request<T>(
    authenticator: &ApiKeyAuthenticator,
    request: &Request<T>,
) -> Result<(), Status> {
    match authenticator.authenticate(request) {
        Ok(_) => Ok(()),
        Err(e) => Err(e.into()),
    }
} 