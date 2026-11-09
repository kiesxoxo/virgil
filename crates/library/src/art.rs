//! Cover thumbnails, rendered once at scan time so the album grid never
//! waits on a full-size JPEG decode.

use std::path::Path;

use image::DynamicImage;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;

use crate::group::fnv1a64;

/// Edge lengths in pixels, smallest first: grid tiles (and lists), the same on Retina screens, and
/// the album page and Now Playing on a phone's 3x screen.
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
    if complete(dir, &key) {
        return Ok(key);
    }
    // Largest first, each size from the one before: only the first has a whole scan to go through.
    let mut image = DynamicImage::ImageRgb8(image::load_from_memory(source)?.into_rgb8());
    for &size in SIZES.iter().rev() {
        if image.width() > size || image.height() > size {
            image = image.resize(size, size, FilterType::CatmullRom);
        }
        let mut jpeg = Vec::new();
        image.write_with_encoder(JpegEncoder::new_with_quality(&mut jpeg, 85))?;
        let target = dir.join(format!("{key}-{size}.jpg"));
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

    #[test]
    fn scales_down_to_each_size_but_never_up() {
        let dir = std::env::temp_dir().join(format!("virgil-art-sizes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut png = Vec::new();
        DynamicImage::new_rgba8(800, 400)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();

        let key = make_thumbs(&dir, &png).unwrap();
        let size = |edge: u32| image::image_dimensions(dir.join(format!("{key}-{edge}.jpg"))).unwrap();
        assert_eq!(size(1000), (800, 400));
        assert_eq!(size(600), (600, 300));
        assert_eq!(size(300), (300, 150));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
