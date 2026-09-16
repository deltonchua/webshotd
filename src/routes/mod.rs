//! HTTP routes, built as a single axum router with shared middleware.

pub mod config;
mod screenshot;
mod status;

use axum::{Router, http::StatusCode};
use config::Config;
use std::time::Duration;
use tower::ServiceBuilder;
use tower_http::{timeout::TimeoutLayer, trace::TraceLayer};

/// Assemble the full router: status and screenshot endpoints nested
/// under `/v1`, wrapped with tracing and request-timeout middleware.
pub fn build(config: Config) -> Router {
    let routes = Router::new()
        .merge(status::route())
        .merge(screenshot::route());

    // Applies to all routes built above
    let layers = ServiceBuilder::new()
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_millis(config.timeout_ms as u64),
        ));

    Router::new()
        .nest("/v1", routes)
        .route_layer(layers)
        .with_state(config)
}
