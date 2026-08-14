use axum::{
    body::Body,
    http::{HeaderName, HeaderValue, Request},
    middleware::Next,
    response::Response,
};

const X_ROBOTS_TAG: HeaderName = HeaderName::from_static("x-robots-tag");

/// Keeps executable frontend artifacts available to crawlers for rendering
/// without allowing those artifacts to become standalone search results.
pub async fn add_search_engine_headers(request: Request<Body>, next: Next) -> Response {
    let should_noindex = should_noindex_path(request.uri().path());
    let mut response = next.run(request).await;

    if should_noindex {
        response
            .headers_mut()
            .insert(X_ROBOTS_TAG, HeaderValue::from_static("noindex, nofollow"));
    }

    response
}

pub fn should_noindex_path(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|filename| filename.ends_with(".wasm"))
}
