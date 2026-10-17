//! Reading and writing tags the way foobar2000's Properties dialog does:
//! every text field by name, custom ones included, on each format's own tag
//! (Vorbis comments, APEv2, ID3v2, iTunes metadata).
//!
//! Files are edited through lofty's concrete file types rather than its
//! generic `Tag`, which has no room for custom fields and would drop frames
//! hifi doesn't show. Embedded pictures and everything else not listed as a
//! field are left as they are.

mod id3;
mod mp4;

use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use hifi_core::{FileInfo, TagEdit, TagField};
use lofty::aac::AacFile;
use lofty::ape::{ApeFile, ApeItem, ApeTag};
use lofty::config::{ParseOptions, WriteOptions};
use lofty::file::{AudioFile, FileType};
use lofty::flac::FlacFile;
use lofty::iff::aiff::AiffFile;
use lofty::iff::wav::WavFile;
use lofty::mp4::Mp4File;
use lofty::mpeg::MpegFile;
use lofty::musepack::MpcFile;
use lofty::ogg::tag::VorbisComments;
use lofty::ogg::{OpusFile, SpeexFile, VorbisFile};
use lofty::properties::FileProperties;
use lofty::tag::ItemValue;
use lofty::wavpack::WavPackFile;

/// A format's own tag, seen as a list of named fields.
trait Native: Default {
    /// Every text field, grouped by name (case-insensitively) in file order.
    fn fields(&self) -> Vec<TagField>;
    /// Replaces everything [`Native::fields`] reads with `fields`, leaving the rest.
    fn replace(&mut self, fields: &[TagField]) -> anyhow::Result<()>;
}

/// Adds a value to `fields`, under an existing field of the same name if there is one.
fn push(fields: &mut Vec<TagField>, name: &str, values: impl IntoIterator<Item = impl Into<String>>) {
    let values = values.into_iter().map(Into::into).filter(|v: &String| !v.is_empty());
    match fields.iter_mut().find(|f| f.name.eq_ignore_ascii_case(name)) {
        Some(field) => field.values.extend(values),
        None => {
            let values: Vec<String> = values.collect();
            if !values.is_empty() {
                fields.push(TagField { name: name.to_string(), values });
            }
        }
    }
}

/// Vorbis keys that hold pictures, not text.
fn is_picture_key(key: &str) -> bool {
    ["METADATA_BLOCK_PICTURE", "COVERART", "COVERARTMIME"].iter().any(|k| k.eq_ignore_ascii_case(key))
}

impl Native for VorbisComments {
    fn fields(&self) -> Vec<TagField> {
        let mut fields = Vec::new();
        for (key, value) in self.items().filter(|(key, _)| !is_picture_key(key)) {
            push(&mut fields, key, [value]);
        }
        fields
    }

    fn replace(&mut self, fields: &[TagField]) -> anyhow::Result<()> {
        let kept: Vec<_> = self.take_items().filter(|(key, _)| is_picture_key(key)).collect();
        for (key, value) in kept {
            self.push(key, value);
        }
        for field in fields {
            for value in &field.values {
                self.push(field.name.clone(), value.clone());
            }
        }
        Ok(())
    }
}

impl Native for ApeTag {
    fn fields(&self) -> Vec<TagField> {
        let mut fields = Vec::new();
        for item in self {
            if let ItemValue::Text(text) = item.value() {
                push(&mut fields, item.key(), text.split('\0'));
            }
        }
        fields
    }

    fn replace(&mut self, fields: &[TagField]) -> anyhow::Result<()> {
        let text: Vec<String> = (&*self)
            .into_iter()
            .filter(|item| matches!(item.value(), ItemValue::Text(_)))
            .map(|i| i.key().to_string())
            .collect();
        for key in text {
            self.remove(&key);
        }
        for field in fields {
            let item = ApeItem::new(field.name.clone(), ItemValue::Text(field.values.join("\0")))
                .with_context(|| format!("{} can't be stored in an APE tag", field.name))?;
            self.insert(item);
        }
        Ok(())
    }
}

