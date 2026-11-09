//! Lidarr's API (v1): its catalogue (backed by MusicBrainz through Lidarr's metadata server), what it
//! has, searches, its download queue and imports. Authenticated with `X-Api-Key`.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{answer, base, client};

const SERVICE: &str = "Lidarr";

pub struct Lidarr {
    http: reqwest::Client,
    base: String,
    key: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootFolder {
    pub id: u32,
    pub path: String,
    #[serde(default)]
    pub default_quality_profile_id: u32,
    #[serde(default)]
    pub default_metadata_profile_id: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    /// 0 for an artist Lidarr doesn't have (in lookup results).
    #[serde(default)]
    pub id: u32,
    pub artist_name: String,
    pub foreign_artist_id: String,
    /// What tells namesakes apart: "UK dubstep".
    #[serde(default)]
    pub disambiguation: String,
    /// Where the artist's folder is (or would be).
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub monitored: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    /// 0 for an album Lidarr doesn't have (in lookup results).
    #[serde(default)]
    pub id: u32,
    pub title: String,
    /// MusicBrainz release group.
    pub foreign_album_id: String,
    #[serde(default)]
    pub album_type: String,
    #[serde(default)]
    pub secondary_types: Vec<String>,
    pub release_date: Option<String>,
    #[serde(default)]
    pub monitored: bool,
    #[serde(default)]
    pub artist_id: u32,
    pub artist: Option<Artist>,
    /// Only in lists of albums Lidarr has, not in lookups.
    pub statistics: Option<Statistics>,
    #[serde(default)]
    pub images: Vec<Image>,
    /// Only in lookups.
    pub remote_cover: Option<String>,
    #[serde(default)]
    pub releases: Vec<Release>,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Statistics {
    pub track_file_count: u32,
    pub track_count: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    pub cover_type: String,
    pub remote_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub id: u32,
    #[serde(default)]
    pub monitored: bool,
    #[serde(default)]
    pub track_count: u32,
}

impl Album {
    /// Cover art on the web (Lidarr's cache of the Cover Art Archive).
    pub fn cover(&self) -> Option<String> {
        self.remote_cover
            .clone()
            .or_else(|| self.images.iter().find(|i| i.cover_type == "cover").and_then(|i| i.remote_url.clone()))
    }

    pub fn year(&self) -> Option<u32> {
        self.release_date.as_deref()?.get(..4)?.parse().ok().filter(|&y| y > 0)
    }

    /// "Album", or "Album · Live", "Single", "EP"…
    pub fn kind(&self) -> String {
        std::iter::once(self.album_type.as_str())
            .chain(self.secondary_types.iter().map(String::as_str))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" · ")
    }

    pub fn artist_name(&self) -> &str {
        self.artist.as_ref().map_or("", |a| a.artist_name.as_str())
    }

    /// The release whose tracks Lidarr is matching files against.
    pub fn monitored_release(&self) -> Option<u32> {
        self.releases.iter().find(|r| r.monitored).map(|r| r.id)
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: u32,
    pub album_id: u32,
    pub title: String,
    /// As printed: "3", "A2"…
    #[serde(default)]
    pub track_number: String,
    #[serde(default)]
    pub absolute_track_number: u32,
    #[serde(default)]
    pub medium_number: u32,
    /// Milliseconds.
    #[serde(default)]
    pub duration: u64,
    #[serde(default)]
    pub has_file: bool,
    /// 0 while it has no file.
    #[serde(default)]
    pub track_file_id: u32,
}

impl Track {
    /// Its number on its disc: the printed one when that is a number, else its place in the album.
    pub fn number(&self) -> u32 {
        self.track_number.parse().unwrap_or(self.absolute_track_number)
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackFile {
    pub id: u32,
    pub album_id: u32,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Command {
    pub id: u32,
    pub name: String,
    /// queued, started, completed, failed, aborted, cancelled, orphaned
    pub status: String,
    pub result: Option<String>,
    pub message: Option<String>,
}

impl Command {
    pub fn finished(&self) -> bool {
        !matches!(self.status.as_str(), "queued" | "started")
    }

    pub fn succeeded(&self) -> bool {
        self.status == "completed" && self.result.as_deref() != Some("unsuccessful")
    }
}

/// A download Lidarr is following, from grab to import.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueItem {
    pub id: u32,
    pub album_id: Option<u32>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub size: f64,
    #[serde(default)]
    pub sizeleft: f64,
    #[serde(default)]
    pub status: String,
    /// ok, warning, error
    pub tracked_download_status: Option<String>,
    /// downloading, downloadFailed, downloadFailedPending, importBlocked, importPending, importing,
    /// importFailed, imported, ignored
    pub tracked_download_state: Option<String>,
    #[serde(default)]
    pub status_messages: Vec<StatusMessage>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusMessage {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub messages: Vec<String>,
}

impl QueueItem {
    /// How much is in, 0 to 1, when the size is known.
    pub fn progress(&self) -> Option<f32> {
        (self.size > 0.0).then(|| ((self.size - self.sizeleft) / self.size).clamp(0.0, 1.0) as f32)
    }

    /// What is wrong, in Lidarr's words, when anything is.
    pub fn problem(&self) -> Option<String> {
        if let Some(error) = self.error_message.as_deref().filter(|e| !e.is_empty()) {
            return Some(error.to_string());
        }
        let messages: Vec<&str> =
            self.status_messages.iter().flat_map(|m| m.messages.iter().map(String::as_str)).collect();
        (!messages.is_empty()).then(|| messages.join(" "))
    }
}

/// A file Lidarr's manual import found, with its guess at what it is.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualImportItem {
    pub path: String,
    #[serde(default)]
    pub album_release_id: u32,
    /// Passed back as it came.
    #[serde(default)]
    pub quality: Value,
    #[serde(default)]
    pub rejections: Vec<Rejection>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rejection {
    #[serde(default)]
    pub reason: String,
}

/// Where a new artist goes and how Lidarr treats it: a root folder's defaults.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Placement {
    pub root: String,
    pub quality_profile: u32,
    pub metadata_profile: u32,
}

impl Lidarr {
    pub fn new(url: &str, key: &str) -> anyhow::Result<Self> {
        Ok(Self { http: client()?, base: base(url), key: key.to_string() })
    }

    fn url(&self, path: &str) -> String {
        format!("{}/api/v1/{path}", self.base)
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str, query: &[(&str, String)]) -> anyhow::Result<T> {
        let request = self.http.get(self.url(path)).header("X-Api-Key", &self.key).query(query);
        answer(SERVICE, request.send().await).await
    }

    async fn send<T: serde::de::DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: &Value,
    ) -> anyhow::Result<T> {
        let request = self.http.request(method, self.url(path)).header("X-Api-Key", &self.key).json(body);
        answer(SERVICE, request.send().await).await
    }

    pub async fn root_folders(&self) -> anyhow::Result<Vec<RootFolder>> {
        self.get("rootfolder", &[]).await
    }

    pub async fn artists(&self) -> anyhow::Result<Vec<Artist>> {
        self.get("artist", &[]).await
    }

    /// Every album Lidarr has of an artist, with statistics.
    pub async fn albums_of(&self, artist: u32) -> anyhow::Result<Vec<Album>> {
        self.get("album", &[("artistId", artist.to_string())]).await
    }

    /// Albums Lidarr has, by id, with statistics.
    pub async fn albums(&self, ids: &[u32]) -> anyhow::Result<Vec<Album>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let query: Vec<_> = ids.iter().map(|id| ("albumIds", id.to_string())).collect();
        self.get("album", &query).await
    }

    pub async fn album(&self, id: u32) -> anyhow::Result<Album> {
        self.get(&format!("album/{id}"), &[]).await
    }

    /// Searches the catalogue: albums Lidarr has (`id` set) and doesn't.
    pub async fn lookup(&self, term: &str) -> anyhow::Result<Vec<Album>> {
        Ok(self.lookup_raw(term).await?.into_iter().filter_map(|v| serde_json::from_value(v).ok()).collect())
    }

    /// One release group, as Lidarr's add expects it back: the raw lookup result.
    pub async fn lookup_album(&self, foreign_album_id: &str) -> anyhow::Result<Option<Value>> {
        let found = self.lookup_raw(&format!("lidarr:{foreign_album_id}")).await?;
        Ok(found.into_iter().find(|v| v["foreignAlbumId"].as_str() == Some(foreign_album_id)))
    }

    async fn lookup_raw(&self, term: &str) -> anyhow::Result<Vec<Value>> {
        self.get("album/lookup", &[("term", term.to_string())]).await
    }

    /// Searches the catalogue for artists: those Lidarr has (`id` set) and those it doesn't.
    pub async fn lookup_artists(&self, term: &str) -> anyhow::Result<Vec<Artist>> {
        let found: Vec<Value> = self.get("artist/lookup", &[("term", term.to_string())]).await?;
        Ok(found.into_iter().filter_map(|v| serde_json::from_value(v).ok()).collect())
    }

    /// One artist (MusicBrainz id), as Lidarr's add expects it back: the raw lookup result.
    pub async fn lookup_artist(&self, foreign_artist_id: &str) -> anyhow::Result<Option<Value>> {
        let found: Vec<Value> = self.get("artist/lookup", &[("term", format!("lidarr:{foreign_artist_id}"))]).await?;
        Ok(found.into_iter().find(|v| v["foreignArtistId"].as_str() == Some(foreign_artist_id)))
    }

    /// Adds an artist; returns it as Lidarr stored it.
    pub async fn add_artist(&self, artist: &Value) -> anyhow::Result<Artist> {
        self.send(reqwest::Method::POST, "artist", artist).await
    }

    /// The tracks of an album's monitored release.
    pub async fn tracks(&self, album: u32) -> anyhow::Result<Vec<Track>> {
        self.get("track", &[("albumId", album.to_string())]).await
    }

    pub async fn track_files(&self, albums: &[u32]) -> anyhow::Result<Vec<TrackFile>> {
        if albums.is_empty() {
            return Ok(Vec::new());
        }
        let query: Vec<_> = albums.iter().map(|id| ("albumId", id.to_string())).collect();
        self.get("trackfile", &query).await
    }

    pub async fn artist_files(&self, artist: u32) -> anyhow::Result<Vec<TrackFile>> {
        self.get("trackfile", &[("artistId", artist.to_string())]).await
    }

    /// Adds an album (and its artist, when Lidarr has neither); returns it as Lidarr stored it.
    pub async fn add_album(&self, album: &Value) -> anyhow::Result<Album> {
        self.send(reqwest::Method::POST, "album", album).await
    }

    pub async fn set_monitored(&self, albums: &[u32], monitored: bool) -> anyhow::Result<()> {
        let body = json!({ "albumIds": albums, "monitored": monitored });
        let _: Value = self.send(reqwest::Method::PUT, "album/monitor", &body).await?;
        Ok(())
    }

    /// Searches every indexer for these albums and grabs the best release it finds.
    pub async fn search(&self, albums: &[u32]) -> anyhow::Result<Command> {
        self.send(reqwest::Method::POST, "command", &json!({ "name": "AlbumSearch", "albumIds": albums })).await
    }

    pub async fn command(&self, id: u32) -> anyhow::Result<Command> {
        self.get(&format!("command/{id}"), &[]).await
    }

    /// Lidarr's downloads of these albums.
    pub async fn queue(&self, albums: &[u32]) -> anyhow::Result<Vec<QueueItem>> {
        let query: Vec<_> = albums.iter().map(|id| ("albumIds", id.to_string())).collect();
        self.get("queue/details", &query).await
    }

    /// The audio files in `folder` (as Lidarr sees it), with what Lidarr makes of them.
    pub async fn import_candidates(&self, folder: &str) -> anyhow::Result<Vec<ManualImportItem>> {
        let query = [("folder", folder.to_string()), ("filterExistingFiles", "false".into())];
        self.get("manualimport", &query).await
    }

    /// Files one file as one track of an album, moving it into the library.
    pub async fn import(&self, file: &ImportFile) -> anyhow::Result<Command> {
        self.send(reqwest::Method::POST, "command", &import_command(file)).await
    }
}

/// One file for [`Lidarr::import`]: exactly which track it is.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportFile {
    pub path: String,
    pub artist: u32,
    pub album: u32,
    pub release: u32,
    pub track: u32,
    pub quality: Value,
}

/// A `ManualImport` command. Lidarr imports what it is told without re-checking the match, so one
/// song goes into its album even though the rest is missing.
pub fn import_command(file: &ImportFile) -> Value {
    json!({
        "name": "ManualImport",
        "importMode": "move",
        "replaceExistingFiles": false,
        "files": [{
            "path": file.path,
            "artistId": file.artist,
            "albumId": file.album,
            "albumReleaseId": file.release,
            "trackIds": [file.track],
            "quality": file.quality,
            "indexerFlags": 0,
            "disableReleaseSwitching": true,
        }],
    })
}

/// An artist lookup result made into an artist for Lidarr to add, to list their albums: it wants
/// none of them, nor anything that comes out later, and searches for nothing.
pub fn new_artist(lookup: &Value, placement: &Placement) -> Value {
    let mut artist = lookup.clone();
    artist["qualityProfileId"] = json!(placement.quality_profile);
    artist["metadataProfileId"] = json!(placement.metadata_profile);
    artist["rootFolderPath"] = json!(placement.root);
    artist["monitored"] = json!(true);
    artist["monitorNewItems"] = json!("none");
    artist["addOptions"] = json!({ "monitor": "none", "monitored": true, "searchForMissingAlbums": false });
    artist
}

/// A lookup result made into an album for Lidarr to add. Lidarr starts no search for it: the caller
/// does that, or just wants the tracklist. An artist Lidarr doesn't have yet is added too, set to
/// want only this album (when `wanted`) and nothing that comes out later, rather than a whole
/// discography.
pub fn new_album(lookup: &Value, placement: &Placement, wanted: bool) -> Value {
    let mut album = lookup.clone();
    album["monitored"] = json!(wanted);
    album["addOptions"] = json!({ "searchForNewAlbum": false });
    let artist = &mut album["artist"];
    if artist["id"].as_u64().unwrap_or(0) == 0 {
        let foreign = lookup["foreignAlbumId"].clone();
        artist["qualityProfileId"] = json!(placement.quality_profile);
        artist["metadataProfileId"] = json!(placement.metadata_profile);
        artist["rootFolderPath"] = json!(placement.root);
        artist["monitored"] = json!(true);
        artist["monitorNewItems"] = json!("none");
        artist["addOptions"] = json!({
            "monitor": "none",
            "albumsToMonitor": if wanted { vec![foreign] } else { Vec::new() },
            "monitored": true,
            "searchForMissingAlbums": false,
        });
    }
    album
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOOKUP: &str = include_str!("../tests/fixtures/lookup.json");
    const ALBUMS: &str = include_str!("../tests/fixtures/albums.json");
    const TRACKS: &str = include_str!("../tests/fixtures/tracks.json");
    const QUEUE: &str = include_str!("../tests/fixtures/queue.json");

    #[test]
    fn reads_lookups() {
        let albums: Vec<Album> = serde_json::from_str(LOOKUP).unwrap();
        let [have, new] = &albums[..] else { panic!("two results") };
        assert_eq!((have.id, have.title.as_str(), have.year()), (42, "Borders", Some(2017)));
        assert_eq!(have.artist_name(), "Emptyset");
        assert!(have.cover().unwrap().starts_with("https://images.lidarr.audio/"));
        assert!(have.statistics.is_none());
        assert_eq!((new.id, new.kind().as_str()), (0, "EP"));
        assert_eq!(new.artist.as_ref().unwrap().id, 1);
    }

    #[test]
    fn reads_albums_and_tracks() {
        let albums: Vec<Album> = serde_json::from_str(ALBUMS).unwrap();
        assert_eq!(albums[0].statistics, Some(Statistics { track_file_count: 6, track_count: 6 }));
        assert_eq!(albums[1].statistics.unwrap().track_file_count, 0);
        assert_eq!(albums[0].monitored_release(), Some(1));
        assert!(albums[0].cover().is_some());

        let tracks: Vec<Track> = serde_json::from_str(TRACKS).unwrap();
        assert_eq!(tracks.len(), 3);
        assert_eq!((tracks[0].title.as_str(), tracks[0].number(), tracks[0].duration), ("Flint", 1, 169_000));
        assert!(tracks[0].has_file && !tracks[2].has_file);
    }

    #[test]
    fn reads_the_queue() {
        let queue: Vec<QueueItem> = serde_json::from_str(QUEUE).unwrap();
        assert_eq!(queue[0].album_id, Some(158));
        assert_eq!(queue[0].progress(), Some(0.25));
        assert_eq!(queue[0].problem(), None);
        assert_eq!(queue[1].tracked_download_state.as_deref(), Some("importBlocked"));
        assert_eq!(queue[1].problem().unwrap(), "Has missing tracks. Album match is not close enough");
    }

    #[test]
    fn commands_finish() {
        let command = |status: &str, result: Option<&str>| Command {
            id: 1,
            name: "AlbumSearch".into(),
            status: status.into(),
            result: result.map(Into::into),
            message: None,
        };
        assert!(!command("queued", None).finished());
        assert!(!command("started", None).finished());
        assert!(command("completed", Some("successful")).succeeded());
        assert!(!command("completed", Some("unsuccessful")).succeeded());
        assert!(command("failed", None).finished() && !command("failed", None).succeeded());
    }

    #[test]
    fn new_artists_want_only_this_album() {
        let lookup: Vec<Value> = serde_json::from_str(LOOKUP).unwrap();
        let mut album = lookup[1].clone();
        album["artist"]["id"] = json!(0);
        let placement = Placement { root: "/data/Music".into(), quality_profile: 2, metadata_profile: 1 };

        let added = new_album(&album, &placement, true);
        assert_eq!(added["monitored"], json!(true));
        assert_eq!(added["addOptions"]["searchForNewAlbum"], json!(false));
        let artist = &added["artist"];
        assert_eq!(artist["rootFolderPath"], json!("/data/Music"));
        assert_eq!((artist["qualityProfileId"].clone(), artist["metadataProfileId"].clone()), (json!(2), json!(1)));
        assert_eq!(artist["monitorNewItems"], json!("none"));
        assert_eq!(artist["addOptions"]["monitor"], json!("none"));
        assert_eq!(artist["addOptions"]["albumsToMonitor"], json!(["452ede5d-70a2-45ab-993d-cab9ecd47cb4"]));

        // Just for its tracklist: nothing wanted.
        let quiet = new_album(&album, &placement, false);
        assert_eq!(quiet["monitored"], json!(false));
        assert_eq!(quiet["artist"]["addOptions"]["albumsToMonitor"], json!([]));

        // An artist Lidarr has keeps its own settings.
        let known = new_album(&lookup[1], &placement, true);
        assert!(known["artist"].get("addOptions").is_none());
        assert_eq!(known["artist"]["monitorNewItems"], json!("all"));
    }

    #[test]
    fn new_artists_want_nothing() {
        let lookup = json!({ "artistName": "Burial", "foreignArtistId": "9ddce51c", "disambiguation": "UK dubstep" });
        let placement = Placement { root: "/data/Music".into(), quality_profile: 2, metadata_profile: 1 };
        let artist = new_artist(&lookup, &placement);
        assert_eq!(artist["rootFolderPath"], json!("/data/Music"));
        assert_eq!(artist["monitorNewItems"], json!("none"));
        assert_eq!(artist["addOptions"]["monitor"], json!("none"));
        assert_eq!(artist["addOptions"]["searchForMissingAlbums"], json!(false));
        let read: Artist = serde_json::from_value(lookup).unwrap();
        assert_eq!((read.id, read.disambiguation.as_str()), (0, "UK dubstep"));
    }

    #[test]
    fn imports_one_track() {
        let file = ImportFile {
            path: "/data/Downloads/slskdpp/Ash/01 Flint.flac".into(),
            artist: 1,
            album: 158,
            release: 9,
            track: 1001,
            quality: json!({"quality": {"id": 6, "name": "FLAC"}}),
        };
        let command = import_command(&file);
        assert_eq!(command["name"], json!("ManualImport"));
        assert_eq!(command["importMode"], json!("move"));
        let sent = &command["files"][0];
        assert_eq!(sent["trackIds"], json!([1001]));
        assert_eq!(sent["albumReleaseId"], json!(9));
        assert_eq!(sent["quality"]["quality"]["name"], json!("FLAC"));
    }
}
