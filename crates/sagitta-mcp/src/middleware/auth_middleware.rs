// crates/sagitta-mcp/src/middleware/auth_middleware.rs

use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
use tracing::{info, warn};

use crate::http_transport::AppState;

pub const API_KEY_HEADER: &str = "X-API-Key";

// This struct will be populated and added to request extensions if auth is successful.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AuthenticatedUser {
    pub user_id: Option<String>,
    pub scopes: Vec<String>,
}

pub async fn auth_layer(
    State(app_state): State<AppState>,
    mut req: Request, 
    next: Next,
) -> Result<Response, StatusCode> {
    info!("Auth middleware: processing request to: {:?}", req.uri());

    let api_key_header = req.headers().get(API_KEY_HEADER).and_then(|value| value.to_str().ok());

    let mut authenticated_as: Option<AuthenticatedUser> = None;

    if let Some(api_key) = api_key_header {
        info!("Auth middleware: Found API key header");
        if let Some(key_info) = app_state.api_key_store.get_key_by_value(api_key).await {
            info!("Auth middleware: API key authenticated with ID: {:?}", key_info.key_id);
            authenticated_as = Some(AuthenticatedUser {
                user_id: key_info.user_id.clone(),
                scopes: key_info.scopes.clone(),
            });
        } else {
            warn!("Auth middleware: Invalid API key provided");
            return Err(StatusCode::UNAUTHORIZED);
        }
    } else {
        warn!("Auth middleware: No authentication provided");
        return Err(StatusCode::UNAUTHORIZED);
    }

    if let Some(user) = authenticated_as {
        info!("Auth middleware: User authenticated successfully: {:?}", user);
        req.extensions_mut().insert(user);
    } else {
        warn!("Auth middleware: Authentication failed");
        return Err(StatusCode::UNAUTHORIZED);
    }

    let res = next.run(req).await;
    Ok(res)
}