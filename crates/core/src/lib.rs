//! Wire types shared by the daemon and the web UI.
//!
//! Every exported type is mirrored into TypeScript by ts-rs; run
//! `cargo test -p hifi-core` to regenerate `web/src/lib/types/`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Name of the built-in source (the library, played through MPD).
pub const LOCAL_SOURCE: &str = "local";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum PlayState {
    #[default]
    Stop,
    Play,
    Pause,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum SampleFormat {
    S8,
    S16,
    S24,
    S32,
    F32,
    Dsd,
}

impl SampleFormat {
    /// Significant bits per sample (1 for DSD).
    pub fn bits(self) -> u8 {
        match self {
            SampleFormat::S8 => 8,
            SampleFormat::S16 => 16,
            SampleFormat::S24 => 24,
            SampleFormat::S32 | SampleFormat::F32 => 32,
            SampleFormat::Dsd => 1,
        }
    }
}

/// A PCM or DSD stream format. For DSD, `rate` is the 1-bit rate (2 822 400 for DSD64).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AudioFormat {
    pub rate: u32,
    pub format: SampleFormat,
    pub channels: u8,
}

impl AudioFormat {
    /// Parses MPD's notation: `44100:24:2`, `44100:f:2`, `dsd64:2` or `352800:dsd:2`.
    pub fn parse_mpd(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        match parts.as_slice() {
            [dsd, channels] => Some(Self {
                rate: dsd.strip_prefix("dsd")?.parse::<u32>().ok()? * 44_100,
                format: SampleFormat::Dsd,
                channels: channels.parse().ok()?,
            }),
            [rate, bits, channels] => {
                let rate: u32 = rate.parse().ok()?;
                let (format, rate) = match *bits {
                    "8" => (SampleFormat::S8, rate),
                    "16" => (SampleFormat::S16, rate),
                    "24" => (SampleFormat::S24, rate),
                    "32" => (SampleFormat::S32, rate),
                    "f" => (SampleFormat::F32, rate),
                    // MPD counts DSD frames in bytes per channel.
                    "dsd" => (SampleFormat::Dsd, rate * 8),
                    _ => return None,
                };
                Some(Self { rate, format, channels: channels.parse().ok()? })
            }
            _ => None,
        }
    }

