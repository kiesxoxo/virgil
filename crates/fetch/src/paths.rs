//! The same files under three names: Lidarr's (`/data/Music/Artist/Album/01.flac`), MPD's
//! (`Artist/Album/01.flac`, relative to the music directory) and slskdpp's (relative to its
//! downloads directory, which Lidarr sees elsewhere).

/// The MPD path of a file Lidarr keeps under `root` (its root folder, which is MPD's music
/// directory), if it is under it.
pub fn in_library(root: &str, path: &str) -> Option<String> {
    let root = root.trim_end_matches('/');
    let rest = path.strip_prefix(root)?.strip_prefix('/')?;
    (!rest.is_empty()).then(|| rest.to_string())
}

/// The top folder of an MPD path: the artist's, in Lidarr's layout. Rescanning it picks up a new
/// album with all its discs.
pub fn artist_folder(uri: &str) -> &str {
    uri.split('/').next().unwrap_or(uri)
}

/// Where Lidarr finds a file slskdpp downloaded, given where Lidarr sees slskdpp's downloads
/// directory and the download's folder and name (relative to that directory).
pub fn downloaded(downloads: &str, folder: &str, name: &str) -> String {
    let mut path = downloads.trim_end_matches('/').to_string();
    for part in [folder, name] {
        let part = part.trim_matches('/');
        if !part.is_empty() && part != "." {
            path.push('/');
            path.push_str(part);
        }
    }
    path
}

/// The folder a path is in.
pub fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_lidarr_paths_to_mpd() {
        let file = "/data/Music/Emptyset/Ash (2023)/Emptyset - Ash - 01 - Flint.flac";
        assert_eq!(in_library("/data/Music", file).unwrap(), "Emptyset/Ash (2023)/Emptyset - Ash - 01 - Flint.flac");
        assert_eq!(in_library("/data/Music/", file).unwrap(), "Emptyset/Ash (2023)/Emptyset - Ash - 01 - Flint.flac");
        assert_eq!(in_library("/data/Musique", file), None);
        assert_eq!(in_library("/data/Mus", file), None);
        assert_eq!(in_library("/data/Music", "/data/Music/"), None);
        assert_eq!(artist_folder("Emptyset/Ash (2023)/01.flac"), "Emptyset");
    }

    #[test]
    fn finds_downloads_where_lidarr_sees_them() {
        assert_eq!(
            downloaded("/data/Downloads/slskdpp/", "Ash (2023)", "01 - Flint.flac"),
            "/data/Downloads/slskdpp/Ash (2023)/01 - Flint.flac"
        );
        assert_eq!(downloaded("/data/Downloads/slskdpp", ".", "a.flac"), "/data/Downloads/slskdpp/a.flac");
        assert_eq!(parent("/data/Downloads/slskdpp/Ash (2023)/01.flac"), "/data/Downloads/slskdpp/Ash (2023)");
    }
}
