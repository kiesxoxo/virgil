//! Looking albums up online for the tag editor: MusicBrainz (open, no account
//! needed) and Discogs (anonymous requests). Both are normalized to one shape;
//! the editor decides what to write, and nothing is written until the user
//! saves.

use std::time::Duration;

use anyhow::{Context, bail};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use virgil_core::{LookupCandidate, LookupQuery, LookupRelease, LookupResults, LookupSource, LookupTrack};

use crate::Library;
use crate::online::{Finder, phrase, space};

/// Discogs allows 25 anonymous requests a minute.
const DISCOGS_INTERVAL: Duration = Duration::from_millis(2500);
/// Candidates per source.
const CANDIDATES: &str = "10";
/// Genres kept per release, most fitting first.
const GENRES: usize = 3;

// --- MusicBrainz ---------------------------------------------------------------

#[derive(Deserialize)]
struct MbSearch {
    #[serde(default)]
    releases: Vec<MbRelease>,
}

#[derive(Deserialize)]
struct MbRelease {
    id: String,
    title: String,
    #[serde(rename = "artist-credit", default)]
    credit: Vec<MbCredit>,
    date: Option<String>,
    country: Option<String>,
    #[serde(rename = "label-info", default)]
    labels: Vec<MbLabelInfo>,
    #[serde(default)]
    media: Vec<MbMedium>,
    #[serde(rename = "track-count")]
    track_count: Option<u32>,
    #[serde(default)]
    genres: Vec<MbGenre>,
    #[serde(rename = "release-group")]
    group: Option<MbGroup>,
}

#[derive(Deserialize)]
struct MbCredit {
    name: String,
    #[serde(default)]
    joinphrase: String,
}

#[derive(Deserialize)]
struct MbLabelInfo {
    #[serde(rename = "catalog-number")]
    catno: Option<String>,
    label: Option<MbName>,
}

#[derive(Deserialize)]
struct MbName {
    name: String,
}

#[derive(Deserialize)]
struct MbMedium {
    format: Option<String>,
    position: Option<u32>,
    #[serde(default)]
    tracks: Vec<MbTrack>,
}

#[derive(Deserialize)]
struct MbTrack {
    title: String,
    position: Option<u32>,
    /// Milliseconds.
    length: Option<u64>,
    #[serde(rename = "artist-credit", default)]
    credit: Vec<MbCredit>,
}

#[derive(Deserialize)]
struct MbGenre {
    name: String,
    #[serde(default)]
    count: u32,
}

#[derive(Deserialize)]
struct MbGroup {
    #[serde(rename = "first-release-date")]
    first_release: Option<String>,
    #[serde(default)]
    genres: Vec<MbGenre>,
}

/// "Daniel Avery & Alessandro Cortini" from its credited names and join phrases.
fn mb_credit(credit: &[MbCredit]) -> String {
    credit.iter().map(|c| format!("{}{}", c.name, c.joinphrase)).collect::<String>().trim().to_string()
}

/// "CD", "2×Vinyl", "CD + DVD".
fn mb_format(media: &[MbMedium]) -> Option<String> {
    let mut counts: Vec<(&str, u32)> = Vec::new();
    for format in media.iter().filter_map(|m| m.format.as_deref()) {
        match counts.iter_mut().find(|(f, _)| *f == format) {
            Some((_, n)) => *n += 1,
            None => counts.push((format, 1)),
        }
    }
    let parts: Vec<String> =
        counts.iter().map(|&(f, n)| if n > 1 { format!("{n}×{f}") } else { f.to_string() }).collect();
    (!parts.is_empty()).then(|| parts.join(" + "))
}

fn mb_candidate(r: MbRelease) -> LookupCandidate {
    let label = r.labels.iter().find_map(|l| l.label.as_ref().map(|n| n.name.clone()));
    // MusicBrainz writes "[none]" for releases without one.
    let catno = r.labels.iter().find_map(|l| l.catno.clone()).filter(|c| !c.is_empty() && c != "[none]");
    LookupCandidate {
        source: LookupSource::Musicbrainz,
        url: format!("https://musicbrainz.org/release/{}", r.id),
        artist: mb_credit(&r.credit),
        year: r.date.as_deref().and_then(|d| d.get(..4)).map(str::to_string),
        format: mb_format(&r.media),
        tracks: r.track_count,
        country: r.country,
        label,
        catno,
        title: r.title,
        id: r.id,
    }
}

