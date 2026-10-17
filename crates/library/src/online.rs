//! Finding cover art online: MusicBrainz + the Cover Art Archive first, then
//! the iTunes Search API. A cover is only taken when artist and title match.

use std::time::{Duration, Instant};

use anyhow::Context;
use serde::Deserialize;
use tokio::sync::Mutex;
use tracing::debug;

const USER_AGENT: &str = concat!("hifi/", env!("CARGO_PKG_VERSION"), " ( https://github.com/mpkonmbk/hifi )");
/// MusicBrainz allows one request per second per client.
const MUSICBRAINZ_INTERVAL: Duration = Duration::from_millis(1100);
/// Covers larger than this are not downloaded.
const MAX_COVER: usize = 20 << 20;
/// Below this edge length (pixels) another source is tried for a sharper cover.
const GOOD_EDGE: u32 = 600;

/// The shorter side of an image, 0 when unreadable.
fn edge(bytes: &[u8]) -> u32 {
    image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()
        .and_then(|reader| reader.into_dimensions().ok())
        .map_or(0, |(w, h)| w.min(h))
}

pub struct Finder {
    http: reqwest::Client,
    last_musicbrainz: Mutex<Option<Instant>>,
}

#[derive(Deserialize)]
struct ReleaseGroups {
    #[serde(rename = "release-groups", default)]
    groups: Vec<ReleaseGroup>,
}

#[derive(Deserialize)]
struct ReleaseGroup {
    id: String,
    title: String,
    #[serde(rename = "artist-credit", default)]
    credits: Vec<Credit>,
}

#[derive(Deserialize)]
struct Credit {
    name: String,
    #[serde(default)]
    joinphrase: String,
}

#[derive(Deserialize)]
struct ItunesResults {
    #[serde(default)]
    results: Vec<ItunesAlbum>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItunesAlbum {
    collection_name: String,
    artist_name: String,
    artwork_url100: Option<String>,
}

/// Lowercase letters and digits only, bracketed parts ("[WEB]", "(Deluxe)")
/// and iTunes' " - EP" / " - Single" suffixes dropped.
pub(crate) fn normalize(s: &str) -> String {
    let lower = s.to_lowercase();
    let lower = lower.trim_end_matches(" - ep").trim_end_matches(" - single");
    let mut out = String::new();
    let mut depth = 0u32;
    for c in lower.chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            c if depth == 0 && c.is_alphanumeric() => out.push(c),
            _ => {}
        }
    }
    if out.is_empty() { lower.chars().filter(|c| c.is_alphanumeric()).collect() } else { out }
}

/// Same title, and the same artist unless the album is a compilation.
pub(crate) fn matches(want_artist: &str, want_title: &str, artist: &str, title: &str) -> bool {
    if normalize(title) != normalize(want_title) {
        return false;
    }
    let want = normalize(want_artist);
    let got = normalize(artist);
    want == "variousartists" || want == got || (!want.is_empty() && (got.contains(&want) || want.contains(&got)))
}

