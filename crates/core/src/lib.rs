//! Wire types shared by the daemon and the web UI.
//!
//! Every exported type is mirrored into TypeScript by ts-rs; run
//! `cargo test -p virgil-core` to regenerate `web/src/lib/types/`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Name of the built-in source (the library, played through MPD).
pub const LOCAL_SOURCE: &str = "local";

/// Days from 1970-01-01 to a date in the proleptic Gregorian calendar (after Howard Hinnant).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
    era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468
}

/// Unix time from an RFC 3339 timestamp: `2026-10-30T11:00:00Z`, with an optional fraction and `Z`
/// or a `+01:00` offset.
pub fn unix_time(s: &str) -> Option<u32> {
    let num = |at: usize, len: usize| s.get(at..at + len)?.parse::<i64>().ok();
    let days = days_from_civil(num(0, 4)?, num(5, 2)?, num(8, 2)?);
    let seconds = num(11, 2)? * 3600 + num(14, 2)? * 60 + num(17, 2)?;
    let zone = s.get(19..)?.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    let offset = match zone {
        "" | "Z" => 0,
        z => {
            let sign = match z.as_bytes()[0] {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            sign * (z.get(1..3)?.parse::<i64>().ok()? * 3600 + z.get(4..6)?.parse::<i64>().ok()? * 60)
        }
    };
    u32::try_from(days * 86_400 + seconds - offset).ok()
}

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
    /// Who asked for it, when anyone did (virgild remembers; MPD can't).
    pub pick: Option<Pick>,
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
    /// How many discs (CDs, LPs) the release spans.
    pub discs: u32,
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
    /// One per disc, in order.
    pub discs: Vec<Disc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Disc {
    pub number: u32,
    /// Set when the disc has a title of its own, e.g. the second LP of
    /// "Flying Fish Ambience" being "Saga of Capsizing Dolphins".
    pub title: Option<String>,
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

/// Virgil as a HomeKit switch, for the Settings page. Absent when it is off.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HomekitStatus {
    /// Added to a home (nobody else can add it until it is removed).
    pub paired: bool,
    /// Visible on the network (registered with avahi-daemon).
    pub advertised: bool,
    /// `XXX-XX-XXX`, while it can be added.
    pub setup_code: Option<String>,
    /// `X-HM://…`, opens the Home app's pairing, while it can be added.
    pub setup_uri: Option<String>,
}

/// One of MPD's audio outputs: a sound card Virgil can play to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Output {
    /// MPD's output id.
    pub id: u32,
    /// MPD's output name; on Linux the ALSA card id ("sndrpihifiberry").
    pub name: String,
    /// What people call it ("HiFiBerry Digi+ Pro").
    pub label: String,
    /// MPD's output plugin: "alsa", "null", …
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

/// An internet radio station: one of NTS's live channels or an Infinite Mixtape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Station {
    /// `nts1`, `nts2`, or a mixtape's alias (`poolside`).
    pub id: String,
    /// "NTS 1", "Poolside".
    pub name: String,
    /// A mixtape's one-liner ("Balearic, boogie, and sophisti-pop…"); empty for live channels.
    pub tagline: String,
    /// A mixtape's longer blurb.
    pub description: Option<String>,
    /// The MP3 stream (256 kbps).
    pub stream: String,
    /// The same as AAC (256 kbps): NTS's HLS playlist. MPD can't play HLS,
    /// so virgild relays it as one stream and MPD plays that instead.
    pub stream_aac: Option<String>,
    /// Square artwork, about 800 px, and a 200 px version. A live channel's
    /// are the show on air's.
    pub art: Option<String>,
    pub thumb: Option<String>,
    /// A mixtape's icon (128 px PNG), drawn in black and in white.
    pub icon_black: Option<String>,
    pub icon_white: Option<String>,
    /// Live channels: the show on air and the one after it.
    pub now: Option<Broadcast>,
    pub next: Option<Broadcast>,
}

/// A show on one of the live channels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Broadcast {
    pub title: String,
    /// Unix time, seconds.
    pub start: u32,
    pub end: u32,
    /// "London".
    pub location: Option<String>,
    pub description: Option<String>,
    pub genres: Vec<String>,
    pub art: Option<String>,
    pub thumb: Option<String>,
    /// The episode's page on nts.live.
    pub url: Option<String>,
}

