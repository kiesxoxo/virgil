//! Edits copies of real files: `HIFI_TAG_SAMPLES=a.flac:b.mp3 cargo test -p hifi-tags -- --ignored`.

use std::path::{Path, PathBuf};

use hifi_core::{TagEdit, TagField};
use lofty::file::TaggedFileExt;

fn field(name: &str, values: &[&str]) -> TagField {
    TagField { name: name.into(), values: values.iter().map(|v| v.to_string()).collect() }
}

fn pictures(path: &Path) -> usize {
    let file = lofty::read_from_path(path).unwrap();
    file.tags().iter().map(|tag| tag.pictures().len()).sum()
}

/// The last 4 MiB: audio, which tag edits must never touch.
fn tail(path: &Path) -> Vec<u8> {
    let data = std::fs::read(path).unwrap();
    data[data.len().saturating_sub(4 << 20)..].to_vec()
}

#[test]
#[ignore]
fn edits_real_files_without_touching_audio_or_art() {
    let samples = std::env::var("HIFI_TAG_SAMPLES").expect("HIFI_TAG_SAMPLES=path[:path…]");
    let dir = std::env::temp_dir().join(format!("hifi-tags-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for sample in samples.split(':') {
        let copy: PathBuf = dir.join(Path::new(sample).file_name().unwrap());
        std::fs::copy(sample, &copy).unwrap();
        let (before, info) = hifi_tags::read(&copy).unwrap();
        let (art, audio) = (pictures(&copy), tail(&copy));
        println!("{sample}: {} {info:?}\n  {before:?}\n  {art} picture(s)", info.codec);

        let edit = TagEdit {
            uri: String::new(),
            set: vec![
                field("TITLE", &["Edited title ünïcode"]),
                field("MY CUSTOM FIELD", &["one", "two"]),
                // Zero-padded on purpose: values must come back exactly as written.
                field("TRACKNUMBER", &["07"]),
                field("TOTALTRACKS", &["9"]),
            ],
            remove: vec!["COMMENT".into()],
        };
        assert!(hifi_tags::write(&copy, &edit).unwrap());
        let (after, info_after) = hifi_tags::read(&copy).unwrap();
        println!("  after: {after:?}");
        assert_eq!(after, hifi_tags::apply(before.clone(), &edit), "fields as edited");
        assert_eq!(pictures(&copy), art, "embedded pictures kept");
        assert_eq!(tail(&copy), audio, "audio untouched");
        assert_eq!(info_after.duration, info.duration);
        assert!(!hifi_tags::write(&copy, &edit).unwrap(), "same edit again changes nothing");
        assert!(
            std::fs::read_dir(&dir).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().ends_with(".hifi-tags"))
        );

        // And back.
        let undo =
            TagEdit { uri: String::new(), set: before.clone(), remove: after.iter().map(|f| f.name.clone()).collect() };
        hifi_tags::write(&copy, &undo).unwrap();
        let (restored, _) = hifi_tags::read(&copy).unwrap();
        let sorted = |mut f: Vec<TagField>| {
            f.sort_by(|a, b| a.name.cmp(&b.name));
            f
        };
        assert_eq!(sorted(restored), sorted(before));
    }
    std::fs::remove_dir_all(&dir).unwrap();
}