/// Defines [`Loaded`] over lofty's file types: `Variant(FileType) => codec, "tag type"`.
macro_rules! files {
    ($($variant:ident($ty:ty) => $codec:literal, $tag_type:literal;)*) => {
        enum Loaded {
            $($variant($ty),)*
        }

        impl Loaded {
            fn read(kind: FileType, file: &mut File, options: ParseOptions) -> anyhow::Result<Self> {
                Ok(match kind {
                    $(FileType::$variant => Self::$variant(<$ty>::read_from(file, options)?),)*
                    other => bail!("{other:?} files have no tag support"),
                })
            }

            fn save(&self, file: &mut File) -> anyhow::Result<()> {
                match self {
                    $(Self::$variant(f) => f.save_to(file, WriteOptions::default())?,)*
                }
                Ok(())
            }

            fn properties(&self) -> FileProperties {
                match self {
                    $(Self::$variant(f) => f.properties().clone().into(),)*
                }
            }

            fn names(&self) -> (&'static str, &'static str) {
                match self {
                    $(Self::$variant(_) => ($codec, $tag_type),)*
                }
            }
        }
    };
}

files! {
    Flac(FlacFile) => "FLAC", "Vorbis comments";
    Opus(OpusFile) => "Opus", "Vorbis comments";
    Vorbis(VorbisFile) => "Vorbis", "Vorbis comments";
    Speex(SpeexFile) => "Speex", "Vorbis comments";
    Ape(ApeFile) => "Monkey's Audio", "APEv2";
    WavPack(WavPackFile) => "WavPack", "APEv2";
    Mpc(MpcFile) => "Musepack", "APEv2";
    Mpeg(MpegFile) => "MP3", "ID3v2";
    Aac(AacFile) => "AAC", "ID3v2";
    Aiff(AiffFile) => "AIFF", "ID3v2";
    Wav(WavFile) => "WAV", "ID3v2";
    Mp4(Mp4File) => "MP4", "iTunes metadata";
}

/// The tag in an optional slot, created when missing.
macro_rules! slot {
    ($file:expr, $get_mut:ident, $set:ident) => {{
        if $file.$get_mut().is_none() {
            $file.$set(Default::default());
        }
        $file.$get_mut().expect("just created")
    }};
}

impl Loaded {
    fn fields(&self) -> Vec<TagField> {
        match self {
            Self::Flac(f) => f.vorbis_comments().map(Native::fields),
            Self::Opus(f) => Some(f.vorbis_comments().fields()),
            Self::Vorbis(f) => Some(f.vorbis_comments().fields()),
            Self::Speex(f) => Some(f.vorbis_comments().fields()),
            Self::Ape(f) => f.ape().map(Native::fields),
            Self::WavPack(f) => f.ape().map(Native::fields),
            Self::Mpc(f) => f.ape().map(Native::fields),
            Self::Mpeg(f) => f.id3v2().map(Native::fields),
            Self::Aac(f) => f.id3v2().map(Native::fields),
            Self::Aiff(f) => f.id3v2().map(Native::fields),
            Self::Wav(f) => f.id3v2().map(Native::fields),
            Self::Mp4(f) => f.ilst().map(Native::fields),
        }
        .unwrap_or_default()
    }

    fn replace(&mut self, fields: &[TagField]) -> anyhow::Result<()> {
        match self {
            Self::Flac(f) => slot!(f, vorbis_comments_mut, set_vorbis_comments).replace(fields),
            Self::Opus(f) => f.vorbis_comments_mut().replace(fields),
            Self::Vorbis(f) => f.vorbis_comments_mut().replace(fields),
            Self::Speex(f) => f.vorbis_comments_mut().replace(fields),
            Self::Ape(f) => slot!(f, ape_mut, set_ape).replace(fields),
            Self::WavPack(f) => slot!(f, ape_mut, set_ape).replace(fields),
            Self::Mpc(f) => slot!(f, ape_mut, set_ape).replace(fields),
            Self::Mpeg(f) => slot!(f, id3v2_mut, set_id3v2).replace(fields),
            Self::Aac(f) => slot!(f, id3v2_mut, set_id3v2).replace(fields),
            Self::Aiff(f) => slot!(f, id3v2_mut, set_id3v2).replace(fields),
            Self::Wav(f) => slot!(f, id3v2_mut, set_id3v2).replace(fields),
            Self::Mp4(f) => slot!(f, ilst_mut, set_ilst).replace(fields),
        }
    }

