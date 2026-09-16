//! Manual integration test, exercising libvips on a real-world image:
//!
//! ```sh
//! cargo test --test image -- --ignored
//! ```
//!
//! Requires the path to an input image, provided via an env var (see
//! below). Must be run manually, outside of CI.
//!
//! Env vars:
//! - `WEBSHOTD_TEST_INPUT`: path to the input image
//!
//! The resize and split unit tests live in `src/image.rs`; unlike
//! those, this test operates on a real-world page capture.

use std::env;
use webshotd::error::Result;
use webshotd::image::{self, ImageFormat};

#[test]
#[ignore]
fn test_resize_and_split_sample() -> Result<()> {
    let input_path = env::var("WEBSHOTD_TEST_INPUT")
        .unwrap_or_else(|_| panic!("missing env var: WEBSHOTD_TEST_INPUT"));
    let format = ImageFormat::Webp;
    let suffix = format.suffix();
    let _app = image::init_lib(10).expect("init_lib failed");

    let input = std::fs::read(input_path)?;
    let input = image::resize(&input, 600, format)?;

    for (i, chunk) in image::split_by_height(&input, 600, format)?.enumerate() {
        let output = env::temp_dir().join(format!("sample_{i}{suffix}"));
        std::fs::write(output, chunk?)?;
    }

    Ok(())
}