/// Everything the Radio tab shows.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Radio {
    pub live: Vec<Station>,
    pub mixtapes: Vec<Station>,
}

/// An album as Lidarr knows it: in the library, partly, or not at all.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CatalogAlbum {
    /// MusicBrainz release group: how a download names the album.
    pub id: String,
    pub title: String,
    pub artist: String,
    pub year: Option<u32>,
    /// "Album", "EP", "Single", "Album · Live"…
    pub kind: String,
    /// Cover art on the web.
    pub cover: Option<String>,
    /// Tracks in the library, of `total`; both 0 while Lidarr doesn't have the album.
    pub have: u32,
    pub total: u32,
    /// Lidarr is looking out for it.
    pub wanted: bool,
    /// Virgil's album, when some of it is in the library.
    pub album_id: Option<String>,
    /// Only when asked for, and once Lidarr has the album.
    pub tracks: Vec<CatalogTrack>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CatalogTrack {
    /// Lidarr's track id: how a download names the song.
    pub id: u32,
    pub title: String,
    pub disc: u32,
    pub number: u32,
    /// Seconds.
    pub duration: f64,
    /// In the library already.
    pub have: bool,
}

/// An artist in Lidarr's catalogue.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct CatalogArtist {
    /// MusicBrainz artist id.
    pub id: String,
    pub name: String,
    /// What tells namesakes apart: "UK dubstep".
    pub disambiguation: String,
    /// Lidarr follows them, so their albums can be listed.
    pub known: bool,
}

/// `GET /api/catalog/search`: artists and albums.
#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct CatalogResults {
    pub artists: Vec<CatalogArtist>,
    pub albums: Vec<CatalogAlbum>,
}

/// What Lidarr knows by an artist that the library has none of.
#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct ArtistCatalog {
    /// Lidarr follows the artist; when not, nothing is listed.
    pub known: bool,
    pub albums: Vec<CatalogAlbum>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum DownloadKind {
    /// A whole album, through Lidarr.
    Album,
    /// One song from Soulseek, filed into its album by Lidarr.
    Song,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DownloadState {
    /// Lidarr is adding the album and reading its tracklist.
    Adding,
    /// Looking for it: Lidarr through its indexers, or Soulseek.
    Searching,
    /// In someone's upload queue.
    Waiting,
    Downloading,
    /// Lidarr is filing it into the library.
    Importing,
    Done,
    /// Nowhere to be found for now; Lidarr keeps a wanted album on its list.
    NotFound,
    Failed,
}

impl DownloadState {
    /// Nothing more will happen unless it is tried again.
    pub fn finished(self) -> bool {
        matches!(self, Self::Done | Self::NotFound | Self::Failed)
    }
}

/// Getting an album or a song the library doesn't have.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Download {
    pub id: u32,
    pub kind: DownloadKind,
    /// The album, as in `CatalogAlbum::id`.
    pub album: String,
    /// The song, as in `CatalogTrack::id`.
    pub track: Option<u32>,
    /// The song's title, or the album's.
    pub title: String,
    pub artist: String,
    pub album_title: String,
    pub cover: Option<String>,
    pub state: DownloadState,
    /// 0 to 1, while downloading.
    pub progress: Option<f32>,
    /// More about the state: a place in a queue, what went wrong.
    pub note: Option<String>,
    /// `Person::id` of whoever asked for it.
    pub by: Option<String>,
    /// Virgil's album, once the music is in.
    pub album_id: Option<String>,
    /// Unix times.
    pub started: u32,
    pub updated: u32,
}

/// Downloads from Lidarr and slskdpp, when Virgil is set up for them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct Downloads {
    /// Single songs can be downloaded too (slskdpp is set up).
    pub songs: bool,
    /// Newest first.
    pub jobs: Vec<Download>,
}

/// `POST /api/downloads`: an album, or one song of it.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct DownloadRequest {
    pub album: String,
    #[ts(optional)]
    pub track: Option<u32>,
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
        homekit: Option<HomekitStatus>,
        party: Party,
        /// None when Virgil isn't set up to download.
        downloads: Option<Downloads>,
    },
    Player(PlayerState),
    Outputs(Outputs),
    Queue(Queue),
    Signal(SignalPath),
    Source(SourceState),
    Library(LibraryStatus),
    Homekit(HomekitStatus),
    Party(Party),
    Downloads(Downloads),
    /// A moment for everyone in the room: someone called in, the room voted a song off.
    Shout {
        text: String,
    },
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
    /// A dedication that comes with the request.
    #[ts(optional)]
    pub note: Option<String>,
}

