//! Which file, of everything a Soulseek search turned up, is the song: right title, right length,
//! a wanted format, and from someone likely to send it soon.

use crate::slskdpp::FolderHit;

/// The song wanted, as Lidarr knows it.
pub struct Want<'a> {
    pub artist: &'a str,
    pub album: &'a str,
    pub title: &'a str,
    /// Milliseconds; 0 when unknown.
    pub duration_ms: u64,
    /// File extensions in order of preference ("flac", "mp3").
    pub formats: &'a [String],
}

/// A file worth trying, best first.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Candidate {
    pub user: String,
    pub path: String,
    pub size: u64,
}

/// How far a file's length may be from the track's.
const DURATION_SLACK_MS: u64 = 3_000;

/// Versions that are another recording than the one asked for, unless the title says so.
const OTHER_VERSIONS: [&str; 12] = [
    "remix",
    "mix",
    "edit",
    "live",
    "instrumental",
    "acapella",
    "demo",
    "version",
    "rework",
    "dub",
    "karaoke",
    "remaster",
];

/// The files that are the song, best first: the preferred format, a length that matches, the
/// album's own folder, then a free upload slot, a short queue and a fast connection.
pub fn candidates(hits: &[FolderHit], want: &Want) -> Vec<Candidate> {
    let title = words(want.title);
    if title.is_empty() {
        return Vec::new();
    }
    let album = words(want.album);
    let artist = words(want.artist);
    let mut found = Vec::new();
    for hit in hits {
        let folder = words(&hit.folder);
        for file in &hit.files {
            let ext = file.ext.to_lowercase();
            let Some(format) = want.formats.iter().position(|f| f.eq_ignore_ascii_case(&ext)) else {
                continue;
            };
            let stem = file.name.rsplit_once('.').map_or(file.name.as_str(), |(stem, _)| stem);
            let name = words(stem);
            if !contains_run(&name, &title) || is_other_version(&name, &title) {
                continue;
            }
            let timing = match (file.duration, want.duration_ms) {
                (_, 0) | (None, _) => Timing::Unknown,
                (Some(seconds), ms) if (u64::from(seconds) * 1000).abs_diff(ms) <= DURATION_SLACK_MS => Timing::Matches,
                _ => continue,
            };
            let rank = Rank {
                format,
                timing,
                outside_album: album.is_empty() || !contains_all(&folder, &album),
                artist_unnamed: artist.is_empty() || !(contains_all(&folder, &artist) || contains_all(&name, &artist)),
                busy: !hit.slot_free,
                queue: hit.queue_length,
                slowness: u32::MAX - hit.upload_speed,
            };
            found.push((rank, Candidate { user: hit.user.clone(), path: file.path.clone(), size: file.size }));
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    // One try per user: if a peer fails, its other copies likely will too.
    let mut users = std::collections::HashSet::new();
    found.into_iter().map(|(_, c)| c).filter(|c| users.insert(c.user.clone())).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Timing {
    Matches,
    Unknown,
}

/// Sorts best first, field by field: for each, less is better.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Rank {
    format: usize,
    timing: Timing,
    outside_album: bool,
    artist_unnamed: bool,
    busy: bool,
    queue: u32,
    slowness: u32,
}

/// Lowercase words, letters and digits only: "Flint (Remix)" → ["flint", "remix"]. Accents stay.
pub fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_lowercase).collect()
}

/// `needle` appears in `haystack` as consecutive words.
fn contains_run(haystack: &[String], needle: &[String]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn contains_all(haystack: &[String], needle: &[String]) -> bool {
    needle.iter().all(|w| haystack.contains(w))
}

fn is_other_version(name: &[String], title: &[String]) -> bool {
    OTHER_VERSIONS.iter().any(|v| name.iter().any(|w| w == v) && !title.iter().any(|w| w == v))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hits() -> Vec<FolderHit> {
        serde_json::from_str(include_str!("../tests/fixtures/hits.json")).unwrap()
    }

    fn formats(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn prefers_the_album_folder_in_the_wanted_format() {
        let formats = formats(&["flac", "mp3"]);
        let want = Want { artist: "Emptyset", album: "Ash", title: "Flint", duration_ms: 169_000, formats: &formats };
        let found = candidates(&hits(), &want);
        let users: Vec<&str> = found.iter().map(|c| c.user.as_str()).collect();
        // The remix and the file a minute too long are left out; mp3 comes last.
        assert_eq!(users, ["ola", "basia", "mp3fan"]);
        assert_eq!(found[0].path, r"@@music\Emptyset\Ash (2023)\01 - Flint.flac");
        assert_eq!(found[0].size, 30_000_000);
    }

    #[test]
    fn keeps_to_the_formats_asked_for() {
        let formats = formats(&["flac"]);
        let want = Want { artist: "Emptyset", album: "Ash", title: "Flint", duration_ms: 169_000, formats: &formats };
        assert!(candidates(&hits(), &want).iter().all(|c| c.path.ends_with(".flac")));
    }

    #[test]
    fn a_remix_is_fine_when_asked_for() {
        let formats = formats(&["flac"]);
        let want = Want { artist: "Emptyset", album: "", title: "Flint (Remix)", duration_ms: 0, formats: &formats };
        let found = candidates(&hits(), &want);
        assert_eq!(found.len(), 1);
        assert!(found[0].path.contains("Remix"));
    }

    #[test]
    fn titles_match_whole_words() {
        assert!(contains_run(&words("01 - Flint.flac"), &words("Flint")));
        assert!(!contains_run(&words("Flintstone"), &words("Flint")));
        assert!(contains_run(&words("Emptyset - Ash - 05 - Ash"), &words("ash")));
        assert_eq!(words("Ça va? (Live, 1999)"), ["ça", "va", "live", "1999"]);
        assert!(
            candidates(&hits(), &Want { artist: "", album: "", title: "  ", duration_ms: 0, formats: &[] }).is_empty()
        );
    }
}
