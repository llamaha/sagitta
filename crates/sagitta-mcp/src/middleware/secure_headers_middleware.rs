use axum::{
    http::{header, HeaderValue, Request, StatusCode},
    middleware::Next,
    response::Response,
    body::Body,
};

// Middleware to add common security headers
pub async fn secure_headers_middleware(
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();

    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::X_FRAME_OPTIONS,
        HeaderValue::from_static("DENY"),
    );
    headers.insert(
        header::X_XSS_PROTECTION,
        HeaderValue::from_str("1; mode=block").unwrap(),
    );
    // Content-Security-Policy is highly dependent on the app, so a restrictive default
    // headers.insert(
    //     header::CONTENT_SECURITY_POLICY,
    //     HeaderValue::from_str("default-src 'self'; frame-ancestors 'none'").unwrap(),
    // );
    // Strict-Transport-Security should only be added if the site is HTTPS only
    // headers.insert(
    //     header::STRICT_TRANSPORT_SECURITY,
    //     HeaderValue::from_str("max-age=31536000; includeSubDomains").unwrap(),
    // );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_str("strict-origin-when-cross-origin").unwrap(),
    );
    headers.insert(
        header::HeaderName::from_static("permissions-policy"),
        HeaderValue::from_str("geolocation=(), microphone=(), camera=()").unwrap(),
    );

    Ok(response)
} 