//! Turns whatever the user uploaded into a small, safe, uniform JPEG.
//!
//! Decoding and re-encoding is the security boundary: the stored file is always one we produced,
//! so metadata (EXIF, GPS), embedded scripts and polyglot tricks in the original never reach
//! other users.

use std::io::Cursor;

use image::{
    ImageFormat, ImageReader, Limits, RgbImage, codecs::jpeg::JpegEncoder, imageops::FilterType,
};

use super::error::AvatarError;

pub const AVATAR_SIZE: u32 = 256;
const JPEG_QUALITY: u8 = 85;
const MAX_DIMENSION: u32 = 8192;
const MAX_DECODE_BYTES: u64 = 128 * 1024 * 1024;

/// Decodes, centre-crops to a square, resizes to 256x256, flattens transparency onto white and
/// re-encodes as JPEG. CPU heavy: call from a blocking thread.
pub fn to_avatar_jpeg(original: &[u8]) -> Result<Vec<u8>, AvatarError> {
    let mut reader = ImageReader::new(Cursor::new(original))
        .with_guessed_format()
        .map_err(|_| AvatarError::InvalidImage)?;

    match reader.format() {
        Some(ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP | ImageFormat::Gif) => {}
        _ => return Err(AvatarError::InvalidImage),
    }

    // Refuse decompression bombs (a tiny file that expands to gigabytes of pixels).
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);

    let decoded = reader.decode().map_err(|error| match error {
        image::ImageError::Limits(_) => AvatarError::TooLarge(2),
        _ => AvatarError::InvalidImage,
    })?;

    let square = decoded
        .resize_to_fill(AVATAR_SIZE, AVATAR_SIZE, FilterType::Lanczos3)
        .to_rgba8();

    let mut flat = RgbImage::new(AVATAR_SIZE, AVATAR_SIZE);
    for (x, y, pixel) in square.enumerate_pixels() {
        let alpha = u32::from(pixel[3]);
        let over_white =
            |channel: u8| ((u32::from(channel) * alpha + 255 * (255 - alpha)) / 255) as u8;
        flat.put_pixel(
            x,
            y,
            image::Rgb([
                over_white(pixel[0]),
                over_white(pixel[1]),
                over_white(pixel[2]),
            ]),
        );
    }

    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY)
        .encode_image(&flat)
        .map_err(|_| AvatarError::InvalidImage)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use image::{DynamicImage, Rgba, RgbaImage};

    use super::*;

    fn png(width: u32, height: u32, color: Rgba<u8>) -> Vec<u8> {
        let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(width, height, color));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn produces_a_square_jpeg() {
        let out = to_avatar_jpeg(&png(600, 200, Rgba([200, 30, 30, 255]))).unwrap();
        let decoded = image::load_from_memory_with_format(&out, ImageFormat::Jpeg).unwrap();
        assert_eq!(
            (decoded.width(), decoded.height()),
            (AVATAR_SIZE, AVATAR_SIZE)
        );
    }

    #[test]
    fn transparent_pixels_become_white() {
        let out = to_avatar_jpeg(&png(64, 64, Rgba([0, 0, 0, 0]))).unwrap();
        let decoded = image::load_from_memory(&out).unwrap().to_rgb8();
        let p = decoded.get_pixel(128, 128);
        assert!(p.0.iter().all(|c| *c > 245), "expected white, got {p:?}");
    }

    #[test]
    fn rejects_non_images() {
        assert!(matches!(
            to_avatar_jpeg(b"<script>alert(1)</script>"),
            Err(AvatarError::InvalidImage)
        ));
        assert!(matches!(
            to_avatar_jpeg(b""),
            Err(AvatarError::InvalidImage)
        ));
    }
}
