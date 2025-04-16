//! gRPC client implementation for the VectorDB service.

use tonic::transport::{Channel, ClientTlsConfig};
use tonic::metadata::MetadataValue;
use std::convert::TryFrom;
use tonic::Request;

use crate::config::ClientConfig;
use crate::error::{ClientError, Result};
use vectordb_proto::vector_db_service_client::VectorDbServiceClient;
use vectordb_proto::vectordb::{
    Empty, ServerInfo, StatusResponse, CreateCollectionRequest,
    CollectionRequest, ListCollectionsResponse, QueryRequest, 
    QueryResponse, IndexFilesRequest, IndexResponse,
    AddRepositoryRequest, RepositoryRequest, RemoveRepositoryRequest,
    SyncRepositoryRequest, UseBranchRequest, ListRepositoriesResponse,
};

/// VectorDB gRPC client
pub struct VectorDBClient {
    client: VectorDbServiceClient<Channel>,
    config: ClientConfig,
}

impl VectorDBClient {
    /// Create a new VectorDB client with the given configuration
    pub async fn new(config: ClientConfig) -> Result<Self> {
        let client = Self::create_client(&config).await?;
        Ok(Self { client, config })
    }
    
    /// Create a new client with default configuration
    pub async fn default() -> Result<Self> {
        Self::new(ClientConfig::default()).await
    }
    
    /// Create a new client connected to the given address
    pub async fn connect<S: Into<String>>(address: S) -> Result<Self> {
        let config = ClientConfig::new(address);
        Self::new(config).await
    }
    
    async fn create_client(config: &ClientConfig) -> Result<VectorDbServiceClient<Channel>> {
        let channel = if config.use_tls {
            let tls_config = if let Some(ca_cert_path) = &config.ca_cert_path {
                let ca_cert = tokio::fs::read(ca_cert_path).await
                    .map_err(|e| ClientError::Configuration(format!("Failed to read CA certificate: {}", e)))?;
                    
                ClientTlsConfig::new()
                    .ca_certificate(tonic::transport::Certificate::from_pem(ca_cert))
                    .domain_name(Self::extract_domain(&config.server_address)?)
            } else {
                ClientTlsConfig::new()
                    .domain_name(Self::extract_domain(&config.server_address)?)
            };
            
            Channel::from_shared(config.server_address.clone())
                .map_err(|e| ClientError::Configuration(format!("Invalid server address: {}", e)))?
                .tls_config(tls_config)
                .map_err(|e| ClientError::Configuration(format!("TLS configuration error: {}", e)))?
                .connect()
                .await?
        } else {
            Channel::from_shared(config.server_address.clone())
                .map_err(|e| ClientError::Configuration(format!("Invalid server address: {}", e)))?
                .connect()
                .await?
        };
        
        // Create a basic client without any authentication
        let client = VectorDbServiceClient::new(channel);
        
        Ok(client)
    }
    
    fn extract_domain(address: &str) -> Result<String> {
        let parts: Vec<&str> = address.split("://").collect();
        let host_part = if parts.len() > 1 {
            parts[1]
        } else {
            parts[0]
        };
        
        let host = host_part.split(':').next().unwrap_or(host_part);
        Ok(host.to_string())
    }
    
    // Helper method to add authentication to requests if needed
    fn prepare_request<T>(&self, request: Request<T>) -> Request<T> {
        if let Some(api_key) = &self.config.api_key {
            // Try to add API key to metadata
            if let Ok(value) = MetadataValue::try_from(api_key.as_str()) {
                let mut req = request;
                req.metadata_mut().insert("x-api-key", value);
                return req;
            }
        }
        request
    }
    
    /// Get server information
    pub async fn get_server_info(&mut self) -> Result<ServerInfo> {
        let request = self.prepare_request(Request::new(Empty {}));
        let response = self.client.get_server_info(request).await?;
        Ok(response.into_inner())
    }
    
    /// Create a new collection
    pub async fn create_collection(
        &mut self, 
        name: String, 
        vector_size: i32, 
        distance: String
    ) -> Result<StatusResponse> {
        let request = self.prepare_request(Request::new(CreateCollectionRequest {
            name,
            vector_size,
            distance,
        }));
        
        let response = self.client.create_collection(request).await?;
        Ok(response.into_inner())
    }
    
    /// List all collections
    pub async fn list_collections(&mut self) -> Result<ListCollectionsResponse> {
        let request = self.prepare_request(Request::new(Empty {}));
        let response = self.client.list_collections(request).await?;
        Ok(response.into_inner())
    }
    
    /// Delete a collection
    pub async fn delete_collection(&mut self, name: String) -> Result<StatusResponse> {
        let request = self.prepare_request(Request::new(CollectionRequest {
            name,
        }));
        
        let response = self.client.delete_collection(request).await?;
        Ok(response.into_inner())
    }
    
    /// Clear a collection (delete and recreate empty)
    pub async fn clear_collection(&mut self, name: String) -> Result<StatusResponse> {
        let request = self.prepare_request(Request::new(CollectionRequest {
            name,
        }));
        
        let response = self.client.clear_collection(request).await?;
        Ok(response.into_inner())
    }
    