/// MusicBrainz genres are lowercase ("future garage"): title-case them as
/// libraries usually spell genres, keeping acronyms capitalized.
fn genre_case(name: &str) -> String {
    const ACRONYMS: [&str; 7] = ["uk", "us", "idm", "edm", "ebm", "dnb", "r&b"];
    let word = |w: &str| {
        if ACRONYMS.contains(&w) {
            return w.to_uppercase();
        }
        w.split('-')
            .map(|part| {
                let mut chars = part.chars();
                chars.next().map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
            })
            .collect::<Vec<_>>()
            .join("-")
    };
    name.split(' ').map(word).collect::<Vec<_>>().join(" ")
}

/// The release's and its release group's genres, by votes.
fn mb_genres(release: &[MbGenre], group: &[MbGenre]) -> Vec<String> {
    let mut votes: Vec<(String, u32)> = Vec::new();
    for g in release.iter().chain(group) {
        let name = g.name.to_lowercase();
        match votes.iter_mut().find(|(n, _)| *n == name) {
            Some((_, count)) => *count += g.count,
            None => votes.push((name, g.count)),
        }
    }
    votes.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    votes.into_iter().take(GENRES).map(|(name, _)| genre_case(&name)).collect()
}

fn mb_release(r: MbRelease) -> LookupRelease {
    let artist = mb_credit(&r.credit);
    let mut tracks = Vec::new();
    for (disc_index, medium) in r.media.iter().enumerate() {
        let disc = medium.position.unwrap_or(disc_index as u32 + 1);
        for (index, track) in medium.tracks.iter().enumerate() {
            let credit = mb_credit(&track.credit);
            tracks.push(LookupTrack {
                disc,
                number: track.position.unwrap_or(index as u32 + 1),
                title: track.title.clone(),
                artist: if credit.is_empty() { artist.clone() } else { credit },
                duration: track.length.map(|ms| ms as f64 / 1000.0),
            });
        }
    }
    let (original_date, group_genres) = match r.group {
        Some(g) => (g.first_release, g.genres),
        None => (None, Vec::new()),
    };
    LookupRelease {
        source: LookupSource::Musicbrainz,
        url: format!("https://musicbrainz.org/release/{}", r.id),
        genres: mb_genres(&r.genres, &group_genres),
        date: r.date.filter(|d| !d.is_empty()),
        original_date: original_date.filter(|d| !d.is_empty()),
        title: r.title,
        id: r.id,
        artist,
        tracks,
    }
}

// --- Discogs -----------------------------------------------------------------

#[derive(Deserialize)]
struct DgSearch {
    #[serde(default)]
    results: Vec<DgResult>,
}

#[derive(Deserialize)]
struct DgResult {
    id: u64,
    /// "Artist - Title".
    title: String,
    year: Option<String>,
    country: Option<String>,
    #[serde(default)]
    label: Vec<String>,
    catno: Option<String>,
    #[serde(default)]
    format: Vec<String>,
}

#[derive(Deserialize)]
struct DgRelease {
    id: u64,
    title: String,
    #[serde(default)]
    artists: Vec<DgArtist>,
    /// "1967-09-00" when the day (or month) is unknown.
    released: Option<String>,
    year: Option<u32>,
    #[serde(default)]
    genres: Vec<String>,
    #[serde(default)]
    styles: Vec<String>,
    #[serde(default)]
    tracklist: Vec<DgTrack>,
}

#[derive(Deserialize)]
struct DgArtist {
    name: String,
    /// The name as credited on this release, if it differs.
    #[serde(default)]
    anv: String,
    #[serde(default)]
    join: String,
}

#[derive(Deserialize)]
struct DgTrack {
    #[serde(default)]
    position: String,
    /// "track", "heading" (a section title) or "index" (a track with sub-tracks).
    #[serde(rename = "type_", default)]
    kind: String,
    title: String,
    #[serde(default)]
    duration: String,
    #[serde(default)]
    artists: Vec<DgArtist>,
}

