//! Turning MPD's flat song list into albums.

use std::collections::{HashMap, HashSet};

use virgil_core::{AudioFormat, Disc, unix_time};
use virgil_engine_mpd::Song;

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
    pub discs: Vec<Disc>,
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

/// Bumped whenever [`album_key`] or the disc handling changes, so existing libraries are regrouped.
pub const GROUPING_VERSION: u32 = 4;

/// What one disc of a release is called: the generic words, and the medium formats that
/// Lidarr names disc folders after (`CD 01`, `12" Vinyl 02`, `Cassette 03`, `Digital Media 01`).
const MEDIA: [&str; 26] = [
    "cd",
    "disc",
    "disk",
    "lp",
    "vinyl",
    "cassette",
    "tape",
    "digital media",
    "enhanced cd",
    "copy control cd",
    "blu-spec cd",
    "shm-cd",
    "hqcd",
    "hdcd",
    "data cd",
    "cd-r",
    "sacd",
    "hybrid sacd",
    "dualdisc",
    "dvd",
    "dvd-audio",
    "dvd-video",
    "blu-ray",
    "bluray",
    "minidisc",
    "shellac",
];

/// `cd`, `cassette`, `12" vinyl`, `8cm cd`: a medium, possibly after its size.
fn is_medium(name: &str) -> bool {
    let after_size = name.trim_start_matches(|c: char| c.is_ascii_digit());
    let name = if after_size.len() < name.len() {
        let unit = after_size.trim_start_matches(['"', '″', '\'']);
        let unit = ["-inch", "inch", "in", "cm"].iter().find_map(|u| unit.strip_prefix(u)).unwrap_or(unit);
        unit.trim_start_matches([' ', '-'])
    } else {
        name
    };
    MEDIA.contains(&name)
}

/// `CD1`, `Disc 2`, `disk_03`, `LP 2 of 2`, `Cassette 01`, `12" Vinyl 02`: a name that is just
/// one disc of a release, and its number.
fn disc_number(name: &str) -> Option<u32> {
    let lower = name.trim().to_lowercase().replace('_', " ");
    let (head, total) = lower.split_once(" of ").unwrap_or((&lower, "1"));
    let medium = head.trim_end_matches(|c: char| c.is_ascii_digit());
    let n = &head[medium.len()..];
    let medium = medium.trim_end_matches([' ', '-', '.', '#']);
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if digits(n) && digits(total) && is_medium(medium) { n.parse().ok().filter(|&n| n > 0) } else { None }
}

/// "Box (Disc 2)", "Box [CD2]", "Box - LP 2": the name before a trailing disc
/// marker, and the disc's number.
fn split_disc(name: &str) -> Option<(&str, u32)> {
    let name = name.trim_end();
    let (base, marker) = match name.strip_suffix([')', ']']) {
        Some(inner) => inner.rfind(['(', '[']).map(|open| (&inner[..open], &inner[open + 1..]))?,
        None => name
            .match_indices([' ', '_', '-', ','])
            .map(|(i, sep)| (&name[..i], &name[i + sep.len()..]))
            .find(|(_, marker)| disc_number(marker).is_some())?,
    };
    let disc = disc_number(marker)?;
    let base = base.trim_end_matches([' ', '_', '-', ',', ':', '–', '—']);
    (!base.is_empty()).then_some((base, disc))
}

/// An album title without a trailing disc marker.
fn release_title(album: &str) -> &str {
    split_disc(album).map_or(album, |(base, _)| base)
}

/// The folder an album lives in. A disc folder ("CD2") counts as its parent,
/// and a folder named for one disc ("Box (Disc 2)") as the release's ("Box").
fn album_dir(uri: &str) -> &str {
    let dir = parent_dir(uri);
    let (parent, last) = dir.rsplit_once('/').unwrap_or(("", dir));
    if disc_number(last).is_some() {
        parent
    } else if let Some((base, _)) = split_disc(last) {
        &dir[..dir.len() - last.len() + base.len()]
    } else {
        dir
    }
}

/// The disc a song is on: its disc tag, else a number in its folder's name or album title.
fn disc_of(song: &Song) -> Option<u32> {
    let folder = parent_dir(&song.uri).rsplit('/').next().unwrap_or("");
    song.disc
        .or_else(|| disc_number(folder))
        .or_else(|| split_disc(folder).map(|(_, n)| n))
        .or_else(|| split_disc(song.album.as_deref()?).map(|(_, n)| n))
}

