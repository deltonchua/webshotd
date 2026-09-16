//! libvips-backed image operations: resize and vertical chunking.

use crate::error::{Error, Result};
use libvips::{VipsApp, VipsImage, ops};
use serde::Deserialize;
use std::iter;

/// Output image format, deserialized from the lowercase value of the
/// `format` query parameter.
#[derive(Debug, Copy, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    Jpeg,
    Png,
    Webp,
}

impl ImageFormat {
    /// Filename suffix, also used by libvips to pick the encoder
    /// when writing an image to a buffer.
    pub fn suffix(&self) -> &'static str {
        match self {
            ImageFormat::Jpeg => ".jpg",
            ImageFormat::Png => ".png",
            ImageFormat::Webp => ".webp",
        }
    }
}

/// Initialize libvips, capping its internal concurrency to match
/// the server's request concurrency.
///
/// The returned `VipsApp` must be kept alive for the libvips calls to work.
pub fn init_lib(concurrency: usize) -> Result<VipsApp> {
    let app = VipsApp::new("webshotd", false)?;
    app.concurrency_set(concurrency as i32);
    Ok(app)
}

/// Resize the image to `width`, preserving the aspect ratio.
/// Upscaling and downscaling are both supported; a no-op returns
/// the input buffer unchanged.
pub fn resize(input: &[u8], width: u32, output_format: ImageFormat) -> Result<Vec<u8>> {
    let image = VipsImage::new_from_buffer(input, "")?;
    let w = image.get_width();
    let h = image.get_height();

    if w as u32 == width {
        return Ok(input.to_owned());
    }

    // libvips targets the longest side, so compute the resized
    // target width such that the *shortest* side matches `width`
    let mut width = width;
    if h > w {
        width = (h as f64 / (w as f64 / width as f64)) as u32;
    }

    let resized = ops::thumbnail_buffer(input, width as i32)?;
    Ok(resized.image_write_to_buffer(output_format.suffix())?)
}

/// Lazily split an image into vertical chunks of at most
/// `chunk_height` pixels, each re-encoded in `output_format`.
///
/// The final chunk may be shorter if `chunk_height` does not
/// divide the image height evenly.
pub fn split_by_height(
    input: &[u8],
    chunk_height: u32,
    output_format: ImageFormat,
) -> Result<impl Iterator<Item = Result<Vec<u8>>>> {
    if chunk_height == 0 {
        return Err(Error::Image("chunk height must be > 0".to_string()));
    }

    let image = VipsImage::new_from_buffer(input, "")?;
    let width = image.get_width() as u32;
    let height = image.get_height() as u32;
    if width == 0 || height == 0 {
        return Err(Error::Image(
            "image width and height must be > 0".to_string(),
        ));
    }

    let mut y = 0;
    Ok(iter::from_fn(move || {
        if y >= height {
            return None;
        }

        // Final chunk may be shorter than `chunk_height`
        let h = (height - y).min(chunk_height);
        let result = (|| {
            let chunk = ops::extract_area(&image, 0, y as i32, width as i32, h as i32)?;
            Ok(chunk.image_write_to_buffer(output_format.suffix())?)
        })();

        y += h;
        Some(result)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    static APP: OnceLock<VipsApp> = OnceLock::new();

    fn init() -> &'static VipsApp {
        APP.get_or_init(|| init_lib(10).expect("init_lib failed"))
    }

    #[test]
    fn test_resize() -> Result<()> {
        let _app = init();
        let format = ImageFormat::Webp;
        let suffix = format.suffix();

        // Size down
        // width > height
        let input = ops::black(200, 100)?;
        let input = input.image_write_to_buffer(suffix)?;

        let output = resize(&input, 50, format)?;
        let output = VipsImage::new_from_buffer(&output, "")?;

        assert_eq!(output.get_width(), 50);
        assert_eq!(output.get_height(), 25);

        // width < height
        let input = ops::black(100, 200)?;
        let input = input.image_write_to_buffer(suffix)?;

        let output = resize(&input, 50, format)?;
        let output = VipsImage::new_from_buffer(&output, "")?;

        assert_eq!(output.get_width(), 50);
        assert_eq!(output.get_height(), 100);

        // width == height
        let input = ops::black(100, 100)?;
        let input = input.image_write_to_buffer(suffix)?;

        let output = resize(&input, 50, format)?;
        let output = VipsImage::new_from_buffer(&output, "")?;

        assert_eq!(output.get_width(), 50);
        assert_eq!(output.get_height(), 50);

        // Size up
        // width > height
        let input = ops::black(50, 25)?;
        let input = input.image_write_to_buffer(suffix)?;

        let output = resize(&input, 100, format)?;
        let output = VipsImage::new_from_buffer(&output, "")?;

        assert_eq!(output.get_width(), 100);
        assert_eq!(output.get_height(), 50);

        // width < height
        let input = ops::black(25, 50)?;
        let input = input.image_write_to_buffer(suffix)?;

        let output = resize(&input, 100, format)?;
        let output = VipsImage::new_from_buffer(&output, "")?;

        assert_eq!(output.get_width(), 100);
        assert_eq!(output.get_height(), 200);

        // width == height
        let input = ops::black(50, 50)?;
        let input = input.image_write_to_buffer(suffix)?;

        let output = resize(&input, 100, format)?;
        let output = VipsImage::new_from_buffer(&output, "")?;

        assert_eq!(output.get_width(), 100);
        assert_eq!(output.get_height(), 100);

        // No-op
        // width == resize width == height
        let input = ops::black(50, 50)?;
        let input = input.image_write_to_buffer(suffix)?;

        let output = resize(&input, 50, format)?;
        let output = VipsImage::new_from_buffer(&output, "")?;

        assert_eq!(output.get_width(), 50);
        assert_eq!(output.get_height(), 50);

        // width == resize width != height
        let input = ops::black(50, 100)?;
        let input = input.image_write_to_buffer(suffix)?;

        let output = resize(&input, 50, format)?;
        let output = VipsImage::new_from_buffer(&output, "")?;

        assert_eq!(output.get_width(), 50);
        assert_eq!(output.get_height(), 100);

        Ok(())
    }

    #[test]
    fn test_split_by_height() -> Result<()> {
        let _app = init();
        let format = ImageFormat::Webp;
        let suffix = format.suffix();

        // height > chunk height
        let input = ops::black(200, 500)?;
        let input = input.image_write_to_buffer(suffix)?;

        let heights = split_by_height(&input, 200, format)?
            .map(|chunk| {
                let image = VipsImage::new_from_buffer(&chunk?, "")?;
                Ok(image.get_height())
            })
            .collect::<Result<Vec<_>>>()?;

        assert_eq!(heights, [200, 200, 100]);

        // height < chunk height
        let input = ops::black(200, 500)?;
        let input = input.image_write_to_buffer(suffix)?;

        let heights = split_by_height(&input, 600, format)?
            .map(|chunk| {
                let image = VipsImage::new_from_buffer(&chunk?, "")?;
                Ok(image.get_height())
            })
            .collect::<Result<Vec<_>>>()?;

        assert_eq!(heights, [500]);

        // height == chunk height
        let input = ops::black(200, 500)?;
        let input = input.image_write_to_buffer(suffix)?;

        let heights = split_by_height(&input, 500, format)?
            .map(|chunk| {
                let image = VipsImage::new_from_buffer(&chunk?, "")?;
                Ok(image.get_height())
            })
            .collect::<Result<Vec<_>>>()?;

        assert_eq!(heights, [500]);

        Ok(())
    }
}