/// Discogs marks namesakes "Burial (2)" and name variations with "*".
fn dg_name(name: &str) -> &str {
    let name = name.trim().trim_end_matches('*');
    match name.rsplit_once(" (") {
        Some((base, n))
            if n.strip_suffix(')').is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())) =>
        {
            base
        }
        _ => name,
    }
}

fn dg_artists(artists: &[DgArtist]) -> String {
    let mut out = String::new();
    for (i, a) in artists.iter().enumerate() {
        out.push_str(dg_name(if a.anv.is_empty() { &a.name } else { &a.anv }));
        if i + 1 < artists.len() {
            match a.join.trim() {
                "" | "," => out.push_str(", "),
                join => out.push_str(&format!(" {join} ")),
            }
        }
    }
    out
}

/// "2:15" or "1:02:03" in seconds.
fn dg_duration(text: &str) -> Option<f64> {
    if text.trim().is_empty() {
        return None;
    }
    text.trim().split(':').try_fold(0.0, |total, part| part.parse::<f64>().ok().map(|n| total * 60.0 + n))
}

/// The disc of a Discogs position: "2-04" and "CD2-04" are on disc 2; vinyl
/// sides ("A1", "B") and plain numbers are on disc 1.
fn dg_disc(position: &str) -> u32 {
    let Some((disc, track)) = position.trim().split_once('-') else { return 1 };
    let digits = disc.trim_start_matches(|c: char| c.is_ascii_alphabetic());
    match (digits.parse(), track.chars().next()) {
        (Ok(n), Some(c)) if c.is_ascii_digit() && n > 0 => n,
        _ => 1,
    }
}

/// "1967-09-00" → "1967-09"; a bare year when that's all there is.
fn dg_date(released: Option<&str>, year: Option<u32>) -> Option<String> {
    let released = released.unwrap_or("").trim();
    let known: Vec<&str> = released.split('-').take_while(|p| !p.is_empty() && p.bytes().any(|b| b != b'0')).collect();
    if known.is_empty() { year.filter(|&y| y > 0).map(|y| y.to_string()) } else { Some(known.join("-")) }
}

fn dg_candidate(r: DgResult) -> LookupCandidate {
    let (artist, title) = match r.title.split_once(" - ") {
        Some((artist, title)) => (dg_name(artist).to_string(), title.to_string()),
        None => (String::new(), r.title.clone()),
    };
    // Discogs lists everything about the format; the first few say what it is.
    let format = (!r.format.is_empty()).then(|| r.format.iter().take(3).cloned().collect::<Vec<_>>().join(", "));
    LookupCandidate {
        source: LookupSource::Discogs,
        id: r.id.to_string(),
        url: format!("https://www.discogs.com/release/{}", r.id),
        year: r.year.filter(|y| !y.is_empty() && y != "0"),
        country: r.country.filter(|c| !c.is_empty()),
        label: r.label.into_iter().next(),
        catno: r.catno.filter(|c| !c.is_empty() && !c.eq_ignore_ascii_case("none")),
        tracks: None,
        format,
        artist,
        title,
    }
}

fn dg_release(r: DgRelease) -> LookupRelease {
    let artist = dg_artists(&r.artists);
    let mut tracks: Vec<LookupTrack> = Vec::new();
    for track in r.tracklist.iter().filter(|t| t.kind != "heading") {
        let disc = dg_disc(&track.position);
        // Positions restart per vinyl side, so number by order within the disc.
        let number = tracks.iter().filter(|t| t.disc == disc).count() as u32 + 1;
        let credit = dg_artists(&track.artists);
        tracks.push(LookupTrack {
            disc,
            number,
            title: track.title.trim().to_string(),
            artist: if credit.is_empty() { artist.clone() } else { credit },
            duration: dg_duration(&track.duration),
        });
    }
    // Styles are specific ("Deep House"), genres broad ("Electronic").
    let mut genres: Vec<String> = Vec::new();
    for g in r.styles.iter().chain(&r.genres) {
        if !genres.iter().any(|known| known.eq_ignore_ascii_case(g)) {
            genres.push(g.clone());
        }
    }
    genres.truncate(GENRES);
    LookupRelease {
        source: LookupSource::Discogs,
        id: r.id.to_string(),
        url: format!("https://www.discogs.com/release/{}", r.id),
        date: dg_date(r.released.as_deref(), r.year),
        original_date: None,
        title: r.title.trim().to_string(),
        artist,
        genres,
        tracks,
    }
}

