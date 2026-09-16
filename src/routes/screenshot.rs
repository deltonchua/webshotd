//! Screenshot endpoint: the primary API.

use crate::{
    browser::ScreenshotRequest,
    error::Error,
    image::{self, ImageFormat},
    routes::config::Config,
};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use serde::Deserialize;
use serde_json::json;
use tokio::{fs, sync::oneshot, task::JoinSet};
use url::Url;

/// Query parameters for `GET /v1/screenshot`.
#[derive(Debug, Deserialize)]
struct Params {
    url: Url,
    /// Viewport width (px)
    vw: u32,
    /// Viewport height (px)
    vh: u32,
    /// Output width (px)
    ow: u32,
    /// Maximum number of vertical frames to slice the page into
    frames: u32,
    /// Output image format
    format: ImageFormat,
    /// Per-request timeout in milliseconds
    timeout_ms: u32,
}

/// Mount the screenshot endpoint on the router.
pub fn route() -> Router<Config> {
    Router::new().route("/screenshot", get(screenshot))
}

/// Capture a full screenshot of `url`, resize it to `ow`, slice it
/// into `frames` vertical chunks, write each chunk under
/// `screenshot_dir/<host>/`, and respond with the resulting paths.
async fn screenshot(
    State(config): State<Config>,
    Query(params): Query<Params>,
) -> impl IntoResponse {
    // Reject when fully saturated rather than queueing
    if config.semaphore.try_acquire().is_err() {
        return (StatusCode::SERVICE_UNAVAILABLE, "Server Busy").into_response();
    }

    // Validation
    if params.vw == 0 || params.vh == 0 || params.ow == 0 || params.frames == 0 {
        return (StatusCode::BAD_REQUEST, "Invalid Image Dimension").into_response();
    }
    if params.timeout_ms == 0 {
        return (StatusCode::BAD_REQUEST, "Invalid Timeout").into_response();
    }

    let result = async {
        let out_dir = config
            .screenshot_dir
            .join(params.url.host_str().unwrap_or_else(|| "host"));
        fs::create_dir_all(&out_dir).await?;

        // Delegate the screenshot to the browser worker
        let (tx, rx) = oneshot::channel();
        config
            .browser_tx
            .send(ScreenshotRequest {
                url: params.url,
                width: params.vw,
                height: params.vh,
                frames: params.frames.min(config.max_frames),
                format: params.format,
                timeout_ms: params.timeout_ms.min(config.timeout_ms),
                tx,
            })
            .await
            .map_err(|_| Error::ChannelSend)?;

        // Resize and split the raw screenshot into frames
        let raw = rx.await?;
        let resized = image::resize(&raw, params.ow, params.format)?;
        let oh = (params.ow as f64 / params.vw as f64) * params.vh as f64;
        let chunks = image::split_by_height(&resized, oh as u32, params.format)?;

        // Write each frame to disk
        let mut set = JoinSet::new();
        for (i, chunk) in chunks.enumerate() {
            let out = out_dir.join(format!("frame_{}{}", i, params.format.suffix()));
            set.spawn(async move {
                fs::write(&out, chunk?).await?;
                Ok::<_, Error>(out.to_string_lossy().into_owned())
            });
        }

        let mut paths: Vec<String> = set
            .join_all()
            .await
            .into_iter()
            .filter_map(|res| res.ok())
            .collect();
        paths.sort();

        Ok::<_, Error>(paths)
    }
    .await;
    match result {
        Ok(paths) => Json(json!({"frames": paths})).into_response(),
        Err(err) => err.into_response(),
    }
}