    fn codec(&self) -> String {
        match self {
            Self::Mp4(f) => match f.properties().codec() {
                Some(lofty::mp4::Mp4Codec::AAC) => "AAC".into(),
                Some(lofty::mp4::Mp4Codec::ALAC) => "ALAC".into(),
                _ => "MP4".into(),
            },
            other => other.names().0.into(),
        }
    }
}

/// Values exactly as stored: lofty would otherwise tidy some up on read
/// (TRACKNUMBER "01" → "1"), and the editor shows what's in the file.
fn options() -> ParseOptions {
    ParseOptions::new().implicit_conversions(false)
}

fn open(path: &Path, write: bool, options: ParseOptions) -> anyhow::Result<(File, Loaded)> {
    let kind =
        lofty::probe::Probe::open(path)?.guess_file_type()?.file_type().context("not an audio format hifi can tag")?;
    let mut file = OpenOptions::new().read(true).write(write).open(path)?;
    let loaded = Loaded::read(kind, &mut file, options)?;
    Ok((file, loaded))
}

/// A file's fields and technical details.
pub fn read(path: &Path) -> anyhow::Result<(Vec<TagField>, FileInfo)> {
    let (file, loaded) = open(path, false, options().read_cover_art(false))?;
    let properties = loaded.properties();
    let info = FileInfo {
        codec: loaded.codec(),
        tag_type: loaded.names().1.into(),
        sample_rate: properties.sample_rate(),
        bits: properties.bit_depth(),
        channels: properties.channels(),
        duration: properties.duration().as_secs_f64(),
        bitrate: properties.audio_bitrate().or(properties.overall_bitrate()),
        size: file.metadata()?.len(),
    };
    Ok((loaded.fields(), info))
}

/// `fields` with `edit` applied. Empty values are dropped, and a field left
/// with no values is removed, as in foobar2000.
pub fn apply(mut fields: Vec<TagField>, edit: &TagEdit) -> Vec<TagField> {
    fields.retain(|f| !edit.remove.iter().any(|name| name.eq_ignore_ascii_case(&f.name)));
    for set in &edit.set {
        let values: Vec<String> = set.values.iter().filter(|v| !v.is_empty()).cloned().collect();
        match fields.iter().position(|f| f.name.eq_ignore_ascii_case(&set.name)) {
            Some(i) if values.is_empty() => {
                fields.remove(i);
            }
            Some(i) => fields[i].values = values,
            None if !values.is_empty() => fields.push(TagField { name: set.name.clone(), values }),
            None => {}
        }
    }
    fields
}

/// Field names every format can store: printable ASCII without '=' (Vorbis' rule).
pub fn check_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty() || name.len() > 255 || !name.bytes().all(|b| (0x20..=0x7D).contains(&b) && b != b'=') {
        bail!("\"{name}\" isn't a valid field name (use letters, digits, spaces and punctuation other than '=')");
    }
    Ok(())
}