// --- Requests ----------------------------------------------------------------

async fn discogs_get<T: DeserializeOwned>(finder: &Finder, path: &str, query: &[(&str, &str)]) -> anyhow::Result<T> {
    for attempt in 0..3 {
        space(&finder.last_discogs, DISCOGS_INTERVAL).await;
        let request = finder.http.get(format!("https://api.discogs.com/{path}")).query(query);
        let response = request.send().await.context("cannot reach Discogs (does the Pi have internet access?)")?;
        match response.status() {
            reqwest::StatusCode::TOO_MANY_REQUESTS if attempt < 2 => {
                tokio::time::sleep(Duration::from_secs(5 * (attempt + 1))).await;
            }
            _ => return Ok(response.error_for_status().map_err(reqwest::Error::without_url)?.json().await?),
        }
    }
    bail!("Discogs is busy; try again in a minute")
}

async fn musicbrainz_search(finder: &Finder, query: &LookupQuery) -> anyhow::Result<Vec<LookupCandidate>> {
    let mut lucene = format!("release:{}", phrase(query.album.trim()));
    if !query.artist.trim().is_empty() {
        lucene.push_str(&format!(" AND artist:{}", phrase(query.artist.trim())));
    }
    let found: MbSearch = finder.musicbrainz_get("release/", &[("query", &lucene), ("limit", CANDIDATES)]).await?;
    Ok(found.releases.into_iter().map(mb_candidate).collect())
}

async fn discogs_search(finder: &Finder, query: &LookupQuery) -> anyhow::Result<Vec<LookupCandidate>> {
    let mut params = vec![("type", "release"), ("release_title", query.album.trim()), ("per_page", CANDIDATES)];
    if !query.artist.trim().is_empty() {
        params.push(("artist", query.artist.trim()));
    }
    let found: DgSearch = discogs_get(finder, "database/search", &params).await?;
    Ok(found.results.into_iter().map(dg_candidate).collect())
}

impl Library {
    /// Releases that might be the album, from MusicBrainz and Discogs at once.
    /// A source that fails is reported in `errors`; the other's results stay.
    pub async fn lookup_online(&self, query: &LookupQuery) -> anyhow::Result<LookupResults> {
        if query.album.trim().is_empty() {
            bail!("the album needs a title to look it up");
        }
        let finder = &self.inner.finder;
        let (mb, dg) = tokio::join!(musicbrainz_search(finder, query), discogs_search(finder, query));
        let mut results = LookupResults::default();
        for (source, found) in [("MusicBrainz", mb), ("Discogs", dg)] {
            match found {
                Ok(candidates) => results.candidates.extend(candidates),
                Err(e) => {
                    let message = format!("{e:#}");
                    // Most messages name the source already ("cannot reach Discogs").
                    results.errors.push(if message.contains(source) {
                        message
                    } else {
                        format!("{source}: {message}")
                    });
                }
            }
        }
        Ok(results)
    }

