//! Cover thumbnails, rendered once at scan time so the album grid never
//! waits on a full-size JPEG decode.

use std::path::Path;

use image::DynamicImage;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;

use crate::group::fnv1a64;

/// Edge lengths in pixels: grid tiles (and lists), the same on Retina screens, and the album page
/// and the cover flow's middle cover on a phone's 3x screen.
pub const SIZES: [u32; 3] = [300, 600, 1000];

/// The key a cover's thumbnails are stored under: a hash of its bytes.
pub fn key(source: &[u8]) -> String {
    format!("{:016x}", fnv1a64(source))
}

/// Whether every size of `key` is there (covers rendered before a size was added aren't).
pub fn complete(dir: &Path, key: &str) -> bool {
    SIZES.iter().all(|&size| dir.join(format!("{key}-{size}.jpg")).exists())
}

/// Writes `{key}-{size}.jpg` for each size and returns the key, a hash of the
/// source bytes, so identical covers are stored once and URLs never go stale.
pub fn make_thumbs(dir: &Path, source: &[u8]) -> anyhow::Result<String> {
    let key = key(source);
    let path = |size: u32| dir.join(format!("{key}-{size}.jpg"));
    if complete(dir, &key) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_missing_sizes_under_the_same_key() {
        let dir = std::env::temp_dir().join(format!("virgil-art-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut png = Vec::new();
        DynamicImage::new_rgb8(1200, 1200)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();

        let key = make_thumbs(&dir, &png).unwrap();
        assert_eq!(key, super::key(&png));
        assert!(complete(&dir, &key));

        // A size added since the cover was first rendered.
        let large = dir.join(format!("{key}-1000.jpg"));
        std::fs::remove_file(&large).unwrap();
        assert!(!complete(&dir, &key));
        assert_eq!(make_thumbs(&dir, &png).unwrap(), key);
        assert_eq!(image::image_dimensions(&large).unwrap(), (1000, 1000));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
