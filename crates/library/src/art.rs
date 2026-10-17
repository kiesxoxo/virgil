//! Cover thumbnails, rendered once at scan time so the album grid never
//! waits on a full-size JPEG decode.

use std::path::Path;

use image::DynamicImage;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;

use crate::group::fnv1a64;

/// Edge lengths in pixels: grid tiles and the album page (2x for HiDPI).
pub const SIZES: [u32; 2] = [300, 600];

/// Writes `{key}-{size}.jpg` for each size and returns the key, a hash of the
/// source bytes, so identical covers are stored once and URLs never go stale.
pub fn make_thumbs(dir: &Path, source: &[u8]) -> anyhow::Result<String> {
    let key = format!("{:016x}", fnv1a64(source));
    let path = |size: u32| dir.join(format!("{key}-{size}.jpg"));
    if SIZES.iter().all(|&size| path(size).exists()) {
        return Ok(key);
    }
    let image = image::load_from_memory(source)?;
    for size in SIZES {
        let scaled = if image.width() > size || image.height() > size {
            image.resize(size, size, FilterType::CatmullRom)
        } else {
            image.clone()
        };
        let mut jpeg = Vec::new();
        DynamicImage::ImageRgb8(scaled.to_rgb8()).write_with_encoder(JpegEncoder::new_with_quality(&mut jpeg, 85))?;
        let target = path(size);
        let tmp = target.with_extension("tmp");
        std::fs::write(&tmp, &jpeg)?;
        std::fs::rename(&tmp, &target)?;
    }
    Ok(key)
}