/// Where a request landed.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct Added {
    /// Queue ids of the new items, in order.
    pub ids: Vec<u32>,
    /// Place in line of the first: 0 is playing now, 1 is next.
    pub position: u32,
    /// Seconds of music before it starts.
    pub ahead: f64,
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

/// Someone using Virgil from their own phone or computer. Each device holds a secret token that
/// virgild maps to its person; only `id` is ever sent to others.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Person {
    pub id: String,
    pub name: String,
    /// One to three letters: the call sign shown on everything they request.
    pub sign: String,
    /// Index into the eight call-sign colours.
    pub color: u8,
    /// Hosts keep every control while the request line is open.
    pub host: bool,
}

/// Who asked for a queue item, and the request it came in with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Pick {
    /// `Person::id`.
    pub by: String,
    /// The songs of one request share it, so an album stays one turn.
    pub request: u32,
    /// Unix time it was requested.
    pub at: u32,
    pub note: Option<String>,
}

/// What guests may do to playback while the line is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum SkipRule {
    /// Enough votes skip the song.
    #[default]
    Vote,
    /// Guests skip like hosts.
    Free,
    /// Only hosts and whoever picked the song skip.
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Rules {
    /// Requests take turns, one per person per round, instead of going to the end.
    pub take_turns: bool,
    /// Guests may pause and resume.
    pub guests_pause: bool,
    pub skip: SkipRule,
}

impl Default for Rules {
    fn default() -> Self {
        Self { take_turns: true, guests_pause: true, skip: SkipRule::Vote }
    }
}

/// Votes on the song playing now.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Votes {
    pub song_id: Option<u32>,
    /// Person ids who want it skipped.
    pub skip: Vec<String>,
    /// Person ids cheering it.
    pub cheer: Vec<String>,
    /// Skip votes it takes.
    pub need: u32,
}

/// The request line: whether friends can call in, and who is around.
#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct Party {
    pub open: bool,
    pub rules: Rules,
    pub people: Vec<Person>,
    /// Person ids with Virgil open now or lately.
    pub here: Vec<String>,
    pub votes: Votes,
}

/// A song that played while the line was open.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Played {
    pub uri: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_id: Option<String>,
    pub art: Option<String>,
    pub duration: f64,
    /// Unix time it started.
    pub at: u32,
    pub pick: Option<Pick>,
    pub cheers: u32,
    pub skip_votes: u32,
    pub skipped: bool,
}

/// The set list of one night on the request line.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Aircheck {
    pub opened: u32,
    pub closed: Option<u32>,
    /// Everyone who requested something, as they were that night.
    pub people: Vec<Person>,
    pub played: Vec<Played>,
}

/// `PUT /api/people/me`: who this device is.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct ProfileRequest {
    pub name: String,
    pub sign: String,
    pub color: u8,
    /// Sent from the join sheet: tell the room someone called in.
    #[serde(default)]
    #[ts(optional)]
    pub join: Option<bool>,
}

/// `POST /api/people/me/link`: this device belongs to someone already here.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct LinkRequest {
    pub person: String,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct HostRequest {
    pub host: bool,
}

/// `POST /api/party`.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[ts(export)]
pub struct PartyRequest {
    #[ts(optional)]
    pub open: Option<bool>,
    #[ts(optional)]
    pub rules: Option<Rules>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_timestamps() {
        assert_eq!(unix_time("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(unix_time("2000-03-01T00:00:00Z"), Some(951_868_800));
        assert_eq!(unix_time("2024-02-29T23:59:59Z"), Some(1_709_251_199));
        assert_eq!(unix_time("2024-03-01T12:34:56Z"), Some(1_709_296_496));
        assert_eq!(unix_time("2026-10-30T11:00:00Z"), Some(1_793_358_000));
        assert_eq!(unix_time("2026-10-30T11:00:00.000Z"), Some(1_793_358_000));
        assert_eq!(unix_time("2026-10-30T11:00:00+00:00"), Some(1_793_358_000));
        assert_eq!(unix_time("2026-10-30T12:00:00+01:00"), Some(1_793_358_000));
        assert_eq!(unix_time("2026-10-30"), None);
        assert_eq!(unix_time("garbage"), None);
    }

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
