//! Unified error type handed around by the whole crate.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

/// Unified error type for this crate.
#[derive(Debug, Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("url parse error: {0}")]
    Url(#[from] url::ParseError),

    // Browser
    #[error("chromiumoxide error: {0}")]
    Chromiumoxide(#[from] chromiumoxide::error::CdpError),

    #[error("chromiumoxide browser config error: {0}")]
    BrowserConfig(String),

    #[error("screenshot error: {0}")]
    Screenshot(String),

    // Image
    #[error("libvips error: {0}")]
    Libvips(#[from] libvips::error::Error),

    #[error("image error: {0}")]
    Image(String),

    // Concurrency
    #[error("channel send error")]
    ChannelSend,

    #[error("oneshot receive error: {0}")]
    OneshotRecv(#[from] tokio::sync::oneshot::error::RecvError),

    #[error("task join error: {0}")]
    Join(#[from] tokio::task::JoinError),

    // Catch-all
    #[error("other error: {0}")]
    Other(String),
}

/// Implemented as a generic internal server error response to
/// avoid leaking internals. The specific error is logged.
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        tracing::error!("{self}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error. Try again later.",
        )
            .into_response()
    }
}

pub type Result<T> = std::result::Result<T, Error>;
