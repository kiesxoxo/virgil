//! SQLite projection of MPD's database: albums, tracks, FTS5 search, artwork.

use std::collections::HashMap;

use hifi_core::{Album, AlbumDetail, AudioFormat, LibraryStatus, SearchResults, Track, TrackHit};
use hifi_engine_mpd::Song;
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};

use crate::group::{self, fnv1a64, sort_key};

const SCHEMA: &str = "
PRAGMA synchronous = NORMAL;
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value INTEGER NOT NULL);
INSERT OR IGNORE INTO meta (key, value) VALUES ('revision', 0), ('fingerprint', 0);
CREATE TABLE IF NOT EXISTS albums (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    artist TEXT NOT NULL,
    year INTEGER,
    genre TEXT,
    tracks INTEGER NOT NULL,
    duration REAL NOT NULL,
    format TEXT,
    added INTEGER NOT NULL,
    cover_uri TEXT NOT NULL,
    sort_artist TEXT NOT NULL,
    sort_title TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS tracks (
    uri TEXT PRIMARY KEY,
    album_id TEXT NOT NULL,
    title TEXT NOT NULL,
    artist TEXT NOT NULL,
    disc INTEGER,
    track INTEGER,
    duration REAL NOT NULL,
    format TEXT
);
CREATE INDEX IF NOT EXISTS tracks_by_album ON tracks (album_id);
-- Survives rebuilds: `checked` is when the album's art was last looked up.
CREATE TABLE IF NOT EXISTS art (album_id TEXT PRIMARY KEY, art TEXT, checked INTEGER NOT NULL);
CREATE VIRTUAL TABLE IF NOT EXISTS fts USING fts5 (
    kind UNINDEXED, ref UNINDEXED, body, tokenize = 'unicode61 remove_diacritics 2'
);
";

const ALBUM_COLUMNS: &str =
    "a.id, a.title, a.artist, a.year, a.genre, a.tracks, a.duration, art.art, a.format, a.added";
const TRACK_COLUMNS: &str = "t.uri, t.title, t.artist, t.disc, t.track, t.duration, t.format";
const TRACK_ORDER: &str = "COALESCE(t.disc, 1), COALESCE(t.track, 4294967295), t.uri";

#[derive(Default)]
pub struct Index {
    pub album_of: HashMap<String, String>,
    pub art_of: HashMap<String, String>,
}

pub fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
    conn.execute_batch(SCHEMA)
}

fn meta(conn: &Connection, key: &str) -> rusqlite::Result<i64> {
    conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
}

pub fn bump_revision(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute("UPDATE meta SET value = value + 1 WHERE key = 'revision'", []).map(drop)
}

fn fingerprint(songs: &[Song]) -> i64 {
    let mut lines: Vec<String> = songs
        .iter()
        .map(|s| format!("{}\0{}\0{:?}", s.uri, s.last_modified.as_deref().unwrap_or(""), s.format))
        .collect();
    lines.sort_unstable();
    // A new grouping algorithm must regroup an unchanged library.
    lines.push(format!("grouping {}", group::GROUPING_VERSION));
    fnv1a64(lines.join("\n").as_bytes()) as i64
}

/// Replaces albums, tracks and the search index with `songs`. Returns false
/// (and leaves the revision alone) when nothing changed since the last run.
pub fn project(conn: &mut Connection, songs: Vec<Song>) -> rusqlite::Result<bool> {
    let fingerprint = fingerprint(&songs);
    if meta(conn, "fingerprint")? == fingerprint {
        return Ok(false);
    }
    let albums = group::group(songs);
    let tx = conn.transaction()?;
    tx.execute_batch("DELETE FROM tracks; DELETE FROM albums; DELETE FROM fts;")?;
    insert_albums(&tx, &albums)?;
    tx.execute("DELETE FROM art WHERE album_id NOT IN (SELECT id FROM albums)", [])?;
    tx.execute("UPDATE meta SET value = ?1 WHERE key = 'fingerprint'", [fingerprint])?;
    bump_revision(&tx)?;
    tx.commit()?;
    Ok(true)
}