    /// Index files or directories into a collection
    pub async fn index_files(
        &mut self,
        collection_name: String,
        paths: Vec<String>,
        extensions: Vec<String>,
    ) -> Result<IndexResponse> {
        let request = self.prepare_request(Request::new(IndexFilesRequest {
            collection_name,
            paths,
            extensions,
        }));
        
        let response = self.client.index_files(request).await?;
        Ok(response.into_inner())
    }
    
    /// Query a collection for similar documents
    pub async fn query_collection(
        &mut self,
        collection_name: String,
        query_text: String,
        limit: i32,
        language: Option<String>,
        element_type: Option<String>,
    ) -> Result<QueryResponse> {
        let request = self.prepare_request(Request::new(QueryRequest {
            collection_name,
            query_text,
            limit,
            language,
            element_type,
        }));
        
        let response = self.client.query_collection(request).await?;
        Ok(response.into_inner())
    }
    
    /// Add a repository to be managed
    pub async fn add_repository(
        &mut self,
        url: String,
        local_path: Option<String>,
        name: Option<String>,
        branch: Option<String>,
        remote: Option<String>,
        ssh_key_path: Option<String>,
        ssh_passphrase: Option<String>,
    ) -> Result<StatusResponse> {
        let request = self.prepare_request(Request::new(AddRepositoryRequest {
            url,
            local_path,
            name,
            branch,
            remote,
            ssh_key_path,
            ssh_passphrase,
        }));
        
        let response = self.client.add_repository(request).await?;
        Ok(response.into_inner())
    }
    
    /// List all managed repositories
    pub async fn list_repositories(&mut self) -> Result<ListRepositoriesResponse> {
        let request = self.prepare_request(Request::new(Empty {}));
        let response = self.client.list_repositories(request).await?;
        Ok(response.into_inner())
    }
    
    /// Set the active repository
    pub async fn use_repository(&mut self, name: String) -> Result<StatusResponse> {
        let request = self.prepare_request(Request::new(RepositoryRequest {
            name,
        }));
        
        let response = self.client.use_repository(request).await?;
        Ok(response.into_inner())
    }
    
    /// Remove a repository
    pub async fn remove_repository(
        &mut self,
        name: String,
        skip_confirmation: bool,
    ) -> Result<StatusResponse> {
        let request = self.prepare_request(Request::new(RemoveRepositoryRequest {
            name,
            skip_confirmation,
        }));
        
        let response = self.client.remove_repository(request).await?;
        Ok(response.into_inner())
    }
    
    /// Synchronize a repository
    pub async fn sync_repository(
        &mut self,
        name: Option<String>,
        extensions: Vec<String>,
        force: bool,
    ) -> Result<StatusResponse> {
        let request = self.prepare_request(Request::new(SyncRepositoryRequest {
            name,
            extensions,
            force,
        }));
        
        let response = self.client.sync_repository(request).await?;
        Ok(response.into_inner())
    }
    
    /// Set the active branch
    pub async fn use_branch(
        &mut self,
        branch_name: String,
        repository_name: Option<String>,
    ) -> Result<StatusResponse> {
        let request = self.prepare_request(Request::new(UseBranchRequest {
            branch_name,
            repository_name,
        }));
        
        let response = self.client.use_branch(request).await?;
        Ok(response.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::metadata::MetadataMap;
    
    #[test]
    fn test_extract_domain_http() {
        let result = VectorDBClient::extract_domain("http://localhost:50051").unwrap();
        assert_eq!(result, "localhost");
    }
    
    #[test]
    fn test_extract_domain_https() {
        let result = VectorDBClient::extract_domain("https://example.com:8080").unwrap();
        assert_eq!(result, "example.com");
    }
    
    #[test]
    fn test_extract_domain_no_protocol() {
        let result = VectorDBClient::extract_domain("127.0.0.1:50051").unwrap();
        assert_eq!(result, "127.0.0.1");
    }
    
    #[test]
    fn test_extract_domain_no_port() {
        let result = VectorDBClient::extract_domain("https://api.example.com").unwrap();
        assert_eq!(result, "api.example.com");
    }
    
    #[test]
    fn test_prepare_request_with_api_key() {
        let client = VectorDBClient {
            client: VectorDbServiceClient::new(Channel::from_static("http://[::1]:50051")),
            config: ClientConfig {
                server_address: "http://localhost:50051".to_string(),
                use_tls: false,
                api_key: Some("test-api-key".to_string()),
                ca_cert_path: None,
            },
        };
        
        let request = Request::new(Empty {});
        let prepared = client.prepare_request(request);
        
        // Get the metadata and verify the API key is set
        let metadata: &MetadataMap = prepared.metadata();
        assert!(metadata.contains_key("x-api-key"));
        assert_eq!(
            metadata.get("x-api-key").unwrap().to_str().unwrap(),
            "test-api-key"
        );
    }
    
    #[test]
    fn test_prepare_request_without_api_key() {
        let client = VectorDBClient {
            client: VectorDbServiceClient::new(Channel::from_static("http://[::1]:50051")),
            config: ClientConfig {
                server_address: "http://localhost:50051".to_string(),
                use_tls: false,
                api_key: None,
                ca_cert_path: None,
            },
        };
        
        let request = Request::new(Empty {});
        let prepared = client.prepare_request(request);
        
        // No API key should be set
        let metadata: &MetadataMap = prepared.metadata();
        assert!(!metadata.contains_key("x-api-key"));
    }
}