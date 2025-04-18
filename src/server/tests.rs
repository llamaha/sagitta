#[cfg(test)]
#[cfg(feature = "server")]
mod tests {
    use std::sync::Arc;
    use std::net::SocketAddr;
    use tokio::sync::oneshot;
    use crate::config::AppConfig;
    use crate::server::{ServerConfig, start_server};
    use qdrant_client::Qdrant;
    use tokio::time::{timeout, Duration};
    
    #[tokio::test]
    async fn test_server_startup_shutdown() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap(); // Use port 0 for random available port
        let config = Arc::new(AppConfig::default());
        let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap());
        let _server_config = ServerConfig::default(); // Prefix with underscore
        
        let (tx, rx) = oneshot::channel();
        
        let server_handle = tokio::spawn(async move {
            start_server(addr, config, client, Some(rx), false, None, None).await
        });
        
        // Give the server a moment to start
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Send shutdown signal
        let _ = tx.send(());
        
        // Wait for server to shut down gracefully (with timeout)
        let result = timeout(Duration::from_secs(5), server_handle).await;
        
        assert!(result.is_ok(), "Server task timed out");
        let server_result = result.unwrap();
        assert!(server_result.is_ok(), "Server task panicked");
        assert!(server_result.unwrap().is_ok(), "Server returned an error during run");
    }
    
    // Helper function to create a mock Qdrant client for testing
    // This function appears unused.
    // fn mock_qdrant_client() -> Arc<Qdrant> {
    //     // For now, we'll use a real client but with a bogus URL
    //     // In a real test, we might want to use a proper mock
    //     let client = qdrant_client::Qdrant::from_url("http://localhost:6334")
    //         .build()
    //         .expect("Failed to create Qdrant client");
        
    //     Arc::new(client)
    // }
} 