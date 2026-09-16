//! Manual integration test, exercising a real browser:
//!
//! ```sh
//! cargo test --test browser -- --ignored
//! ```
//!
//! Requires the paths to a chromium binary and a profile directory,
//! provided via env vars (see below). Must be run manually, outside
//! of CI.
//!
//! Env vars:
//! - `WEBSHOTD_TEST_CHROMIUM`: path to the chromium binary
//! - `WEBSHOTD_TEST_PROFILE`: path to the chromium profile directory

use std::{env, path::PathBuf};
use tokio::{fs, sync::oneshot};
use tokio_util::sync::CancellationToken;
use url::Url;
use webshotd::{
    browser::{self, ScreenshotRequest},
    error::{Error, Result},
    image::ImageFormat,
};

fn env_path(name: &str) -> PathBuf {
    env::var(name)
        .unwrap_or_else(|_| panic!("missing env var: {name}"))
        .into()
}

/// Capture a sample page and write the raw screenshot to
/// `/tmp/sample.webp`.
#[tokio::test]
#[ignore]
async fn test_screenshot_sample() -> Result<()> {
    let chromium_path = env_path("WEBSHOTD_TEST_CHROMIUM");
    let user_data_dir = env_path("WEBSHOTD_TEST_PROFILE");

    let (mtx, mrx) = tokio::sync::mpsc::channel(10);
    let token = CancellationToken::new();
    let browser_handle = browser::launch(
        chromium_path,
        user_data_dir,
        // headless, incognito
        false,
        true,
        mrx,
        token.clone(),
    )
    .await?;

    let request_handle = tokio::spawn(async move {
        let (otx, orx) = oneshot::channel();
        mtx.send(ScreenshotRequest {
            url: Url::parse("https://www.cnbc.com")?,
            width: 1080,
            height: 1080,
            frames: 10,
            format: ImageFormat::Webp,
            timeout_ms: 10_000,
            tx: otx,
        })
        .await
        .map_err(|_| Error::ChannelSend)?;

        let image = orx.await?;
        let out = std::env::temp_dir().join("sample.webp");
        fs::write(out, image).await?;

        Ok::<_, Error>(())
    });

    request_handle.await??;
    token.cancel();
    browser_handle.await?;
    Ok(())
}
