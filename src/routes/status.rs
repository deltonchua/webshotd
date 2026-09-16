//! Health-check endpoint.

use crate::routes::config::Config;
use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};

/// Mount the status endpoint on the router.
pub fn route() -> Router<Config> {
    Router::new().route("/status", get(status))
}

/// Health check: always returns `200 OK` with a plain-text body.
async fn status() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}