    /// Inverse of [`AudioFormat::parse_mpd`].
    pub fn to_mpd(&self) -> String {
        match self.format {
            SampleFormat::Dsd if self.rate % 44_100 == 0 => {
                format!("dsd{}:{}", self.rate / 44_100, self.channels)
            }
            SampleFormat::Dsd => format!("{}:dsd:{}", self.rate / 8, self.channels),
            SampleFormat::F32 => format!("{}:f:{}", self.rate, self.channels),
            f => format!("{}:{}:{}", self.rate, f.bits(), self.channels),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlayerState {
    /// False while MPD is unreachable.
    pub online: bool,
    pub state: PlayState,
    /// Queue id of the current item.
    pub song_id: Option<u32>,
    /// Queue position of the current item.
    pub song_pos: Option<u32>,
    pub elapsed: f64,
    pub duration: f64,
    /// Instantaneous bitrate in kbit/s.
    pub bitrate: Option<u32>,
    /// What the decoder produces, before any conversion.
    pub audio: Option<AudioFormat>,
    /// None when MPD has no mixer, which is what bit-perfect playback wants.
    pub volume: Option<u8>,
    pub repeat: bool,
    pub random: bool,
    pub single: bool,
    pub consume: bool,
    pub queue_version: u32,
    pub queue_length: u32,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct QueueItem {
    pub id: u32,
    pub uri: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f64,
    pub format: Option<AudioFormat>,
    pub album_id: Option<String>,
    pub art: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Queue {
    pub version: u32,
    pub items: Vec<QueueItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Album {
    /// Stable across rescans (hash of the grouping key).
    pub id: String,
    pub title: String,
    pub artist: String,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub tracks: u32,
    pub duration: f64,
    /// Artwork key; images live at `/art/{art}-{300|600}.jpg`.
    pub art: Option<String>,
    /// Set when every track shares one format.
    pub format: Option<AudioFormat>,
    /// Unix time of the newest file.
    pub added: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Track {
    pub uri: String,
    pub title: String,
    pub artist: String,
    pub disc: Option<u32>,
    pub track: Option<u32>,
    pub duration: f64,
    pub format: Option<AudioFormat>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AlbumDetail {
    pub album: Album,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TrackHit {
    pub track: Track,
    pub album_id: String,
    pub album_title: String,
    pub art: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SearchResults {
    pub albums: Vec<Album>,
    pub tracks: Vec<TrackHit>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LibraryStatus {
    /// Bumped whenever album data or artwork changes; doubles as the album list's ETag.
    pub revision: u32,
    pub scanning: bool,
    pub albums: u32,
    pub tracks: u32,
    /// Albums checked for artwork and found to have none.
    pub missing_art: u32,
    /// Set while missing artwork is being downloaded.
    pub art_download: Option<ArtProgress>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ArtProgress {
    pub done: u32,
    pub total: u32,
    pub saved: u32,
    /// Albums whose cover could not be saved (e.g. a folder that isn't writable).
    pub failed: u32,
    /// Why the run stopped early (e.g. no internet access).
    pub error: Option<String>,
}

/// What a cover-art download for one album did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ArtOutcome {
    /// Downloaded and saved as cover.jpg / cover.png in the album's folder.
    Saved,
    /// The album already has embedded or folder art; nothing was downloaded.
    AlreadyPresent,
    NotFound,
}

/// What the DAC is actually being fed, from `/proc/asound/<card>/pcm0p/sub0/hw_params`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct HwParams {
    /// ALSA sample format name, e.g. `S24_LE`.
    pub format: String,
    pub rate: u32,
    pub channels: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Verdict {
    /// The output device is closed.
    #[default]
    Idle,
    /// Every source bit reaches the DAC unchanged (zero-padding to a wider container counts).
    BitPerfect,
    Resampled,
    /// Bit depth, channel count or sample type was changed.
    Converted,
    /// Another source (AirPlay, Spotify) owns the device.
    External,
    Unknown,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SignalPath {
    pub verdict: Verdict,
    pub source: Option<AudioFormat>,
    pub output: Option<HwParams>,
    /// The output device's name, e.g. "HiFiBerry Digi+ Pro".
    pub device: Option<String>,
    /// DSD is being carried as DoP inside PCM frames.
    pub dop: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SourceState {
    /// `local` or the name of an external source from the config.
    pub active: String,
    pub available: Vec<String>,
}

impl Default for SourceState {
    fn default() -> Self {
        Self { active: LOCAL_SOURCE.into(), available: Vec::new() }
    }
}

/// One of MPD's audio outputs: a sound card (or, on a desktop, a device) hifi can play to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Output {
    /// MPD's output id.
    pub id: u32,
    /// MPD's output name; on Linux the ALSA card id ("sndrpihifiberry").
    pub name: String,
    /// What people call it ("HiFiBerry Digi+ Pro").
    pub label: String,
    /// MPD's output plugin: "alsa", "wasapi", "null", …
    pub plugin: String,
    pub enabled: bool,
}

/// The outputs, and sound cards plugged in since MPD started (usable after a restart).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Outputs {
    pub outputs: Vec<Output>,
    /// Descriptions of cards without an output yet ("USB Audio Device").
    pub new_cards: Vec<String>,
}

/// Desktop mode (hifi.exe on a PC): settings hifi keeps itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DesktopSettings {
    /// The music library folder MPD plays from.
    pub music_dir: String,
    /// Port of the web UI on 127.0.0.1.
    pub port: u16,
}

/// Pushed over `/api/ws`. `hello` carries the full state; the rest replace one slice of it.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
// `Hello` is sent once per connection; boxing it would only add noise.
#[allow(clippy::large_enum_variant)]
pub enum ServerEvent {
    Hello {
        player: PlayerState,
        queue: Queue,
        signal: SignalPath,
        source: SourceState,
        library: LibraryStatus,
        outputs: Outputs,
    },
    Player(PlayerState),
    Outputs(Outputs),
    Queue(Queue),
    Signal(SignalPath),
    Source(SourceState),
    Library(LibraryStatus),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum AddMode {
    Append,
    /// Insert right after the current item.
    Next,
    /// Clear the queue first.
    Replace,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct AddRequest {
    #[serde(default)]
    pub uris: Vec<String>,
    /// Album id whose tracks are added after `uris`.
    #[ts(optional)]
    pub album: Option<String>,
    pub mode: AddMode,
    #[serde(default)]
    pub play: bool,
    /// One of the added URIs to start playback at (defaults to the first).
    #[ts(optional)]
    pub start: Option<String>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct SeekRequest {
    pub seconds: f64,
}

#[derive(Debug, Clone, Default, Deserialize, TS)]
#[ts(export)]
pub struct ModesRequest {
    #[ts(optional)]
    pub repeat: Option<bool>,
    #[ts(optional)]
    pub random: Option<bool>,
    #[ts(optional)]
    pub single: Option<bool>,
    #[ts(optional)]
    pub consume: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct MoveRequest {
    /// Target queue position.
    pub to: u32,
}

/// One tag field and its values, foobar2000-style: `ARTIST`, `ALBUM ARTIST`,
/// `TRACKNUMBER`, any custom name. Most fields have one value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TagField {
    pub name: String,
    pub values: Vec<String>,
}

/// A file's tags and technical details, as the tag editor shows them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TaggedFile {
    pub uri: String,
    /// Fields in file order. Empty when the file can't be read (see `error`).
    pub fields: Vec<TagField>,
    pub info: Option<FileInfo>,
    /// Why the file can't be read or written (e.g. a format without tag support).
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileInfo {
    /// "FLAC", "MP3", "ALAC", …
    pub codec: String,
    /// "Vorbis comments", "ID3v2", "APEv2", "iTunes metadata"
    pub tag_type: String,
    pub sample_rate: Option<u32>,
    pub bits: Option<u8>,
    pub channels: Option<u8>,
    /// Seconds.
    pub duration: f64,
    /// kbit/s.
    pub bitrate: Option<u32>,
    /// Bytes.
    #[ts(type = "number")]
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct TagsRequest {
    pub uris: Vec<String>,
}

/// Changes to one file. `set` replaces a field's values (an empty list
/// removes it); `remove` drops fields. Names match case-insensitively.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TagEdit {
    pub uri: String,
    #[serde(default)]
    pub set: Vec<TagField>,
    #[serde(default)]
    pub remove: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct TagWriteRequest {
    pub edits: Vec<TagEdit>,
}

#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
pub struct TagWriteResult {
    /// Files whose tags changed.
    pub written: u32,
    pub failed: Vec<TagWriteError>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct TagWriteError {
    pub uri: String,
    pub error: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mpd_formats() {
        let pcm = AudioFormat::parse_mpd("96000:24:2").unwrap();
        assert_eq!(pcm, AudioFormat { rate: 96_000, format: SampleFormat::S24, channels: 2 });
        assert_eq!(pcm.to_mpd(), "96000:24:2");

        let dsd = AudioFormat::parse_mpd("dsd64:2").unwrap();
        assert_eq!(dsd.rate, 2_822_400);
        assert_eq!(dsd.format, SampleFormat::Dsd);
        assert_eq!(dsd.to_mpd(), "dsd64:2");

        let dsd_bytes = AudioFormat::parse_mpd("352800:dsd:2").unwrap();
        assert_eq!(dsd_bytes, dsd);

        assert_eq!(AudioFormat::parse_mpd("44100:f:2").unwrap().format, SampleFormat::F32);
        assert_eq!(AudioFormat::parse_mpd("44100:x:2"), None);
        assert_eq!(AudioFormat::parse_mpd("garbage"), None);
    }
}
