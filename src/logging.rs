//! Logging setup for the crate.

use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

/// Initialize the tracing subscriber.
///
/// Defaults to `info` for this crate and `tower_http`; override via `RUST_LOG`.
pub fn enable() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| format!("{}=info,tower_http=info", env!("CARGO_CRATE_NAME")).into());

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}