fn insert_albums(tx: &Transaction, albums: &[group::AlbumGroup]) -> rusqlite::Result<()> {
    let mut album_stmt = tx.prepare(
        "INSERT INTO albums (id, title, artist, year, genre, tracks, duration, format, added, cover_uri, sort_artist, sort_title)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
    )?;
    let mut track_stmt = tx.prepare(
        "INSERT OR IGNORE INTO tracks (uri, album_id, title, artist, disc, track, duration, format)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?;
    let mut fts_stmt = tx.prepare("INSERT INTO fts (kind, ref, body) VALUES (?1, ?2, ?3)")?;
    for album in albums {
        album_stmt.execute(params![
            album.id,
            album.title,
            album.artist,
            album.year,
            album.genre,
            album.tracks.len() as u32,
            album.duration,
            album.format.map(|f| f.to_mpd()),
            album.added,
            album.tracks[0].uri,
            sort_key(&album.artist),
            sort_key(&album.title),
        ])?;
        let year = album.year.map(|y| y.to_string()).unwrap_or_default();
        fts_stmt.execute(params!["album", album.id, format!("{} {} {year}", album.title, album.artist)])?;
        for song in &album.tracks {
            let title = song.display_title();
            let artist = song.artist.clone().or_else(|| song.album_artist.clone()).unwrap_or_default();
            track_stmt.execute(params![
                song.uri,
                album.id,
                title,
                artist,
                song.disc,
                song.track,
                song.duration,
                song.format.map(|f| f.to_mpd()),
            ])?;
            fts_stmt.execute(params!["track", song.uri, format!("{title} {artist} {}", album.title)])?;
        }
    }
    Ok(())
}

fn format_column(row: &Row, idx: usize) -> rusqlite::Result<Option<AudioFormat>> {
    Ok(row.get::<_, Option<String>>(idx)?.as_deref().and_then(AudioFormat::parse_mpd))
}

fn album_from_row(row: &Row) -> rusqlite::Result<Album> {
    Ok(Album {
        id: row.get(0)?,
        title: row.get(1)?,
        artist: row.get(2)?,
        year: row.get(3)?,
        genre: row.get(4)?,
        tracks: row.get(5)?,
        duration: row.get(6)?,
        art: row.get(7)?,
        format: format_column(row, 8)?,
        added: row.get(9)?,
    })
}

/// Reads [`TRACK_COLUMNS`] starting at `at`.
fn track_from_row(row: &Row, at: usize) -> rusqlite::Result<Track> {
    Ok(Track {
        uri: row.get(at)?,
        title: row.get(at + 1)?,
        artist: row.get(at + 2)?,
        disc: row.get(at + 3)?,
        track: row.get(at + 4)?,
        duration: row.get(at + 5)?,
        format: format_column(row, at + 6)?,
    })
}

pub fn albums(conn: &Connection) -> rusqlite::Result<Vec<Album>> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {ALBUM_COLUMNS} FROM albums a LEFT JOIN art ON art.album_id = a.id
         ORDER BY a.sort_artist, a.year, a.sort_title"
    ))?;
    stmt.query_map([], album_from_row)?.collect()
}

fn album_by_id(conn: &Connection, id: &str) -> rusqlite::Result<Option<Album>> {
    conn.prepare_cached(&format!(
        "SELECT {ALBUM_COLUMNS} FROM albums a LEFT JOIN art ON art.album_id = a.id WHERE a.id = ?1"
    ))?
    .query_row([id], album_from_row)
    .optional()
}

pub fn album(conn: &Connection, id: &str) -> rusqlite::Result<Option<AlbumDetail>> {
    let Some(album) = album_by_id(conn, id)? else { return Ok(None) };
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {TRACK_COLUMNS} FROM tracks t WHERE t.album_id = ?1 ORDER BY {TRACK_ORDER}"
    ))?;
    let tracks = stmt.query_map([id], |row| track_from_row(row, 0))?.collect::<rusqlite::Result<_>>()?;
    Ok(Some(AlbumDetail { album, tracks }))
}

pub fn album_uris(conn: &Connection, id: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt =
        conn.prepare_cached(&format!("SELECT t.uri FROM tracks t WHERE t.album_id = ?1 ORDER BY {TRACK_ORDER}"))?;
    stmt.query_map([id], |row| row.get(0))?.collect()
}

/// Every word becomes a prefix match: `pink flo` finds "Pink Floyd".
fn fts_query(q: &str) -> Option<String> {
    let terms: Vec<String> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{}\"*", t.to_lowercase()))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

pub fn search(conn: &Connection, q: &str) -> rusqlite::Result<SearchResults> {
    let Some(query) = fts_query(q) else { return Ok(SearchResults::default()) };
    let album_ids: Vec<String> = conn
        .prepare_cached("SELECT ref FROM fts WHERE fts MATCH ?1 AND kind = 'album' ORDER BY rank LIMIT 24")?
        .query_map([&query], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut albums = Vec::with_capacity(album_ids.len());
    for id in &album_ids {
        albums.extend(album_by_id(conn, id)?);
    }
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {TRACK_COLUMNS}, a.id, a.title, art.art
         FROM fts JOIN tracks t ON t.uri = fts.ref JOIN albums a ON a.id = t.album_id
         LEFT JOIN art ON art.album_id = a.id
         WHERE fts MATCH ?1 AND fts.kind = 'track' ORDER BY fts.rank LIMIT 50"
    ))?;
    let tracks = stmt
        .query_map([&query], |row| {
            Ok(TrackHit {
                track: track_from_row(row, 0)?,
                album_id: row.get(7)?,
                album_title: row.get(8)?,
                art: row.get(9)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(SearchResults { albums, tracks })
}

/// Albums whose artwork was never looked up, or whose files changed since; newest first.
pub fn art_todo(conn: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.cover_uri FROM albums a LEFT JOIN art ON art.album_id = a.id
         WHERE art.album_id IS NULL OR art.checked < a.added ORDER BY a.added DESC",
    )?;
    stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?.collect()
}

pub fn set_art(conn: &Connection, album_id: &str, art: Option<&str>, checked: u32) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO art (album_id, art, checked) VALUES (?1, ?2, ?3)
         ON CONFLICT (album_id) DO UPDATE SET art = excluded.art, checked = excluded.checked",
        params![album_id, art, checked],
    )
    .map(drop)
}

