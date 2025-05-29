use axum::{
    async_trait,
    extract::{FromRequestParts, State},
    http::{request::Parts, StatusCode},
    response::Response,
};
use axum_limit::Key;
use serde::Serialize;
use tracing::{info, warn};
use std::convert::Infallible;

use crate::middleware::auth_middleware::AuthenticatedUser;

// 1. Define the Extractor for AuthenticatedUser (or just use AuthenticatedUser if it impls FromRequestParts directly)
// For rate limiting, we often need to extract something that identifies the user/tenant.
// AuthenticatedUser is already placed in extensions by auth_middleware.

// This struct will be our Extractor. It wraps AuthenticatedUser for clarity if needed,
// or we could try to impl Key directly using AuthenticatedUser as Extractor if it meets bounds.
// For simplicity, let's assume AuthenticatedUser itself can be the "thing extracted",
// and our Key::from_extractor will operate on Option<AuthenticatedUser>.

// Define TenantKey for rate limiting.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct TenantKey(pub String);

// Define an extractor specifically for getting Option<AuthenticatedUser>
// This helps manage the case where AuthenticatedUser might not be present.
#[derive(Debug, Clone)]
pub struct OptionalAuthUserExtractor(pub Option<AuthenticatedUser>);

#[async_trait]
impl<S> FromRequestParts<S> for OptionalAuthUserExtractor
where
    S: Send + Sync,
{
    type Rejection = Infallible; // Or a specific rejection type if extraction can fail recoverably

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // .get() returns Option<&AuthenticatedUser>, .cloned() gives Option<AuthenticatedUser>
        Ok(OptionalAuthUserExtractor(parts.extensions.get::<AuthenticatedUser>().cloned()))
    }
}

#[async_trait]
impl Key for TenantKey {
    // The Extractor will be our OptionalAuthUserExtractor
    type Extractor = OptionalAuthUserExtractor;

    // No Rejection type needed here as per axum-limit 0.1.0-alpha.2 Key trait

    fn from_extractor(extractor: &Self::Extractor) -> Self {
        match &extractor.0 { // extractor is &OptionalAuthUserExtractor, so extractor.0 is Option<AuthenticatedUser>
            Some(auth_user) => {
                // auth_user.tenant_id is now String and guaranteed to be present.
                info!("Rate limiting key from tenant_id: {}", auth_user.tenant_id);
                TenantKey(auth_user.tenant_id.clone())
            }
            None => {
                warn!("Rate limiting key for unauthenticated request (using default_unauth_key).");
                TenantKey("__default_unauthenticated__".to_string())
            }
        }
    }
}

// In the future, we might want different rate limits or keys based on more factors.
// For instance, a combined key (TenantId, UserId) or (IPAddress, TenantId). 