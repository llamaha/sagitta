use crate::mcp::types::{ErrorObject, PingParams, PingResult};
use anyhow::Result;

pub async fn handle_ping(_params: PingParams) -> Result<PingResult, ErrorObject> {
    Ok(PingResult { message: "pong".to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::types::PingParams;

    #[tokio::test]
    async fn test_handle_ping_success() {
        let params = PingParams {}; // PingParams is currently an empty struct
        let result = handle_ping(params).await;

        assert!(result.is_ok());
        let ping_result = result.unwrap();
        assert_eq!(ping_result.message, "pong");
    }
} 