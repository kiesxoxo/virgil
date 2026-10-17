//! The music library: MPD scans and reads tags (so anything listed is
//! something MPD can play); this crate projects its database into SQLite for
//! album grouping, instant search and a pre-rendered artwork cache.

mod art;
mod db;
mod group;
mod online;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, bail};
use hifi_core::{Album, AlbumDetail, ArtOutcome, ArtProgress, LibraryStatus, SearchResults};
use hifi_engine_mpd::{Entry, MpdEngine, Song};
use rusqlite::Connection;
use tokio::sync::{Notify, watch};
use tracing::{debug, info, warn};

pub use art::SIZES as ART_SIZES;

/// Artwork changes are published in batches of this many albums.
const ART_BATCH: usize = 50;

#[derive(Clone)]
pub struct Library {
    inner: Arc<Inner>,
}

struct Inner {
    db: Mutex<Connection>,
    mpd: MpdEngine,
    art_dir: PathBuf,
    index: RwLock<db::Index>,
    status: watch::Sender<LibraryStatus>,
    sync_requested: Notify,
    /// MPD's music directory as this process sees it; downloaded covers go there.
    music_dir: Option<PathBuf>,
    finder: online::Finder,
    downloading: AtomicBool,
}

/// Folder images MPD's `albumart` doesn't read but people use (folder.jpg, front.png, …).
const FOLDER_ART: [&str; 4] = ["cover", "folder", "front", "album"];
const IMAGE_EXTENSIONS: [&str; 4] = ["jpg", "jpeg", "png", "webp"];

fn unix_now() -> u32 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as u32)
}