/// Applies `edit` to the file at `path`. The file is rewritten as a copy that
/// replaces the original only once it is complete, so a failure (or a full
/// disk) never leaves a half-written file. Returns false when nothing changed.
pub fn write(path: &Path, edit: &TagEdit) -> anyhow::Result<bool> {
    for field in &edit.set {
        check_name(&field.name)?;
    }
    let (current, _) = read(path)?;
    let next = apply(current.clone(), edit);
    if next == current {
        return Ok(false);
    }
    let name = path.file_name().context("not a file")?.to_string_lossy();
    let staging: PathBuf = path.with_file_name(format!(".{name}.hifi-tags"));
    let result = (|| {
        std::fs::copy(path, &staging).context("copying the file")?;
        keep_owner(path, &staging);
        // Cover art is read too: whatever isn't read isn't written back.
        let (mut file, mut loaded) = open(&staging, true, options())?;
        loaded.replace(&next)?;
        file.seek(SeekFrom::Start(0))?;
        loaded.save(&mut file)?;
        file.sync_all()?;
        drop(file);
        // Read it back: a file lofty can't parse any more must not replace the original.
        let (written, _) = read(&staging).context("reading the rewritten file back")?;
        let names = |fields: &[TagField]| {
            let mut names: Vec<String> = fields.iter().map(|f| f.name.to_uppercase()).collect();
            names.sort();
            names
        };
        if names(&written) != names(&next) {
            bail!("the rewritten file doesn't hold the expected fields");
        }
        std::fs::rename(&staging, path).context("replacing the file")
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&staging);
    }
    result.with_context(|| format!("writing tags to {}", path.display()))?;
    Ok(true)
}

/// The rewritten copy belongs to whoever owned the original, not to hifid.
#[cfg(unix)]
fn keep_owner(original: &Path, copy: &Path) {
    use std::os::unix::fs::MetadataExt;
    if let Ok(meta) = std::fs::metadata(original) {
        let _ = std::os::unix::fs::chown(copy, Some(meta.uid()), Some(meta.gid()));
    }
}

/// Elsewhere a new file simply belongs to the user running hifi.
#[cfg(not(unix))]
fn keep_owner(_original: &Path, _copy: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(name: &str, values: &[&str]) -> TagField {
        TagField { name: name.into(), values: values.iter().map(|v| v.to_string()).collect() }
    }

    #[test]
    fn applies_edits_like_foobar() {
        let fields = vec![field("ARTIST", &["A"]), field("Genre", &["Ambient", "Drone"]), field("MOOD", &["calm"])];
        let edit = TagEdit {
            uri: String::new(),
            set: vec![field("GENRE", &["Ambient"]), field("ALBUM", &["New"]), field("artist", &[""])],
            remove: vec!["mood".into()],
        };
        assert_eq!(apply(fields, &edit), vec![field("Genre", &["Ambient"]), field("ALBUM", &["New"])]);
    }

    #[test]
    fn vorbis_round_trip_keeps_pictures_and_spelling() {
        let mut tag = VorbisComments::default();
        tag.push("title".into(), "Conta".into());
        tag.push("ARTIST".into(), "A".into());
        tag.push("Artist".into(), "B".into());
        tag.push("METADATA_BLOCK_PICTURE".into(), "xyz".into());
        let fields = tag.fields();
        assert_eq!(fields, vec![field("title", &["Conta"]), field("ARTIST", &["A", "B"])]);

        tag.replace(&[field("title", &["Conta"]), field("MY FIELD", &["1"])]).unwrap();
        let items: Vec<_> = tag.items().collect();
        assert_eq!(items, vec![("METADATA_BLOCK_PICTURE", "xyz"), ("title", "Conta"), ("MY FIELD", "1")]);
    }

    #[test]
    fn ape_round_trip_splits_multiple_values() {
        let mut tag = ApeTag::default();
        tag.replace(&[field("Artist", &["A", "B"]), field("CUSTOM", &["x"])]).unwrap();
        assert_eq!(tag.fields(), vec![field("Artist", &["A", "B"]), field("CUSTOM", &["x"])]);
    }

    #[test]
    fn rejects_names_vorbis_cannot_store() {
        assert!(check_name("ALBUM ARTIST").is_ok());
        assert!(check_name("A=B").is_err());
        assert!(check_name("").is_err());
        assert!(check_name("TITLE\u{e9}").is_err());
    }
}