/// Which album a song belongs to: its folder plus the album title. Tags such as
/// album artist are often inconsistent within one album, so they don't split it.
fn album_key(song: &Song) -> String {
    let dir = album_dir(&song.uri);
    match &song.album {
        Some(album) => format!("{dir}\u{1f}{}", release_title(album).to_lowercase()),
        None => dir.to_string(),
    }
}

fn folder_of(tracks: &[Song]) -> Option<String> {
    Some(album_dir(&tracks[0].uri).to_string())
}

fn release_of(tracks: &[Song]) -> Option<String> {
    let id = tracks[0].mb_album_id.as_ref()?;
    tracks.iter().all(|t| t.mb_album_id.as_ref() == Some(id)).then(|| id.clone())
}

/// Joins the discs of a release that are tagged as different albums, like a
/// 2×LP whose second record has a title of its own: groups in one folder, or
/// from one MusicBrainz release, that all have disc numbers and share none.
fn join_discs(groups: HashMap<String, Vec<Song>>) -> Vec<(String, Vec<Song>)> {
    let mut groups: Vec<Option<(String, Vec<Song>)>> = groups.into_iter().map(Some).collect();
    let discs = |tracks: &[Song]| tracks.iter().filter_map(|t| t.disc).collect::<HashSet<_>>();
    for cluster_of in [folder_of as fn(&[Song]) -> Option<String>, release_of] {
        let mut clusters: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, group) in groups.iter().enumerate() {
            let Some((_, tracks)) = group else { continue };
            if tracks.iter().all(|t| t.disc.is_some())
                && let Some(cluster) = cluster_of(tracks)
            {
                clusters.entry(cluster).or_default().push(i);
            }
        }
        for mut members in clusters.into_values().filter(|m| m.len() > 1) {
            let sets: Vec<_> = members.iter().map(|&i| discs(&groups[i].as_ref().expect("member").1)).collect();
            let all: HashSet<_> = sets.iter().flatten().collect();
            if all.len() != sets.iter().map(HashSet::len).sum::<usize>() {
                continue;
            }
            // The album keeps the key (so the id and artwork) of its first disc.
            members.sort_by_key(|&i| groups[i].as_ref().expect("member").1.iter().filter_map(|t| t.disc).min());
            let rest: Vec<Song> = members[1..].iter().flat_map(|&i| groups[i].take().expect("member").1).collect();
            groups[members[0]].as_mut().expect("member").1.extend(rest);
        }
    }
    groups.into_iter().flatten().collect()
}

/// The discs of an album's (sorted) tracks, each titled by its own album tag
/// where that isn't the album's `title`.
fn discs(tracks: &[Song], title: &str) -> Vec<Disc> {
    let title = title.to_lowercase();
    let mut discs: Vec<Disc> = Vec::new();
    for track in tracks {
        let number = track.disc.unwrap_or(1);
        if discs.last().is_some_and(|d| d.number == number) {
            continue;
        }
        let own = track.album.as_deref().map(release_title).filter(|t| t.to_lowercase() != title);
        discs.push(Disc { number, title: own.map(str::to_string) });
    }
    discs
}

fn year(song: &Song) -> Option<u32> {
    let date = song.original_date.as_deref().or(song.date.as_deref())?;
    date.get(..4)?.parse().ok().filter(|&y| y > 0)
}

