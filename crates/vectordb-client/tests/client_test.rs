use std::error::Error;
use vectordb_client::{VectorDBClient, ClientConfig};

/// This test is disabled by default as it requires a running server.
/// Use `cargo test --features server -- --ignored` to run it.
#[cfg(feature = "server")]
#[tokio::test]
#[ignore]
async fn test_connect_to_server() -> Result<(), Box<dyn Error>> {
    let config = ClientConfig::new("http://localhost:50051");
    let mut client = VectorDBClient::new(config).await?;
    
    let server_info = client.get_server_info().await?;
    println!("Connected to server version: {}", server_info.version);
    assert!(!server_info.version.is_empty(), "Server version should not be empty");
    
    Ok(())
}

/// This test verifies client configuration works correctly.
#[test]
fn test_client_config() {
    let config = ClientConfig::default();
    assert_eq!(config.server_address, "http://localhost:50051");
    assert_eq!(config.use_tls, false);
    assert_eq!(config.api_key, None);
    assert_eq!(config.ca_cert_path, None);
    
    let config = ClientConfig::new("http://example.com:8080")
        .with_tls(true)
        .with_api_key("my-api-key");
    
    assert_eq!(config.server_address, "http://example.com:8080");
    assert_eq!(config.use_tls, true);
    assert_eq!(config.api_key, Some("my-api-key".to_string()));
} 