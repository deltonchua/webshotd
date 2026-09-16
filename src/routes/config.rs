//! Shared state extracted by the request handlers.

use crate::browser::ScreenshotRequest;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::{Semaphore, mpsc::Sender};

/// Server state shared by all request handlers, implemented as
/// axum state and re-cloned into each handler via `Clone`.
#[derive(Debug, Clone)]
pub struct Config {
    // Simple limits, mirroring the CLI options
    pub concurrency: usize,
    pub timeout_ms: u32,
    pub max_frames: u32,

    /// Directory for generated images
    pub screenshot_dir: PathBuf,

    // Shared handles
    /// Permits bounding concurrent screenshot requests
    pub semaphore: Arc<Semaphore>,

    /// Sender for the browser worker's request channel
    pub browser_tx: Sender<ScreenshotRequest>,
}