pub fn group(songs: Vec<Song>) -> Vec<AlbumGroup> {
    let mut groups: HashMap<String, Vec<Song>> = HashMap::new();
    for mut song in songs {
        song.disc = disc_of(&song);
        groups.entry(album_key(&song)).or_default().push(song);
    }
    join_discs(groups)
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
            let several_discs = first.disc != tracks[tracks.len() - 1].disc;
            let title = match &first.album {
                Some(album) if several_discs => release_title(album).to_string(),
                Some(album) => album.clone(),
                None => album_dir(&first.uri)
                    .rsplit('/')
                    .next()
                    .filter(|s| !s.is_empty())
                    .unwrap_or("Unknown Album")
                    .to_string(),
            };
            let discs = discs(&tracks, &title);
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
                added: tracks.iter().filter_map(|t| t.last_modified.as_deref().and_then(unix_time)).max().unwrap_or(0),
                discs,
                tracks,
            }
        })
        .collect()
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
    fn merges_disc_subfolders() {
        let groups = group(vec![
            song("X/CD1/01.flac", Some("Box"), Some("Band"), "Band", 1),
            song("X/Disc 2/01.flac", Some("Box"), Some("Band"), "Band", 1),
        ]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].artist, "Band");
    }

    #[test]
    fn inconsistent_album_artist_tags_do_not_split_an_album() {
        let groups = group(vec![
            song("36/Void Dance/01.flac", Some("Void Dance"), Some("36"), "36", 1),
            song("36/Void Dance/11.flac", Some("Void Dance"), None, "36", 11),
            song("36/Void Dance/12.flac", Some("void dance"), Some("36 & Friend"), "36", 12),
        ]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].tracks.len(), 3);
        assert_eq!(groups[0].artist, "36");
    }

    #[test]
    fn recognises_disc_folders() {
        for (name, n) in
            [("CD1", 1), ("cd 2", 2), ("Disc 3", 3), ("disk_04", 4), ("DISC-1", 1), ("LP2", 2), ("Vinyl 1", 1)]
        {
            assert_eq!(disc_number(name), Some(n), "{name}");
        }
        assert_eq!(disc_number("Disc 2 of 2"), Some(2));
        for name in ["CD", "Discography", "Disco Inferno", "2014", "CDs", "LPs", "CD0"] {
            assert_eq!(disc_number(name), None, "{name}");
        }
        assert_eq!(album_dir("A/B/CD2/01.flac"), "A/B");
        assert_eq!(album_dir("A/B/01.flac"), "A/B");
        assert_eq!(album_dir("A/Box (Disc 2)/01.flac"), "A/Box");
    }

    #[test]
    fn recognises_lidarr_medium_folders() {
        for (name, n) in [
            ("CD 01", 1),
            ("Cassette 03", 3),
            ("12 Vinyl 02", 2),
            ("12\" Vinyl 01", 1),
            ("7in Vinyl 2", 2),
            ("Digital Media 01", 1),
            ("Enhanced CD 02", 2),
            ("Hybrid SACD 1", 1),
            ("Blu-ray 01", 1),
            ("8cm CD 3", 3),
        ] {
            assert_eq!(disc_number(name), Some(n), "{name}");
        }
        // Numbered folders that aren't one disc of something.
        for name in ["Forse 1", "Part 2", "Volume 3", "Side 1", "Greatest Hits CD 2", "12 34", "Tape"] {
            assert_eq!(disc_number(name), None, "{name}");
        }
        assert_eq!(album_dir("A/Fantasy (2025)/Cassette 02/01.flac"), "A/Fantasy (2025)");
        assert_eq!(album_dir("A/Forse 1 (2013)/12 Vinyl 01/01.flac"), "A/Forse 1 (2013)");
    }

    #[test]
    fn untagged_discs_in_medium_folders_make_one_album() {
        // Fantasy: three cassettes, numbered 1–40 straight through, no disc tags.
        let dir = "DJ Sabrina/Fantasy (2025)";
        let groups = group(vec![
            song(&format!("{dir}/Cassette 03/01.flac"), Some("Fantasy"), None, "DJ Sabrina", 30),
            song(&format!("{dir}/Cassette 01/02.flac"), Some("Fantasy"), None, "DJ Sabrina", 2),
            song(&format!("{dir}/Cassette 02/01.flac"), Some("Fantasy"), None, "DJ Sabrina", 15),
            song(&format!("{dir}/Cassette 01/01.flac"), Some("Fantasy"), None, "DJ Sabrina", 1),
        ]);
        assert_eq!(groups.len(), 1);
        let order: Vec<_> = groups[0].tracks.iter().map(|t| (t.disc, t.track)).collect();
        assert_eq!(order, [(Some(1), Some(1)), (Some(1), Some(2)), (Some(2), Some(15)), (Some(3), Some(30))]);
        assert_eq!(groups[0].discs.len(), 3);
        assert_eq!(groups[0].title, "Fantasy");
    }

    #[test]
    fn numbered_albums_side_by_side_stay_apart() {
        let groups = group(vec![
            song("Cortini/Forse 1/01.flac", Some("Forse 1"), None, "Cortini", 1),
            song("Cortini/Forse 2/01.flac", Some("Forse 2"), None, "Cortini", 1),
        ]);
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn splits_disc_markers_off_titles() {
        assert_eq!(split_disc("Box (Disc 2)"), Some(("Box", 2)));
        assert_eq!(split_disc("Box [CD1]"), Some(("Box", 1)));
        assert_eq!(split_disc("Box - LP 2"), Some(("Box", 2)));
        assert_eq!(split_disc("Box CD2"), Some(("Box", 2)));
        assert_eq!(split_disc("Box, Disc 1 of 2"), Some(("Box", 1)));
        for name in ["LP1", "Disc 2", "In Rainbows", "Box (Remastered)", "Discovery", "Box (2014)"] {
            assert_eq!(split_disc(name), None, "{name}");
        }
    }

    fn on_disc(mut song: Song, disc: u32) -> Song {
        song.disc = Some(disc);
        song
    }

    #[test]
    fn joins_discs_titled_as_separate_albums() {
        let dir = "RSE/2022 - Flying Fish Ambience + Saga of Capsizing Dolphins";
        let songs = vec![
            on_disc(song(&format!("{dir}/06.flac"), Some("Saga of Capsizing Dolphins"), None, "RSE", 1), 2),
            on_disc(song(&format!("{dir}/01.flac"), Some("Flying Fish Ambience"), None, "RSE", 1), 1),
            on_disc(song(&format!("{dir}/07.flac"), Some("Saga of Capsizing Dolphins"), None, "RSE", 2), 2),
            on_disc(song(&format!("{dir}/02.flac"), Some("Flying Fish Ambience"), None, "RSE", 2), 1),
            on_disc(song(&format!("{dir}/03.flac"), Some("Flying Fish Ambience"), None, "RSE", 3), 1),
        ];
        let groups = group(songs);
        assert_eq!(groups.len(), 1);
        let album = &groups[0];
        assert_eq!(album.title, "Flying Fish Ambience");
        let uris: Vec<_> = album.tracks.iter().map(|t| &t.uri[dir.len() + 1..]).collect();
        assert_eq!(uris, ["01.flac", "02.flac", "03.flac", "06.flac", "07.flac"]);
        assert_eq!(
            album.discs,
            [Disc { number: 1, title: None }, Disc { number: 2, title: Some("Saga of Capsizing Dolphins".into()) }]
        );
        // Disc 1's id, so its artwork stays put.
        let disc1 = format!("{dir}\u{1f}flying fish ambience");
        assert_eq!(album.id, format!("{:016x}", fnv1a64(disc1.as_bytes())));
    }

    #[test]
    fn keeps_albums_apart_when_their_discs_overlap() {
        let groups = group(vec![
            on_disc(song("Mix/01.flac", Some("One"), None, "A", 1), 1),
            on_disc(song("Mix/02.flac", Some("Two"), None, "B", 1), 1),
            song("Mix/03.flac", Some("Three"), None, "C", 1),
        ]);
        assert_eq!(groups.len(), 3);
    }

    #[test]
    fn disc_markers_in_folders_and_titles_make_one_album() {
        let groups = group(vec![
            song("A/Box (CD2)/01.flac", Some("Box (Disc 2)"), None, "A", 1),
            song("A/Box (CD1)/02.flac", Some("Box (Disc 1)"), None, "A", 2),
            song("A/Box (CD1)/01.flac", Some("Box (Disc 1)"), None, "A", 1),
        ]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].title, "Box");
        let order: Vec<_> = groups[0].tracks.iter().map(|t| (t.disc, t.track)).collect();
        assert_eq!(order, [(Some(1), Some(1)), (Some(1), Some(2)), (Some(2), Some(1))]);
        assert!(groups[0].discs.iter().all(|d| d.title.is_none()));

        // A lone disc keeps its full title.
        let groups = group(vec![song("B/Box (Disc 2)/01.flac", Some("Box (Disc 2)"), None, "B", 1)]);
        assert_eq!(groups[0].title, "Box (Disc 2)");
    }

    #[test]
    fn disc_folders_number_untagged_discs() {
        let groups = group(vec![
            song("X/CD2/01.flac", Some("Box"), None, "Band", 1),
            song("X/CD1/02.flac", Some("Box"), None, "Band", 2),
            song("X/CD1/01.flac", Some("Box"), None, "Band", 1),
        ]);
        let uris: Vec<_> = groups[0].tracks.iter().map(|t| t.uri.as_str()).collect();
        assert_eq!(uris, ["X/CD1/01.flac", "X/CD1/02.flac", "X/CD2/01.flac"]);
        assert_eq!(groups[0].discs.len(), 2);
    }

    #[test]
    fn joins_discs_of_one_musicbrainz_release() {
        let tagged = |uri: &str, album: &str, disc: u32| Song {
            mb_album_id: Some("a1b2".into()),
            ..on_disc(song(uri, Some(album), None, "A", 1), disc)
        };
        let groups =
            group(vec![tagged("A/Side One/01.flac", "Side One", 1), tagged("A/Side Two/01.flac", "Side Two", 2)]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].title, "Side One");

        // Two copies of the same disc are versions, not discs.
        let groups = group(vec![tagged("A/FLAC/01.flac", "Same", 1), tagged("A/MP3/01.mp3", "Same", 1)]);
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn sort_keys_ignore_articles() {
        assert_eq!(sort_key("The Beatles"), "beatles");
        assert_eq!(sort_key("Theo"), "theo");
    }
}
