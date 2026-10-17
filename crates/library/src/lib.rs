//! The music library: MPD scans and reads tags (so anything listed is
//! something MPD can play); this crate projects its database into SQLite for
//! album grouping, instant search and a pre-rendered artwork cache.

mod art;
mod db;
mod group;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::Context;
use hifi_core::{Album, AlbumDetail, LibraryStatus, SearchResults};
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
}

impl Library {
    /// Opens (or creates) `data_dir/library.db` and starts the background
    /// worker. Nothing is scanned until [`Library::request_sync`].
    pub fn open(data_dir: &Path, mpd_addr: &str) -> anyhow::Result<Self> {
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
            *current = LibraryStatus { scanning: current.scanning, ..status };
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
                Some(bytes) => {
                    let dir = self.inner.art_dir.clone();
                    match tokio::task::spawn_blocking(move || art::make_thumbs(&dir, &bytes)).await? {
                        Ok(key) => Some(key),
                        Err(e) => {
                            debug!("unreadable artwork for {uri}: {e:#}");
                            None
                        }
                    }
                }
                None => None,
            };
            if let Some(key) = &key {
                let mut index = self.inner.index.write().expect("index lock");
                index.art_of.insert(album_id.clone(), key.clone());
                pending += 1;
            }
            let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as u32);
            self.db(move |conn| db::set_art(conn, &album_id, key.as_deref(), now)).await?;
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