/// A cover/folder/front/album image already in `dir`, if any.
fn folder_image(dir: &Path) -> anyhow::Result<Option<PathBuf>> {
    let entries = std::fs::read_dir(dir).with_context(|| format!("reading album folder {}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let lower = |s: Option<&std::ffi::OsStr>| s.and_then(|s| s.to_str()).map(str::to_lowercase);
        if let (Some(stem), Some(ext)) = (lower(path.file_stem()), lower(path.extension()))
            && FOLDER_ART.contains(&stem.as_str())
            && IMAGE_EXTENSIONS.contains(&ext.as_str())
        {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

/// Writes a new file; fails with AlreadyExists rather than overwrite one.
fn write_new(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(path)?;
    if let Err(e) = file.write_all(data).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(e);
    }
    Ok(())
}

impl Library {
    /// Opens (or creates) `data_dir/library.db` and starts the background
    /// worker. Nothing is scanned until [`Library::request_sync`]. Without a
    /// `music_dir` covers can't be downloaded.
    pub fn open(data_dir: &Path, mpd_addr: &str, music_dir: Option<PathBuf>) -> anyhow::Result<Self> {
        let art_dir = data_dir.join("art");
        std::fs::create_dir_all(&art_dir).with_context(|| format!("creating {}", art_dir.display()))?;
        let db_path = data_dir.join("library.db");
        let conn = Connection::open(&db_path).with_context(|| format!("opening {}", db_path.display()))?;
        db::init(&conn)?;
        let index = db::load_index(&conn)?;
        let status = db::status(&conn)?;
        let library = Self {
            inner: Arc::new(Inner {
                db: Mutex::new(conn),
                mpd: MpdEngine::bulk(mpd_addr),
                art_dir,
                index: RwLock::new(index),
                status: watch::Sender::new(status),
                sync_requested: Notify::new(),
                music_dir,
                finder: online::Finder::new()?,
                downloading: AtomicBool::new(false),
            }),
        };
        library.spawn_worker();
        Ok(library)
    }

    pub fn art_dir(&self) -> &Path {
        &self.inner.art_dir
    }

    pub fn status(&self) -> LibraryStatus {
        self.inner.status.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<LibraryStatus> {
        self.inner.status.subscribe()
    }

    /// Re-reads MPD's database. Requests made while a sync runs coalesce into one more.
    pub fn request_sync(&self) {
        self.inner.sync_requested.notify_one();
    }

    /// The album id and artwork key for a song URI.
    pub fn lookup(&self, uri: &str) -> Option<(String, Option<String>)> {
        let index = self.inner.index.read().expect("index lock");
        let album = index.album_of.get(uri)?;
        Some((album.clone(), index.art_of.get(album).cloned()))
    }

    pub async fn albums(&self) -> anyhow::Result<Vec<Album>> {
        self.db(|conn| db::albums(conn)).await
    }

    pub async fn album(&self, id: String) -> anyhow::Result<Option<AlbumDetail>> {
        self.db(move |conn| db::album(conn, &id)).await
    }

    pub async fn album_uris(&self, id: String) -> anyhow::Result<Vec<String>> {
        self.db(move |conn| db::album_uris(conn, &id)).await
    }

    pub async fn search(&self, query: String) -> anyhow::Result<SearchResults> {
        self.db(move |conn| db::search(conn, &query)).await
    }

    async fn db<R, F>(&self, f: F) -> anyhow::Result<R>
    where
        R: Send + 'static,
        F: FnOnce(&mut Connection) -> rusqlite::Result<R> + Send + 'static,
    {
        let inner = self.inner.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut conn = inner.db.lock().expect("db lock");
            f(&mut conn)
        })
        .await?;
        Ok(result?)
    }

    fn publish(&self, status: LibraryStatus) {
        self.inner.status.send_modify(|current| {
            *current =
                LibraryStatus { scanning: current.scanning, art_download: current.art_download.clone(), ..status };
        });
    }

    fn set_scanning(&self, scanning: bool) {
        self.inner.status.send_if_modified(|current| {
            let changed = current.scanning != scanning;
            current.scanning = scanning;
            changed
        });
    }

    fn spawn_worker(&self) {
        let library = self.clone();
        tokio::spawn(async move {
            loop {
                library.inner.sync_requested.notified().await;
                library.set_scanning(true);
                if let Err(e) = library.sync().await {
                    warn!("library sync failed: {e:#}");
                } else if let Err(e) = library.fetch_art().await {
                    warn!("artwork pass stopped: {e:#}");
                }
                library.set_scanning(false);
            }
        });
    }

    async fn sync(&self) -> anyhow::Result<()> {
        let started = Instant::now();
        let songs = self.walk().await?;
        let count = songs.len();
        let (changed, status, index) = self
            .db(move |conn| {
                let changed = db::project(conn, songs)?;
                Ok((changed, db::status(conn)?, db::load_index(conn)?))
            })
            .await?;
        if changed {
            *self.inner.index.write().expect("index lock") = index;
            self.publish(status.clone());
            info!("library: {} albums, {count} tracks in {:.1?}", status.albums, started.elapsed());
        } else {
            debug!("library unchanged ({count} tracks, {:.1?})", started.elapsed());
        }
        Ok(())
    }

    /// Walks MPD's database directory by directory: `listallinfo` on a large
    /// library overflows MPD's output buffer.
    async fn walk(&self) -> anyhow::Result<Vec<Song>> {
        let mut songs = Vec::new();
        let mut dirs = vec![String::new()];
        while let Some(dir) = dirs.pop() {
            for entry in self.inner.mpd.lsinfo(&dir).await? {
                match entry {
                    Entry::Directory(path) => dirs.push(path),
                    Entry::Song(song) => songs.push(*song),
                    Entry::Playlist(_) => {}
                }
            }
        }
        Ok(songs)
    }

    async fn fetch_art(&self) -> anyhow::Result<()> {
        let todo = self.db(|conn| db::art_todo(conn)).await?;
        if todo.is_empty() {
            return Ok(());
        }
        info!("artwork: checking {} albums", todo.len());
        let mut pending = 0;
        for (album_id, uri) in todo {
            let picture = match self.inner.mpd.picture(&uri).await {
                Ok(picture) => picture,
                Err(e) if e.is_connection() => return Err(e.into()),
                Err(e) => {
                    debug!("no artwork for {uri}: {e}");
                    None
                }
            };
            let key = match picture {
                Some(bytes) => match self.thumbs(bytes).await {
                    Ok(key) => Some(key),
                    Err(e) => {
                        debug!("unreadable artwork for {uri}: {e:#}");
                        None
                    }
                },
                None => None,
            };
            if key.is_some() {
                pending += 1;
            }
            self.record_art(album_id, key).await?;
            if pending == ART_BATCH {
                self.bump().await?;
                pending = 0;
            }
        }
        if pending > 0 {
            self.bump().await?;
        }
        Ok(())
    }

    async fn thumbs(&self, bytes: Vec<u8>) -> anyhow::Result<String> {
        let dir = self.inner.art_dir.clone();
        tokio::task::spawn_blocking(move || art::make_thumbs(&dir, &bytes)).await?
    }

    /// Stores an album's artwork key; None means "checked, none found".
    async fn record_art(&self, album_id: String, key: Option<String>) -> anyhow::Result<()> {
        if let Some(key) = &key {
            self.inner.index.write().expect("index lock").art_of.insert(album_id.clone(), key.clone());
        }
        let now = unix_now();
        self.db(move |conn| db::set_art(conn, &album_id, key.as_deref(), now)).await
    }

    /// Downloads a cover for one album and saves it in the album's folder,
    /// unless the album already has embedded or folder art.
    pub async fn download_art(&self, album_id: String) -> anyhow::Result<ArtOutcome> {
        let outcome = self.download_one(&album_id).await?;
        self.bump().await?;
        Ok(outcome)
    }

    async fn download_one(&self, album_id: &str) -> anyhow::Result<ArtOutcome> {
        let id = album_id.to_string();
        let Some(target) = self.db(move |conn| db::art_target(conn, &id)).await? else { bail!("no such album") };
        if target.art.is_some() {
            return Ok(ArtOutcome::AlreadyPresent);
        }
        // Art may have appeared since the last scan: embedded, or a cover.* MPD reads.
        if let Ok(Some(bytes)) = self.inner.mpd.picture(&target.cover_uri).await {
            let key = self.thumbs(bytes).await?;
            self.record_art(album_id.to_string(), Some(key)).await?;
            return Ok(ArtOutcome::AlreadyPresent);
        }
        let Some(music_dir) = &self.inner.music_dir else {
            bail!("music_dir is not set in hifid's config, so covers can't be saved");
        };
        let dir = music_dir.join(target.cover_uri.rsplit_once('/').map_or("", |(dir, _)| dir));
        // Folder images MPD ignores (folder.jpg, front.png) still count as art.
        let scan = dir.clone();
        if let Some(existing) = tokio::task::spawn_blocking(move || folder_image(&scan)).await?? {
            let bytes = tokio::fs::read(&existing).await?;
            if let Ok(key) = self.thumbs(bytes).await {
                self.record_art(album_id.to_string(), Some(key)).await?;
            }
            return Ok(ArtOutcome::AlreadyPresent);
        }
        let Some(bytes) = self.inner.finder.find(&target.artist, &target.title).await? else {
            return Ok(ArtOutcome::NotFound);
        };
        let extension = if matches!(image::guess_format(&bytes), Ok(image::ImageFormat::Png)) { "png" } else { "jpg" };
        let path = dir.join(format!("cover.{extension}"));
        let (to, data) = (path.clone(), bytes.clone());
        let written = tokio::task::spawn_blocking(move || write_new(&to, &data)).await?;
        match written {
            Ok(()) => {}
            // Someone put a cover there meanwhile: leave it alone.
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Ok(ArtOutcome::AlreadyPresent),
            Err(e) => return Err(e).with_context(|| format!("saving {}", path.display())),
        }
        info!("artwork: saved {}", path.display());
        let key = self.thumbs(bytes).await?;
        self.record_art(album_id.to_string(), Some(key)).await?;
        Ok(ArtOutcome::Saved)
    }

    /// Starts downloading covers for every album without art. False when a run
    /// is already going.
    pub fn download_missing_art(&self) -> bool {
        if self.inner.downloading.swap(true, Ordering::SeqCst) {
            return false;
        }
        let library = self.clone();
        tokio::spawn(async move {
            library.download_missing().await;
            library.inner.downloading.store(false, Ordering::SeqCst);
        });
        true
    }

    async fn download_missing(&self) {
        let ids = match self.db(|conn| db::missing_art(conn)).await {
            Ok(ids) => ids,
            Err(e) => return warn!("artwork download: {e:#}"),
        };
        let mut progress = ArtProgress { done: 0, total: ids.len() as u32, saved: 0, error: None };
        self.set_progress(Some(progress.clone()));
        info!("artwork: downloading covers for {} albums", progress.total);
        for id in ids {
            match self.download_one(&id).await {
                Ok(outcome) => {
                    if outcome == ArtOutcome::Saved {
                        progress.saved += 1;
                    }
                    if outcome != ArtOutcome::NotFound {
                        let _ = self.bump().await;
                    }
                }
                // The first album failing means no internet or no write access: stop and say why.
                Err(e) if progress.done == 0 => {
                    warn!("artwork download stopped: {e:#}");
                    progress.error = Some(format!("{e:#}"));
                    self.set_progress(Some(progress));
                    return;
                }
                Err(e) => warn!("artwork for album {id}: {e:#}"),
            }
            progress.done += 1;
            self.set_progress(Some(progress.clone()));
        }
        info!("artwork: saved {} of {} covers", progress.saved, progress.total);
        self.set_progress(None);
        let _ = self.bump().await;
    }

    fn set_progress(&self, progress: Option<ArtProgress>) {
        self.inner.status.send_modify(|status| status.art_download = progress);
    }

    async fn bump(&self) -> anyhow::Result<()> {
        let status = self
            .db(|conn| {
                db::bump_revision(conn)?;
                db::status(conn)
            })
            .await?;
        self.publish(status);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_folder_art_and_never_overwrites() {
        let dir = std::env::temp_dir().join(format!("hifi-art-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("01 Track.flac"), b"x").unwrap();
        assert_eq!(folder_image(&dir).unwrap(), None);

        write_new(&dir.join("cover.jpg"), b"first").unwrap();
        let again = write_new(&dir.join("cover.jpg"), b"second").unwrap_err();
        assert_eq!(again.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(dir.join("cover.jpg")).unwrap(), b"first");

        std::fs::remove_file(dir.join("cover.jpg")).unwrap();
        std::fs::write(dir.join("Folder.JPG"), b"x").unwrap();
        assert_eq!(folder_image(&dir).unwrap(), Some(dir.join("Folder.JPG")));
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(folder_image(&dir).is_err(), "a missing folder (share not mounted) is an error");
    }
}
