//! Browser worker: owns the single Chromium instance and processes
//! screenshot requests received over an mpsc channel.

use crate::{
    error::{Error, Result},
    image::ImageFormat,
};
use chromiumoxide::{
    Browser, BrowserConfig, Page,
    cdp::browser_protocol::{
        emulation::SetDeviceMetricsOverrideParams, page::CaptureScreenshotFormat,
    },
    page::ScreenshotParams,
};
use futures::StreamExt;
use std::{path::Path, time::Duration};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
    time,
};
use tokio_util::sync::CancellationToken;
use url::Url;

/// A request to capture a screenshot of a page. Capture parameters
/// first, reply channel last.
#[derive(Debug)]
pub struct ScreenshotRequest {
    pub url: Url,
    pub width: u32,
    pub height: u32,
    pub frames: u32,
    pub format: ImageFormat,
    pub timeout_ms: u32,
    pub tx: oneshot::Sender<Vec<u8>>,
}

impl From<ImageFormat> for CaptureScreenshotFormat {
    /// Maps to the corresponding CDP screenshot format enum.
    fn from(format: ImageFormat) -> Self {
        match format {
            ImageFormat::Jpeg => CaptureScreenshotFormat::Jpeg,
            ImageFormat::Png => CaptureScreenshotFormat::Png,
            ImageFormat::Webp => CaptureScreenshotFormat::Webp,
        }
    }
}

/// Launch the browser and spawn its worker task.
///
/// The worker polls the request channel until the token is cancelled,
/// in which case the browser is closed and the join handle resolves.
pub async fn launch(
    chromium_path: impl AsRef<Path>,
    user_data_dir: impl AsRef<Path>,
    headless: bool,
    incognito: bool,
    mut rx: mpsc::Receiver<ScreenshotRequest>,
    token: CancellationToken,
) -> Result<JoinHandle<()>> {
    let browser_config = {
        let mut builder = BrowserConfig::builder()
            .chrome_executable(chromium_path)
            .user_data_dir(user_data_dir);
        if headless {
            builder = builder.new_headless_mode();
        } else {
            builder = builder.with_head();
        }
        builder.build().map_err(Error::BrowserConfig)?
    };
    let (mut browser, mut handler) = Browser::launch(browser_config).await?;

    let handle = tokio::spawn(async move {
        while let Some(result) = handler.next().await {
            if let Err(err) = result {
                tracing::error!(%err, "chromiumoxide handler error");
                break;
            }
        }
    });

    if incognito {
        if let Err(err) = browser.start_incognito_context().await {
            tracing::error!(%err, "failed to start incognito context");
        }
    }

    let handle: JoinHandle<()> = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    if let Err(err) = async {
                        browser.close().await?;
                        handle.await?;
                        Ok::<_, Error>(())
                    }
                    .await
                    {
                        tracing::error!(%err, "browser shutdown error");
                    }
                    break;
                }

                Some(request) = rx.recv() => {
                    match browser.new_page("about:blank").await {
                        Ok(page) => {
                            // Non-blocking
                            tokio::spawn(async move {
                                match time::timeout(
                                    Duration::from_millis(request.timeout_ms as u64),
                                    screenshot(&page, request),
                                )
                                .await
                                {
                                    Ok(Err(err)) => tracing::error!(%err, "browser screenshot error"),
                                    Err(err) => tracing::error!(%err, "browser screenshot timed out"),
                                    _ => (),
                                }
                                if let Err(err) = page.close().await {
                                    tracing::error!(%err, "page not closed");
                                }
                            });
                        }
                        Err(err) => tracing::error!(%err, "browser new page error")
                    }
                }

                // Channel closed and token not yet cancelled;
                // treat as shutdown
                else => {
                    break;
                }
            }
        }
    });

    Ok(handle)
}

/// Capture a screenshot of a page: navigate, expand the viewport to
/// cover the requested number of frames, capture, and send the raw
/// image bytes back over the request's reply channel.
async fn screenshot(page: &Page, request: ScreenshotRequest) -> Result<()> {
    // Set initial viewport
    page.execute(SetDeviceMetricsOverrideParams::new(
        request.width as i64,
        request.height as i64,
        1,
        false,
    ))
    .await?;

    // Navigate to page
    page.goto(request.url).await?;

    // Calculate actual page height
    let page_height = page
        .evaluate("document.documentElement.scrollHeight")
        .await?
        .value()
        .and_then(|v| v.as_i64())
        .ok_or_else(|| Error::Screenshot("failed to get document height".to_string()))?
        as u32;
    if page_height == 0 {
        return Err(Error::Screenshot("page height is 0".to_string()));
    }

    // Calculate desired height
    // Expanding the viewport once to full desired height is
    // preferred over stitching multiple viewport-sized captures.
    let max_height = request.height * request.frames;
    let desired_frames = if page_height < max_height {
        page_height.div_ceil(request.height)
    } else {
        request.frames
    };
    let desired_height = desired_frames * request.height;

    // Set desired viewport
    page.execute(SetDeviceMetricsOverrideParams::new(
        request.width as i64,
        desired_height as i64,
        1,
        false,
    ))
    .await?;

    // Take screenshot
    let image = page
        .screenshot(ScreenshotParams::builder().format(request.format).build())
        .await?;
    request.tx.send(image).map_err(|_| Error::ChannelSend)?;

    Ok(())
}
