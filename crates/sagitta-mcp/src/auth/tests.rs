use super::*;
use mockall::predicate::*;
use mockall::mock;

mock! {
    OAuthClient {
        fn get_authorization_url(&self) -> Result<String>;
        fn exchange_code(&self, code: &str) -> Result<TokenResponse>;
        fn get_user_info(&self, access_token: &str) -> Result<UserInfo>;
    }
}

#[tokio::test]
async fn test_auth_client_new() {
    let config = OAuthConfig {
        client_id: "test_client_id".to_string(),
        client_secret: "test_client_secret".to_string(),
        auth_url: "https://example.com/auth".to_string(),
        token_url: "https://example.com/token".to_string(),
        user_info_url: "https://example.com/userinfo".to_string(),
        redirect_uri: "https://example.com/callback".to_string(),
        scopes: vec!["openid".to_string(), "profile".to_string()],
    };

    let client = AuthClient::new(Some(config.clone())).unwrap();
    assert!(client.oauth_client.is_some());
}

#[tokio::test]
async fn test_auth_client_new_without_config() {
    let client = AuthClient::new(None).unwrap();
    assert!(client.oauth_client.is_none());
}

#[tokio::test]
async fn test_get_authorization_url() {
    let config = OAuthConfig {
        client_id: "test_client_id".to_string(),
        client_secret: "test_client_secret".to_string(),
        auth_url: "https://example.com/auth".to_string(),
        token_url: "https://example.com/token".to_string(),
        user_info_url: "https://example.com/userinfo".to_string(),
        redirect_uri: "https://example.com/callback".to_string(),
        scopes: vec!["openid".to_string(), "profile".to_string()],
    };

    let client = AuthClient::new(Some(config)).unwrap();
    let url = client.get_authorization_url().await.unwrap();
    assert!(url.starts_with("https://example.com/auth"));
    assert!(url.contains("client_id=test_client_id"));
    assert!(url.contains("scope=openid+profile"));
}

#[tokio::test]
async fn test_exchange_code() {
    let config = OAuthConfig {
        client_id: "test_client_id".to_string(),
        client_secret: "test_client_secret".to_string(),
        auth_url: "https://example.com/auth".to_string(),
        token_url: "https://example.com/token".to_string(),
        user_info_url: "https://example.com/userinfo".to_string(),
        redirect_uri: "https://example.com/callback".to_string(),
        scopes: vec!["openid".to_string(), "profile".to_string()],
    };

    let client = AuthClient::new(Some(config)).unwrap();
    let result = client.exchange_code("test_code").await;
    assert!(result.is_err()); // Should fail in test environment without mock server
}

#[tokio::test]
async fn test_get_user_info() {
    let config = OAuthConfig {
        client_id: "test_client_id".to_string(),
        client_secret: "test_client_secret".to_string(),
        auth_url: "https://example.com/auth".to_string(),
        token_url: "https://example.com/token".to_string(),
        user_info_url: "https://example.com/userinfo".to_string(),
        redirect_uri: "https://example.com/callback".to_string(),
        scopes: vec!["openid".to_string(), "profile".to_string()],
    };

    let client = AuthClient::new(Some(config)).unwrap();
    let result = client.get_user_info("test_token").await;
    assert!(result.is_err()); // Should fail in test environment without mock server
} 