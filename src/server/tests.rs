#[cfg(test)]
#[cfg(feature = "server")]
mod tests {
    use std::sync::Arc;
    use std::net::SocketAddr;
    use tokio::sync::oneshot;
    use crate::config::AppConfig;
    use crate::server::{ServerConfig, start_server};
    use qdrant_client::Qdrant;
    
    #[tokio::test]
    async fn test_server_startup_shutdown() {
        // Create a test configuration
        let config = AppConfig::default();
        let server_config = ServerConfig::default();
        
        // Choose a random high port for testing
        let port = 50052 + fastrand::u16(0..1000);
        let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
        
        // Create a shutdown channel
        let (tx, rx) = oneshot::channel();
        
        // Create a Qdrant client (mock)
        let client = mock_qdrant_client();
        
        // Start the server in a background task
        let server_handle = tokio::spawn(async move {
            match start_server(
                addr,
                Arc::new(config),
                client,
                Some(rx),
                false,
                None,
                None,
            )
            .await
            {
                Ok(_) => println!("Server shut down gracefully"),
                Err(e) => panic!("Server error: {}", e),
            }
        });
        
        // Wait a moment for the server to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        // Send the shutdown signal
        tx.send(()).unwrap();
        
        // Wait for the server to shut down
        let _ = tokio::time::timeout(
            tokio::time::Duration::from_secs(1),
            server_handle,
        )
        .await
        .expect("Server did not shut down within timeout");
    }
    
    // Helper function to create a mock Qdrant client for testing
    fn mock_qdrant_client() -> Arc<Qdrant> {
        // For now, we'll use a real client but with a bogus URL
        // In a real test, we might want to use a proper mock
        let client = qdrant_client::Qdrant::from_url("http://localhost:6334")
            .build()
            .expect("Failed to create Qdrant client");
        
        Arc::new(client)
    }
} 