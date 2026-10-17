//! Turning MPD's flat song list into albums.

use std::collections::HashMap;

use hifi_core::AudioFormat;
use hifi_engine_mpd::Song;

pub struct AlbumGroup {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub duration: f64,
    pub format: Option<AudioFormat>,
    pub added: u32,
    /// In play order: disc, track number, then path.
    pub tracks: Vec<Song>,
}

pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

fn parent_dir(uri: &str) -> &str {
    uri.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// Which album a song belongs to. Album artist + album when tagged; otherwise
/// album + directory, so untagged compilations don't split per track artist.
fn album_key(song: &Song) -> String {
    if let Some(mbid) = &song.mb_album_id {
        return format!("mb:{mbid}");
    }
    let dir = parent_dir(&song.uri);
    match (&song.album, &song.album_artist) {
        (Some(album), Some(artist)) => {
            format!("aa:{}\u{1f}{}", artist.to_lowercase(), album.to_lowercase())
        }
        (Some(album), None) => format!("dir:{dir}\u{1f}{}", album.to_lowercase()),
        (None, _) => format!("dir:{dir}"),
    }
}

fn year(song: &Song) -> Option<u32> {
    let date = song.original_date.as_deref().or(song.date.as_deref())?;
    date.get(..4)?.parse().ok().filter(|&y| y > 0)
}

pub fn group(songs: Vec<Song>) -> Vec<AlbumGroup> {
    let mut groups: HashMap<String, Vec<Song>> = HashMap::new();
    for song in songs {
        groups.entry(album_key(&song)).or_default().push(song);
    }
    groups
        .into_iter()
        .map(|(key, mut tracks)| {
            tracks.sort_by(|a, b| {
                (a.disc.unwrap_or(1), a.track.unwrap_or(u32::MAX), &a.uri).cmp(&(
                    b.disc.unwrap_or(1),
                    b.track.unwrap_or(u32::MAX),
                    &b.uri,
                ))
            });
            let first = &tracks[0];
            let title = first.album.clone().unwrap_or_else(|| {
                let dir = parent_dir(&first.uri);
                dir.rsplit('/').next().filter(|s| !s.is_empty()).unwrap_or("Unknown Album").to_string()
            });
            let artist = tracks
                .iter()
                .find_map(|t| t.album_artist.clone())
                .or_else(|| {
                    let artist = first.artist.as_ref()?;
                    tracks.iter().all(|t| t.artist.as_ref() == Some(artist)).then(|| artist.clone())
                })
                .unwrap_or_else(|| {
                    if tracks.iter().any(|t| t.artist.is_some()) { "Various Artists" } else { "Unknown Artist" }
                        .to_string()
                });
            let format = first.format.filter(|f| tracks.iter().all(|t| t.format.as_ref() == Some(f)));
            AlbumGroup {
                id: format!("{:016x}", fnv1a64(key.as_bytes())),
                title,
                artist,
                year: tracks.iter().filter_map(year).min(),
                genre: tracks.iter().find_map(|t| t.genre.clone()),
                duration: tracks.iter().map(|t| t.duration).sum(),
                format,
                added: tracks
                    .iter()
                    .filter_map(|t| t.last_modified.as_deref().and_then(parse_rfc3339))
                    .max()
                    .unwrap_or(0),
                tracks,
            }
        })
        .collect()
}

/// `2024-03-01T12:34:56Z` → Unix seconds. MPD always prints UTC.
pub fn parse_rfc3339(s: &str) -> Option<u32> {
    let num = |range: std::ops::Range<usize>| s.get(range)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, se) = (num(11..13)?, num(14..16)?, num(17..19)?);
    // Days from civil, after Howard Hinnant.
    let y = if mo <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((mo + 9) % 12) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u32::try_from(days * 86_400 + h * 3_600 + mi * 60 + se).ok()
}

/// Sort key: case-insensitive, leading "The " ignored.
pub fn sort_key(s: &str) -> String {
    let lower = s.to_lowercase();
    lower.strip_prefix("the ").map(str::to_string).unwrap_or(lower)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(uri: &str, album: Option<&str>, album_artist: Option<&str>, artist: &str, track: u32) -> Song {
        Song {
            uri: uri.into(),
            album: album.map(Into::into),
            album_artist: album_artist.map(Into::into),
            artist: Some(artist.into()),
            track: Some(track),
            duration: 100.0,
            ..Song::default()
        }
    }

    #[test]
    fn groups_compilations_by_directory() {
        let groups = group(vec![
            song("Comp/02.flac", Some("Hits"), None, "B", 2),
            song("Comp/01.flac", Some("Hits"), None, "A", 1),
            song("Other/01.flac", Some("Hits"), None, "C", 1),
        ]);
        assert_eq!(groups.len(), 2);
        let comp = groups.iter().find(|g| g.tracks.len() == 2).unwrap();
        assert_eq!(comp.artist, "Various Artists");
        assert_eq!(comp.tracks[0].uri, "Comp/01.flac");
        assert_eq!(comp.duration, 200.0);
    }

    #[test]
    fn groups_by_album_artist_across_directories() {
        let groups = group(vec![
            song("X/CD1/01.flac", Some("Box"), Some("Band"), "Band", 1),
            song("X/CD2/01.flac", Some("Box"), Some("Band"), "Band", 1),
        ]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].artist, "Band");
    }

    #[test]
    fn parses_mpd_timestamps() {
        assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_rfc3339("2024-03-01T12:34:56Z"), Some(1_709_296_496));
        assert_eq!(parse_rfc3339("nope"), None);
    }

    #[test]
    fn sort_keys_ignore_articles() {
        assert_eq!(sort_key("The Beatles"), "beatles");
        assert_eq!(sort_key("Theo"), "theo");
    }
}
