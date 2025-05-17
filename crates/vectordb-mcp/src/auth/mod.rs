use anyhow::Result;
use oauth2::{
    basic::{BasicClient, BasicTokenIntrospectionResponse},
    AuthUrl,
    ClientId,
    ClientSecret,
    RedirectUrl,
    TokenUrl,
    Scope,
    TokenResponse as OAuthTokenResponse,
    AccessToken,
    IntrospectionUrl,
    StandardTokenIntrospectionResponse,
    TokenIntrospectionResponse,
};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use vectordb_core::config::OAuthConfig;
use anyhow::Context;
use async_trait::async_trait;

#[async_trait]
pub trait AuthClientOperations: Send + Sync {
    async fn validate_token(&self, token: &str) -> Result<bool>;
    async fn get_user_info(&self, access_token: &str) -> Result<UserInfo>;
    async fn get_authorization_url(&self) -> Result<String>;
    async fn exchange_code(&self, code: &str) -> Result<TokenResponse>;
}

#[derive(Debug, Clone)]
pub struct AuthClient {
    oauth_client: Option<oauth2::Client<
        oauth2::StandardErrorResponse<oauth2::basic::BasicErrorResponseType>,
        oauth2::StandardTokenResponse<oauth2::EmptyExtraTokenFields, oauth2::basic::BasicTokenType>,
        oauth2::basic::BasicTokenType,
        StandardTokenIntrospectionResponse<oauth2::EmptyExtraTokenFields, oauth2::basic::BasicTokenType>,
        oauth2::StandardRevocableToken,
        oauth2::StandardErrorResponse<oauth2::RevocationErrorResponseType>
    >>,
    config: Arc<RwLock<OAuthConfig>>,
}

impl AuthClient {
    pub fn new(config_opt: Option<OAuthConfig>) -> Result<Self> {
        let oauth_client_instance = if let Some(ref oauth_config) = config_opt {
            if oauth_config.client_id.is_empty() || 
               oauth_config.auth_url.is_empty() || 
               oauth_config.token_url.is_empty() || 
               oauth_config.redirect_uri.is_empty() {
                None
            } else {
                let client_secret_option = if oauth_config.client_secret.is_empty() {
                    None
                } else {
                    Some(ClientSecret::new(oauth_config.client_secret.clone()))
                };

                let mut client_builder = oauth2::Client::new(
                    ClientId::new(oauth_config.client_id.clone()),
                    client_secret_option,
                    AuthUrl::new(oauth_config.auth_url.clone())?,
                    Some(TokenUrl::new(oauth_config.token_url.clone())?),
                )
                .set_redirect_uri(RedirectUrl::new(oauth_config.redirect_uri.clone())?);
    
                if let Some(intro_url_str) = &oauth_config.introspection_url {
                    if !intro_url_str.is_empty() {
                        client_builder = client_builder.set_introspection_uri(
                            IntrospectionUrl::new(intro_url_str.clone())?
                        );
                    }
                }
                Some(client_builder)
            }
        } else {
            None
        };

        Ok(Self {
            oauth_client: oauth_client_instance,
            config: Arc::new(RwLock::new(config_opt.unwrap_or_default())),
        })
    }
}

#[async_trait]
impl AuthClientOperations for AuthClient {
    async fn get_authorization_url(&self) -> Result<String> {
        let config = self.config.read().await;
        let client = self.oauth_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("OAuth client not configured"))?;

        let (auth_url, _) = client
            .authorize_url(oauth2::CsrfToken::new_random)
            .add_scopes(config.scopes.iter().map(|s| Scope::new(s.clone())))
            .url();

        Ok(auth_url.to_string())
    }

    async fn exchange_code(&self, code: &str) -> Result<TokenResponse> {
        let client = self.oauth_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("OAuth client not configured"))?;

        let token_res = client
            .exchange_code(oauth2::AuthorizationCode::new(code.to_string()))
            .request_async(oauth2::reqwest::async_http_client)
            .await.context("Failed to exchange authorization code")?;

        Ok(TokenResponse {
            access_token: token_res.access_token().secret().to_string(),
            token_type: "Bearer".to_string(),
            expires_in: token_res.expires_in().map(|d| d.as_secs()),
            refresh_token: token_res.refresh_token().map(|rt| rt.secret().to_string()),
            scope: token_res.scopes().map_or_else(String::new, |scopes| {
                scopes.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" ")
            }),
        })
    }

    async fn validate_token(&self, token_str: &str) -> Result<bool> {
        let client = self.oauth_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("OAuth client not configured for introspection"))?;
        
        if client.introspection_url().is_none() {
            return Err(anyhow::anyhow!("Introspection URL not configured for OAuth client"));
        }

        let access_token = AccessToken::new(token_str.to_string());
        
        let introspection_response = client.introspect(&access_token)?
            .request_async(oauth2::reqwest::async_http_client)
            .await
            .context("Token introspection request failed")?;

        Ok(introspection_response.active())
    }

    async fn get_user_info(&self, access_token: &str) -> Result<UserInfo> {
        let config = self.config.read().await;
        if config.user_info_url.is_empty() {
            return Err(anyhow::anyhow!("User info URL not configured"));
        }
        let http_client = HttpClient::new();

        let response = http_client
            .get(&config.user_info_url)
            .bearer_auth(access_token)
            .send()
            .await.context("Failed to send user info request")?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_else(|_| "<failed to read error body>".to_string());
            return Err(anyhow::anyhow!(
                "Failed to get user info: status {}, body: {}",
                status, error_body
            ));
        }
        response.json::<UserInfo>().await.context("Failed to parse user info response")
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<u64>,
    pub refresh_token: Option<String>,
    pub scope: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct UserInfo {
    pub sub: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub picture: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectordb_core::config::OAuthConfig;
    use mockall::{mock, predicate::*};

    fn create_oauth_config_with_introspection() -> OAuthConfig {
        OAuthConfig {
            client_id: "test_client_id".to_string(),
            client_secret: "test_client_secret".to_string(),
            auth_url: "https://example.com/auth".to_string(),
            token_url: "https://example.com/token".to_string(),
            user_info_url: "https://example.com/userinfo".to_string(),
            redirect_uri: "https://example.com/callback".to_string(),
            introspection_url: Some("https://example.com/introspect".to_string()),
            scopes: vec!["openid".to_string(), "profile".to_string()],
        }
    }

    fn create_oauth_config_without_introspection() -> OAuthConfig {
        OAuthConfig {
            introspection_url: None,
            ..create_oauth_config_with_introspection()
        }
    }
    
    #[tokio::test]
    async fn auth_client_new_with_introspection_url() {
        let config = create_oauth_config_with_introspection();
        let auth_client = AuthClient::new(Some(config)).unwrap();
        assert!(auth_client.oauth_client.is_some());
        assert!(auth_client.oauth_client.as_ref().unwrap().introspection_url().is_some());
        assert_eq!(auth_client.oauth_client.as_ref().unwrap().introspection_url().unwrap().url().as_str(), "https://example.com/introspect");
    }

    #[tokio::test]
    async fn validate_token_requires_introspection_url_configured() {
        let config = create_oauth_config_without_introspection();
        let auth_client = AuthClient::new(Some(config)).unwrap();
        let result = auth_client.validate_token("some_token").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Introspection URL not configured"));
    }
} 