    /// One release's tags and tracklist.
    pub async fn lookup_release(&self, source: LookupSource, id: &str) -> anyhow::Result<LookupRelease> {
        let finder = &self.inner.finder;
        match source {
            LookupSource::Musicbrainz => {
                if id.is_empty() || !id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
                    bail!("not a MusicBrainz release id: {id}");
                }
                let release: MbRelease = finder
                    .musicbrainz_get(
                        &format!("release/{id}"),
                        &[("inc", "recordings+artist-credits+genres+release-groups")],
                    )
                    .await?;
                Ok(mb_release(release))
            }
            LookupSource::Discogs => {
                if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
                    bail!("not a Discogs release id: {id}");
                }
                let release: DgRelease = discogs_get(finder, &format!("releases/{id}"), &[]).await?;
                Ok(dg_release(release))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_musicbrainz_search() {
        let found: MbSearch = serde_json::from_str(
            r#"{"releases": [{"id": "d84892a9-0b1d-3694-a2a1-f19ddb7d0c53", "score": 100, "title": "Untrue",
                "artist-credit": [{"name": "Burial", "artist": {"id": "x"}}],
                "date": "2007-11-05", "country": "XW", "track-count": 13,
                "label-info": [{"catalog-number": "HDBCD002", "label": {"name": "Hyperdub"}}],
                "media": [{"format": "Vinyl", "track-count": 7}, {"format": "Vinyl", "track-count": 6}]}]}"#,
        )
        .unwrap();
        let c = mb_candidate(found.releases.into_iter().next().unwrap());
        assert_eq!((c.title.as_str(), c.artist.as_str(), c.year.as_deref()), ("Untrue", "Burial", Some("2007")));
        assert_eq!((c.label.as_deref(), c.catno.as_deref(), c.tracks), (Some("Hyperdub"), Some("HDBCD002"), Some(13)));
        assert_eq!(c.format.as_deref(), Some("2×Vinyl"));
        assert_eq!(c.url, "https://musicbrainz.org/release/d84892a9-0b1d-3694-a2a1-f19ddb7d0c53");
    }

    #[test]
    fn reads_a_musicbrainz_release() {
        let release: MbRelease = serde_json::from_str(
            r#"{"id": "abc", "title": "Untrue", "date": "2007-11-05",
                "artist-credit": [{"name": "Burial", "joinphrase": ""}],
                "genres": [{"name": "electronic", "count": 1}],
                "release-group": {"first-release-date": "2007-11-05", "genres": [
                    {"name": "2-step", "count": 1}, {"name": "dubstep", "count": 6},
                    {"name": "electronic", "count": 2}, {"name": "future garage", "count": 4},
                    {"name": "uk garage", "count": 1}]},
                "media": [
                    {"position": 1, "format": "CD", "tracks": [
                        {"position": 1, "number": "1", "title": "[untitled]", "length": 50000,
                         "artist-credit": [{"name": "Burial", "joinphrase": ""}]},
                        {"position": 2, "number": "2", "title": "Archangel", "length": 238213,
                         "artist-credit": [{"name": "Burial", "joinphrase": " & "}, {"name": "Four Tet", "joinphrase": ""}]}]},
                    {"position": 2, "format": "CD", "tracks": [
                        {"position": 1, "number": "1", "title": "Bonus", "length": null, "artist-credit": []}]}]}"#,
        )
        .unwrap();
        let r = mb_release(release);
        assert_eq!(r.genres, ["Dubstep", "Future Garage", "Electronic"], "by votes, release and group summed");
        assert_eq!((r.date.as_deref(), r.original_date.as_deref()), (Some("2007-11-05"), Some("2007-11-05")));
        assert_eq!(r.tracks.len(), 3);
        assert_eq!(r.tracks[1].artist, "Burial & Four Tet");
        assert_eq!(r.tracks[1].duration, Some(238.213));
        assert_eq!((r.tracks[2].disc, r.tracks[2].number, r.tracks[2].artist.as_str()), (2, 1, "Burial"));
        assert_eq!(r.tracks[2].duration, None);
    }

    #[test]
    fn cases_genres_like_a_library() {
        assert_eq!(genre_case("future garage"), "Future Garage");
        assert_eq!(genre_case("uk garage"), "UK Garage");
        assert_eq!(genre_case("hip-hop"), "Hip-Hop");
        assert_eq!(genre_case("idm"), "IDM");
    }

    #[test]
    fn reads_a_discogs_search() {
        let found: DgSearch = serde_json::from_str(
            r#"{"pagination": {}, "results": [{"id": 24422834, "title": "Burial - Untrue", "year": "2010",
                "country": "UK", "label": ["Hyperdub", "Domino Publishing Ltd."], "catno": "HDBLP002",
                "format": ["Vinyl", "12\"", "33 ⅓ RPM", "Album", "Reissue"], "uri": "/release/24422834-Burial-Untrue"},
                {"id": 1, "title": "Burial (2) - Something", "catno": "none", "year": "0"}]}"#,
        )
        .unwrap();
        let mut candidates = found.results.into_iter().map(dg_candidate);
        let c = candidates.next().unwrap();
        assert_eq!((c.artist.as_str(), c.title.as_str(), c.year.as_deref()), ("Burial", "Untrue", Some("2010")));
        assert_eq!((c.label.as_deref(), c.catno.as_deref()), (Some("Hyperdub"), Some("HDBLP002")));
        assert_eq!(c.format.as_deref(), Some("Vinyl, 12\", 33 ⅓ RPM"));
        assert_eq!((c.tracks, c.url.as_str()), (None, "https://www.discogs.com/release/24422834"));
        let c = candidates.next().unwrap();
        assert_eq!((c.artist.as_str(), c.catno, c.year), ("Burial", None, None));
    }

    #[test]
    fn reads_a_discogs_release() {
        let release: DgRelease = serde_json::from_str(
            r#"{"id": 1103567, "title": "Have Some More Tea", "year": 1967, "released": "1967-09-00",
                "genres": ["Rock", "Pop"], "styles": ["Psychedelic Rock", "Mod", "Beat"],
                "artists": [{"name": "The Smoke (3)", "anv": "", "join": ""}],
                "tracklist": [
                    {"position": "", "type_": "heading", "title": "Side One", "duration": ""},
                    {"position": "A1", "type_": "track", "title": "Have Some More Tea", "duration": "2:15"},
                    {"position": "A2", "type_": "track", "title": "Medley", "duration": "1:02:03",
                     "artists": [{"name": "Artist*", "anv": "", "join": "Feat."}, {"name": "Guest (12)", "anv": "G.", "join": ""}]},
                    {"position": "B1", "type_": "track", "title": "Victor Henry's Cool Book", "duration": ""},
                    {"position": "2-1", "type_": "track", "title": "Disc Two Opener", "duration": "3:00"}]}"#,
        )
        .unwrap();
        let r = dg_release(release);
        assert_eq!((r.artist.as_str(), r.date.as_deref()), ("The Smoke", Some("1967-09")));
        assert_eq!(r.genres, ["Psychedelic Rock", "Mod", "Beat"], "styles first, three at most");
        let shape: Vec<(u32, u32, &str)> = r.tracks.iter().map(|t| (t.disc, t.number, t.title.as_str())).collect();
        assert_eq!(
            shape,
            [
                (1, 1, "Have Some More Tea"),
                (1, 2, "Medley"),
                (1, 3, "Victor Henry's Cool Book"),
                (2, 1, "Disc Two Opener")
            ],
            "headings skipped, vinyl sides one disc"
        );
        assert_eq!(r.tracks[1].artist, "Artist Feat. G.");
        assert_eq!(
            (r.tracks[0].duration, r.tracks[1].duration, r.tracks[2].duration),
            (Some(135.0), Some(3723.0), None)
        );
        assert_eq!(r.tracks[2].artist, "The Smoke", "a track without artists is the release's");
    }

    #[test]
    fn reads_discogs_positions_names_and_dates() {
        assert_eq!(
            [dg_disc("A1"), dg_disc("B"), dg_disc("1"), dg_disc("1-3"), dg_disc("CD2-04"), dg_disc("A-B")],
            [1, 1, 1, 1, 2, 1]
        );
        assert_eq!(
            [dg_name("Burial (2)"), dg_name("Four Tet*"), dg_name("Sunn O)))"), dg_name("Band (UK)")],
            ["Burial", "Four Tet", "Sunn O)))", "Band (UK)"]
        );
        assert_eq!(dg_date(Some("2007-11-05"), Some(2007)).as_deref(), Some("2007-11-05"));
        assert_eq!(dg_date(Some("1967-00-00"), Some(1967)).as_deref(), Some("1967"));
        assert_eq!(dg_date(None, Some(1999)).as_deref(), Some("1999"));
        assert_eq!(dg_date(Some(""), Some(0)), None);
    }
}