pub fn status(conn: &Connection) -> rusqlite::Result<LibraryStatus> {
    let count = |table: &str| conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get::<_, u32>(0));
    Ok(LibraryStatus {
        revision: meta(conn, "revision")? as u32,
        scanning: false,
        albums: count("albums")?,
        tracks: count("tracks")?,
    })
}

pub fn load_index(conn: &Connection) -> rusqlite::Result<Index> {
    let album_of = conn
        .prepare("SELECT uri, album_id FROM tracks")?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let art_of = conn
        .prepare("SELECT album_id, art FROM art WHERE art IS NOT NULL")?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Index { album_of, art_of })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(uri: &str, album: &str, artist: &str, title: &str, track: u32) -> Song {
        Song {
            uri: uri.into(),
            album: Some(album.into()),
            album_artist: Some(artist.into()),
            artist: Some(artist.into()),
            title: Some(title.into()),
            track: Some(track),
            date: Some("1973-03-01".into()),
            duration: 200.0,
            format: AudioFormat::parse_mpd("96000:24:2"),
            last_modified: Some("2024-01-01T00:00:00Z".into()),
            ..Song::default()
        }
    }

    fn library() -> (Connection, Vec<Song>) {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        let songs = vec![
            song("PF/DSOTM/02.flac", "The Dark Side of the Moon", "Pink Floyd", "Breathe", 2),
            song("PF/DSOTM/01.flac", "The Dark Side of the Moon", "Pink Floyd", "Speak to Me", 1),
            song("Björk/Post/01.flac", "Post", "Björk", "Army of Me", 1),
        ];
        (conn, songs)
    }

    #[test]
    fn projects_queries_and_skips_unchanged_scans() {
        let (mut conn, songs) = library();
        assert!(project(&mut conn, songs.clone()).unwrap());
        assert!(!project(&mut conn, songs).unwrap(), "same songs must not bump the revision");

        let st = status(&conn).unwrap();
        assert_eq!((st.revision, st.albums, st.tracks), (1, 2, 3));

        let albums = albums(&conn).unwrap();
        assert_eq!(albums[0].artist, "Björk");
        let dsotm = &albums[1];
        assert_eq!((dsotm.year, dsotm.tracks), (Some(1973), 2));
        assert_eq!(dsotm.format, AudioFormat::parse_mpd("96000:24:2"));

        let detail = album(&conn, &dsotm.id).unwrap().unwrap();
        assert_eq!(detail.tracks[0].title, "Speak to Me");
        assert_eq!(album_uris(&conn, &dsotm.id).unwrap(), ["PF/DSOTM/01.flac", "PF/DSOTM/02.flac"]);
    }

    #[test]
    fn searches_by_prefix_and_ignores_diacritics() {
        let (mut conn, songs) = library();
        project(&mut conn, songs).unwrap();

        let hits = search(&conn, "pink fl").unwrap();
        assert_eq!(hits.albums.len(), 1);
        assert_eq!(hits.tracks.len(), 2);

        let hits = search(&conn, "bjork").unwrap();
        assert_eq!(hits.albums[0].title, "Post");
        assert_eq!(hits.tracks[0].album_title, "Post");

        assert!(search(&conn, "\"* OR").unwrap().albums.is_empty());
    }

    #[test]
    fn art_survives_rebuilds() {
        let (mut conn, songs) = library();
        project(&mut conn, songs.clone()).unwrap();
        let todo = art_todo(&conn).unwrap();
        assert_eq!(todo.len(), 2);
        for (id, _) in &todo {
            set_art(&conn, id, Some("abc"), 2_000_000_000).unwrap();
        }
        assert!(art_todo(&conn).unwrap().is_empty());

        let mut changed = songs;
        changed.pop();
        project(&mut conn, changed).unwrap();
        assert!(albums(&conn).unwrap().iter().all(|a| a.art.as_deref() == Some("abc")));
        assert_eq!(load_index(&conn).unwrap().album_of.len(), 2);
    }
}
