use axum::{
    extract::{State, Json, Query, Path, Extension},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    middleware,
};
use serde::{Deserialize, Serialize};
use tracing::{info, error, warn};

use crate::http_transport::AppState;
use crate::api_key::{ApiKey, ApiKeyStore, ApiKeyInfo, InMemoryApiKeyStore, API_KEY_PREFIX};
use crate::middleware::auth_middleware::{API_KEY_HEADER, AuthenticatedUser};

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateApiKeyRequest {
    #[serde(default)]
    pub user_id: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub expires_at: Option<u64>, // Unix timestamp
}

// The response will be the full ApiKey struct, which includes the generated key value.
// This is typically only shown on creation.
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateApiKeyResponse {
    // We can directly use the ApiKey struct here if its fields are suitable for the response
    // or create a custom response struct if we want to shape it differently.
    // For now, let's assume ApiKey is fine.
    #[serde(flatten)]
    pub api_key: ApiKey,
    pub raw_key: String, // The unhashed key, only shown on creation
}

pub async fn create_api_key_handler(
    State(app_state): State<AppState>,
    Json(payload): Json<CreateApiKeyRequest>,
) -> impl IntoResponse {
    info!("Handling create API key request: {:?}", payload);

    match app_state.api_key_store.create_key(
        payload.user_id,
        payload.description,
        payload.scopes,
        payload.expires_at,
    ).await {
        Ok(created) => {
            info!("API key created successfully with ID: {}", created.api_key.id);
            (StatusCode::CREATED, Json(CreateApiKeyResponse {
                api_key: created.api_key,
                raw_key: created.raw_key,
            }))
        }
        Err(e) => {
            error!("Failed to create API key: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(CreateApiKeyResponse {
                api_key: ApiKey {
                    id: String::new(),
                    key: String::new(),
                    user_id: None,
                    description: Some(format!("Error: {}", e)),
                    created_at: 0,
                    expires_at: None,
                    last_used_at: None,
                    scopes: vec![],
                    revoked: false,
                },
                raw_key: String::new(),
            }))
        }
    }
}

// List keys response
#[derive(Debug, Serialize)]
pub struct ListApiKeysResponse {
    pub keys: Vec<ApiKey>,
}

pub async fn list_api_keys_handler(
    State(app_state): State<AppState>,
) -> impl IntoResponse {
    info!("Handling list API keys request");

    let keys = app_state.api_key_store.list_keys().await;
    
    info!("Found {} API keys", keys.len());
    (StatusCode::OK, Json(ListApiKeysResponse { keys }))
}

pub async fn delete_api_key_handler(
    State(app_state): State<AppState>,
    Path(key_id): Path<String>,
) -> impl IntoResponse {
    info!("Handling delete API key request for ID: {}", key_id);

    match app_state.api_key_store.delete_key(&key_id).await {
        Ok(()) => {
            info!("API key deleted successfully: {}", key_id);
            StatusCode::NO_CONTENT
        }
        Err(e) => {
            error!("Failed to delete API key {}: {}", key_id, e);
            StatusCode::NOT_FOUND
        }
    }
}