/// A Lucene phrase for MusicBrainz' search.
fn phrase(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

impl Finder {
    pub fn new() -> anyhow::Result<Self> {
        // reqwest is built without a bundled crypto provider; ring is pure Rust and cross-compiles cleanly.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self { http, last_musicbrainz: Mutex::new(None) })
    }

    /// The cover's bytes (JPEG or PNG as published), or None when nothing matches.
    pub async fn find(&self, artist: &str, title: &str) -> anyhow::Result<Option<Vec<u8>>> {
        match self.musicbrainz(artist, title).await {
            Ok(Some(cover)) if edge(&cover) >= GOOD_EDGE => return Ok(Some(cover)),
            // Small scan on MusicBrainz: keep whichever source has the bigger one.
            Ok(Some(cover)) => {
                let other = self.itunes(artist, title).await.unwrap_or_default();
                return Ok(Some(match other {
                    Some(other) if edge(&other) > edge(&cover) => other,
                    _ => cover,
                }));
            }
            Ok(None) => {}
            // No connection at all: iTunes won't fare better, so say so.
            Err(e) if e.downcast_ref::<reqwest::Error>().is_some_and(|e| e.is_connect() || e.is_timeout()) => {
                return Err(e);
            }
            // MusicBrainz busy or erroring: try iTunes.
            Err(e) => debug!("MusicBrainz lookup for {artist} - {title} failed: {e:#}"),
        }
        self.itunes(artist, title).await
    }

    /// One MusicBrainz search, spaced to its rate limit and retried while it says it is busy (503).
    async fn musicbrainz_search(&self, query: &str) -> anyhow::Result<ReleaseGroups> {
        let mut backoff = Duration::from_secs(2);
        for attempt in 0..3 {
            {
                let mut last = self.last_musicbrainz.lock().await;
                if let Some(at) = *last {
                    tokio::time::sleep(MUSICBRAINZ_INTERVAL.saturating_sub(at.elapsed())).await;
                }
                *last = Some(Instant::now());
            }
            let response = self
                .http
                .get("https://musicbrainz.org/ws/2/release-group/")
                .query(&[("query", query), ("fmt", "json"), ("limit", "8")])
                .send()
                .await
                .context("cannot reach MusicBrainz (does the Pi have internet access?)")?;
            if response.status() == reqwest::StatusCode::SERVICE_UNAVAILABLE && attempt < 2 {
                tokio::time::sleep(backoff).await;
                backoff *= 2;
                continue;
            }
            return Ok(response.error_for_status()?.json().await?);
        }
        unreachable!("the last attempt returns")
    }

    async fn musicbrainz(&self, artist: &str, title: &str) -> anyhow::Result<Option<Vec<u8>>> {
        let mut query = format!("releasegroup:{}", phrase(title));
        if normalize(artist) != "variousartists" {
            query.push_str(&format!(" AND artist:{}", phrase(artist)));
        }
        let found = self.musicbrainz_search(&query).await?;
        for group in found.groups {
            let credit: String = group.credits.iter().map(|c| format!("{}{}", c.name, c.joinphrase)).collect();
            if !matches(artist, title, &credit, &group.title) {
                continue;
            }
            for size in ["front-1200", "front"] {
                let url = format!("https://coverartarchive.org/release-group/{}/{size}", group.id);
                if let Some(cover) = self.image(&url).await? {
                    debug!("cover for {artist} - {title} from {url}");
                    return Ok(Some(cover));
                }
            }
        }
        Ok(None)
    }

    async fn itunes(&self, artist: &str, title: &str) -> anyhow::Result<Option<Vec<u8>>> {
        let term = format!("{artist} {title}");
        let found: ItunesResults = self
            .http
            .get("https://itunes.apple.com/search")
            .query(&[("term", term.as_str()), ("entity", "album"), ("limit", "10")])
            .send()
            .await
            .context("cannot reach the iTunes Search API")?
            .error_for_status()?
            .json()
            .await?;
        for album in found.results {
            let Some(small) = album.artwork_url100 else { continue };
            if !matches(artist, title, &album.artist_name, &album.collection_name) {
                continue;
            }
            let url = small.replace("100x100bb", "1200x1200bb");
            if let Some(cover) = self.image(&url).await? {
                debug!("cover for {artist} - {title} from {url}");
                return Ok(Some(cover));
            }
        }
        Ok(None)
    }

    /// Downloads an image; a 404 is "no cover", other failures are errors.
    async fn image(&self, url: &str) -> anyhow::Result<Option<Vec<u8>>> {
        let response = self.http.get(url).send().await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let bytes = response.error_for_status()?.bytes().await?;
        let ok = bytes.len() <= MAX_COVER
            && matches!(image::guess_format(&bytes), Ok(image::ImageFormat::Jpeg | image::ImageFormat::Png));
        Ok(ok.then(|| bytes.to_vec()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_titles_like_people_read_them() {
        assert_eq!(normalize("Hollow [WEB]"), "hollow");
        assert_eq!(normalize("Hollow (2014) [WEB, Album]"), "hollow");
        assert_eq!(normalize("Static Bloom - EP"), "staticbloom");
        assert_eq!(normalize("[Untitled]"), "untitled");
        assert_eq!(normalize("新しい日の誕生"), "新しい日の誕生");
    }

    /// Talks to the real services: `cargo test -p hifi-library -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn finds_a_real_cover() {
        let finder = Finder::new().unwrap();
        let cover = finder.find("Burial", "Untrue").await.unwrap().expect("a cover");
        assert!(cover.len() > 10_000);
        assert!(finder.find("Nobody At All", "No Such Album 7d1f").await.unwrap().is_none());
    }

    #[test]
    fn matches_title_and_artist() {
        assert!(matches("36", "Hollow [WEB]", "36", "Hollow"));
        assert!(matches("Burial", "Untrue", "Burial", "Untrue"));
        assert!(matches("Various Artists", "Frame.Sync", "Someone Else", "Frame Sync"));
        assert!(matches("HKE", "Charming Devil", "HKE & Friend", "Charming Devil"));
        assert!(!matches("Burial", "Untrue", "Burial", "Rival Dealer"));
        assert!(!matches("Burial", "Untrue", "Somebody", "Untrue"));
    }
}
