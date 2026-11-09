//! Downloads: albums through Lidarr, single songs from Soulseek through slskdpp, filed into the
//! library by Lidarr. Virgil never writes the music itself; it asks, follows along, and rescans
//! the folder once the music is in (the library is often a network share, which tells MPD
//! nothing).
//!
//! Each download is a little state machine, moved on every few seconds while any is under way and
//! kept in `downloads.json`, so a restart picks up where it was.

use std::future::Future;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, anyhow, bail};
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;
use tracing::{info, warn};
use virgil_core::{Download, DownloadKind, DownloadRequest, DownloadState, Downloads};
use virgil_fetch::lidarr::{Album, Artist, ImportFile, Lidarr, Placement};
use virgil_fetch::pick::{Candidate, Want, candidates};
use virgil_fetch::slskdpp::Slskdpp;
use virgil_fetch::{paths, pick};

use crate::config::{LidarrConfig, SlskdppConfig};
use crate::party::now;

/// What the runner needs from the rest of Virgil; tests stand in for it.
pub trait Host: Sync {
    /// Has MPD rescan a folder of the music directory.
    fn rescan(&self, folder: &str) -> impl Future<Output = anyhow::Result<()>> + Send;
    /// Virgil's album holding a file, once the library has it.
    fn album_of(&self, uri: &str) -> Option<String>;
}

/// How long to wait for things, in seconds.
#[derive(Debug, Clone, Copy)]
pub struct Patience {
    /// For Lidarr to put a search's grab in its queue, or a new grab after a failed one.
    pub grab: u32,
    /// For Lidarr to read a new album's tracklist.
    pub tracks: u32,
    /// For Soulseek answers before taking the best so far.
    pub answers: u32,
    /// For Soulseek answers at all.
    pub search: u32,
    /// In someone's upload queue before trying someone else.
    pub queue: u32,
    /// For the library to show what was rescanned.
    pub library: u32,
    /// For a service to answer again before giving up.
    pub trouble: u32,
}

impl Default for Patience {
    fn default() -> Self {
        Self { grab: 90, tracks: 180, answers: 15, search: 60, queue: 20 * 60, library: 180, trouble: 180 }
    }
}

/// Songs tried from at most this many people.
const MAX_SOURCES: usize = 3;
/// Finished downloads stay listed this long.
const KEEP: u32 = 7 * 24 * 3600;
/// The artist list is read again after this long.
const ARTISTS_TTL: Duration = Duration::from_secs(60);

pub struct Fetcher {
    pub lidarr: Lidarr,
    pub slskdpp: Option<Slskdpp>,
    root: Option<String>,
    /// slskdpp's downloads directory, as Lidarr sees it.
    downloads_for_lidarr: String,
    formats: Vec<String>,
    patience: Patience,
    /// Lidarr's root folder and the defaults new artists get.
    placement: tokio::sync::Mutex<Option<Placement>>,
    artists: tokio::sync::Mutex<Option<(Instant, Vec<Artist>)>>,
    jobs: Mutex<Vec<Job>>,
    path: Option<PathBuf>,
    /// Held while downloads are moved on, so two rounds never work on one.
    round: tokio::sync::Mutex<()>,
    pub wake: Notify,
}

/// A download, with where it is in its steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Job {
    public: Download,
    step: Step,
    /// Lidarr's ids of the album and its artist, once it has them.
    album: Option<u32>,
    artist: Option<u32>,
    /// The song's length in milliseconds, for telling it apart on Soulseek.
    #[serde(default)]
    duration_ms: u64,
    /// When a service started failing to answer, while it does.
    #[serde(default)]
    trouble: Option<u32>,
    /// Track files Lidarr had of the album before searching: any more, and something came in.
    #[serde(default)]
    files_before: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "camelCase")]
enum Step {
    /// Have Lidarr add the album (and its artist), or watch for it if it has it.
    Add,
    /// Wait for Lidarr to read the tracklist.
    Tracks {
        since: u32,
    },
    /// Start Lidarr's search.
    Search,
    /// Lidarr is searching; wait for the grab to show up in its queue.
    Grab {
        command: u32,
        since: u32,
        ended: Option<u32>,
    },
    /// Follow Lidarr's download until it is imported.
    Queue {
        gone: Option<u32>,
    },
    /// Start a Soulseek search for the song.
    Soulseek,
    /// Soulseek answers are coming in.
    Answers {
        search: u32,
        since: u32,
    },
    /// Ask the next person on the list for the file.
    Fetch {
        candidates: Vec<Candidate>,
        next: usize,
    },
    /// Follow slskdpp's download.
    Transfer {
        download: i64,
        candidates: Vec<Candidate>,
        next: usize,
        since: u32,
    },
    /// Have Lidarr file the song into its album.
    Import {
        file: String,
    },
    Importing {
        command: u32,
    },
    /// The music is in: rescan its folder.
    Rescan,
    /// Wait for the library to show it.
    Link {
        uri: String,
        since: u32,
    },
    End,
}

/// Whether a step moved on and the next can go right away.
enum Flow {
    Next,
    Wait,
}

impl Fetcher {
    pub fn new(
        lidarr: &LidarrConfig,
        slskdpp: Option<&SlskdppConfig>,
        path: Option<PathBuf>,
        patience: Patience,
    ) -> anyhow::Result<Self> {
        let jobs = path.as_deref().map(load).unwrap_or_default();
        Ok(Self {
            lidarr: Lidarr::new(&lidarr.url, &lidarr.api_key)?,
            slskdpp: slskdpp.map(|s| Slskdpp::new(&s.url, &s.api_key)).transpose()?,
            root: lidarr.root.clone(),
            downloads_for_lidarr: slskdpp.map(|s| s.downloads.clone()).unwrap_or_default(),
            formats: slskdpp.map(|s| s.formats.clone()).unwrap_or_default(),
            patience,
            placement: tokio::sync::Mutex::default(),
            artists: tokio::sync::Mutex::default(),
            jobs: Mutex::new(jobs),
            path,
            round: tokio::sync::Mutex::default(),
            wake: Notify::new(),
        })
    }

    pub fn downloads(&self) -> Downloads {
        let jobs = self.jobs.lock().expect("jobs lock");
        Downloads { songs: self.slskdpp.is_some(), jobs: jobs.iter().rev().map(|j| j.public.clone()).collect() }
    }

    fn active(&self) -> bool {
        self.jobs.lock().expect("jobs lock").iter().any(|j| !j.public.state.finished())
    }

    /// Lidarr's root folder (which is MPD's music directory) and its defaults for new artists.
    pub async fn placement(&self) -> anyhow::Result<Placement> {
        let mut cached = self.placement.lock().await;
        if let Some(placement) = cached.as_ref() {
            return Ok(placement.clone());
        }
        let folders = self.lidarr.root_folders().await?;
        let folder = match &self.root {
            Some(root) => folders.iter().find(|f| f.path.trim_end_matches('/') == root.trim_end_matches('/')),
            None => folders.first(),
        };
        let folder = folder.ok_or_else(|| match &self.root {
            Some(root) => anyhow!("Lidarr has no root folder {root}"),
            None => anyhow!("Lidarr has no root folder"),
        })?;
        let placement = Placement {
            root: folder.path.trim_end_matches('/').to_string(),
            quality_profile: folder.default_quality_profile_id,
            metadata_profile: folder.default_metadata_profile_id,
        };
        *cached = Some(placement.clone());
        Ok(placement)
    }

    /// The artists Lidarr has, read at most once a minute.
    pub async fn artists(&self) -> anyhow::Result<Vec<Artist>> {
        let mut cached = self.artists.lock().await;
        if let Some((_, artists)) = cached.as_ref().filter(|(at, _)| at.elapsed() < ARTISTS_TTL) {
            return Ok(artists.clone());
        }
        let artists = self.lidarr.artists().await?;
        *cached = Some((Instant::now(), artists.clone()));
        Ok(artists)
    }

    /// Forgets the artist list, after Virgil added one.
    pub async fn artists_changed(&self) {
        *self.artists.lock().await = None;
    }

    /// Starts getting an album or a song, or returns the download already doing it.
    pub async fn start(&self, request: &DownloadRequest, by: Option<String>) -> anyhow::Result<Download> {
        let kind = if request.track.is_some() { DownloadKind::Song } else { DownloadKind::Album };
        if kind == DownloadKind::Song && self.slskdpp.is_none() {
            bail!("Virgil isn't set up to download single songs (it needs slskdpp)");
        }
        if let Some(job) =
            self.jobs.lock().expect("jobs lock").iter().find(|j| {
                j.public.album == request.album && j.public.track == request.track && !j.public.state.finished()
            })
        {
            return Ok(job.public.clone());
        }

        let found = self.lidarr.lookup_album(&request.album).await?;
        let found = found.ok_or_else(|| anyhow!("Lidarr doesn't know that album"))?;
        let album: Album = serde_json::from_value(found).context("reading Lidarr's album")?;
        let lidarr_id = (album.id > 0).then_some(album.id);
        let artist_id = album.artist.as_ref().map(|a| a.id).filter(|&id| id > 0);
        let mut job = Job {
            public: Download {
                id: 0,
                kind,
                album: request.album.clone(),
                track: request.track,
                title: album.title.clone(),
                artist: album.artist_name().to_string(),
                album_title: album.title.clone(),
                cover: album.cover(),
                state: DownloadState::Adding,
                progress: None,
                note: None,
                by,
                album_id: None,
                started: now(),
                updated: now(),
            },
            step: Step::Add,
            album: lidarr_id,
            artist: artist_id,
            duration_ms: 0,
            trouble: None,
            files_before: None,
        };
        if let Some(track) = request.track {
            let album = lidarr_id.ok_or_else(|| anyhow!("Open the album first, so Lidarr reads its tracklist"))?;
            let tracks = self.lidarr.tracks(album).await?;
            let track = tracks.into_iter().find(|t| t.id == track).ok_or_else(|| anyhow!("Lidarr has no such song"))?;
            if track.has_file {
                bail!("That song is in the library already");
            }
            job.public.title = track.title;
            job.duration_ms = track.duration;
            job.public.state = DownloadState::Searching;
            job.step = Step::Soulseek;
        }

        let public = {
            let mut jobs = self.jobs.lock().expect("jobs lock");
            prune(&mut jobs);
            // Tried before and done with: this one takes its place.
            jobs.retain(|j| !(j.public.album == request.album && j.public.track == request.track));
            job.public.id = jobs.iter().map(|j| j.public.id).max().unwrap_or(0) + 1;
            info!("download {}: {} – {}", job.public.id, job.public.artist, job.public.title);
            jobs.push(job.clone());
            self.save(&jobs);
            job.public
        };
        self.wake.notify_one();
        Ok(public)
    }

    /// Starts a finished download over.
    pub fn retry(&self, id: u32) -> Option<Download> {
        let mut jobs = self.jobs.lock().expect("jobs lock");
        let job = jobs.iter_mut().find(|j| j.public.id == id)?;
        if job.public.state.finished() {
            job.step = match job.public.kind {
                DownloadKind::Album => Step::Add,
                DownloadKind::Song => Step::Soulseek,
            };
            job.public.state =
                if job.public.kind == DownloadKind::Album { DownloadState::Adding } else { DownloadState::Searching };
            job.public.progress = None;
            job.public.note = None;
            job.trouble = None;
            job.public.updated = now();
        }
        let public = job.public.clone();
        self.save(&jobs);
        drop(jobs);
        self.wake.notify_one();
        Some(public)
    }

    /// Takes a download off the list, and stops slskdpp's part of it. Lidarr carries on with an
    /// album it is getting: it is on its wanted list.
    pub async fn dismiss(&self, id: u32) -> bool {
        let job = {
            let mut jobs = self.jobs.lock().expect("jobs lock");
            let Some(at) = jobs.iter().position(|j| j.public.id == id) else { return false };
            let job = jobs.remove(at);
            self.save(&jobs);
            job
        };
        if let (Step::Transfer { download, .. }, Some(slskdpp)) = (&job.step, &self.slskdpp) {
            if let Err(e) = slskdpp.cancel(*download).await {
                warn!("download {id}: cancelling on slskdpp: {e:#}");
            }
        }
        true
    }

    /// Moves every download under way on as far as it goes now. Returns whether any is still
    /// under way.
    pub async fn round(&self, host: &impl Host) -> bool {
        let _round = self.round.lock().await;
        let ids: Vec<u32> = {
            let jobs = self.jobs.lock().expect("jobs lock");
            jobs.iter().filter(|j| !j.public.state.finished()).map(|j| j.public.id).collect()
        };
        for id in ids {
            let Some(mut job) = self.jobs.lock().expect("jobs lock").iter().find(|j| j.public.id == id).cloned() else {
                continue;
            };
            let before = job.public.clone();
            self.advance(&mut job, host).await;
            if job.public != before {
                job.public.updated = now();
            }
            let mut jobs = self.jobs.lock().expect("jobs lock");
            // Dismissed meanwhile: let it go.
            if let Some(slot) = jobs.iter_mut().find(|j| j.public.id == id) {
                *slot = job;
                self.save(&jobs);
            }
        }
        self.active()
    }

    async fn advance(&self, job: &mut Job, host: &impl Host) {
        // Enough steps for any download to go from start to finish in one round.
        for _ in 0..16 {
            match self.step(job, host).await {
                Ok(Flow::Next) => job.trouble = None,
                Ok(Flow::Wait) => {
                    job.trouble = None;
                    return;
                }
                Err(e) => {
                    let since = *job.trouble.get_or_insert(now());
                    let message = format!("{e:#}");
                    if now().saturating_sub(since) >= self.patience.trouble {
                        warn!("download {}: giving up: {message}", job.public.id);
                        fail(job, message);
                    } else {
                        job.public.note = Some(message);
                    }
                    return;
                }
            }
        }
    }

    async fn step(&self, job: &mut Job, host: &impl Host) -> anyhow::Result<Flow> {
        let p = self.patience;
        let t = now();
        let public = &mut job.public;
        match job.step.clone() {
            Step::Add => {
                public.state = DownloadState::Adding;
                match job.album {
                    Some(album) => self.lidarr.set_monitored(&[album], true).await?,
                    None => {
                        let lookup = self.lidarr.lookup_album(&public.album).await?;
                        let lookup = lookup.ok_or_else(|| anyhow!("Lidarr doesn't know that album"))?;
                        let id = lookup["id"].as_u64().unwrap_or(0) as u32;
                        if id > 0 {
                            job.album = Some(id);
                            self.lidarr.set_monitored(&[id], true).await?;
                        } else {
                            let placement = self.placement().await?;
                            let added = self
                                .lidarr
                                .add_album(&virgil_fetch::lidarr::new_album(&lookup, &placement, true))
                                .await?;
                            info!("download {}: Lidarr added {} – {}", public.id, public.artist, public.title);
                            job.album = Some(added.id);
                            self.artists_changed().await;
                        }
                    }
                }
                job.step = Step::Tracks { since: t };
                Ok(Flow::Next)
            }
            Step::Tracks { since } => {
                let album = job.album.context("no album")?;
                if self.lidarr.tracks(album).await?.is_empty() {
                    if t.saturating_sub(since) >= p.tracks {
                        fail(job, "Lidarr couldn't read this album's tracklist".into());
                        return Ok(Flow::Wait);
                    }
                    return Ok(Flow::Wait);
                }
                job.step = Step::Search;
                Ok(Flow::Next)
            }
            Step::Search => {
                let album = job.album.context("no album")?;
                job.files_before = Some(self.files(album).await?.0);
                let command = self.lidarr.search(&[album]).await?;
                public.state = DownloadState::Searching;
                public.note = None;
                job.step = Step::Grab { command: command.id, since: t, ended: None };
                Ok(Flow::Next)
            }
            Step::Grab { command, since, ended } => {
                let album = job.album.context("no album")?;
                if !self.lidarr.queue(&[album]).await?.is_empty() {
                    job.step = Step::Queue { gone: None };
                    return Ok(Flow::Next);
                }
                let ended = match ended {
                    Some(ended) => ended,
                    None => {
                        let status = self.lidarr.command(command).await?;
                        if !status.finished() {
                            return Ok(Flow::Wait);
                        }
                        if !status.succeeded() {
                            let why = status.message.unwrap_or_else(|| "Lidarr's search failed".into());
                            fail(job, why);
                            return Ok(Flow::Wait);
                        }
                        job.step = Step::Grab { command, since, ended: Some(t) };
                        t
                    }
                };
                if self.arrived(album, job.files_before).await? {
                    job.step = Step::Rescan;
                    return Ok(Flow::Next);
                }
                if t.saturating_sub(ended) >= p.grab {
                    job.public.state = DownloadState::NotFound;
                    job.public.note = Some("Lidarr found no release; it keeps looking".into());
                    job.step = Step::End;
                }
                Ok(Flow::Wait)
            }
            Step::Queue { gone } => {
                let album = job.album.context("no album")?;
                let queue = self.lidarr.queue(&[album]).await?;
                let Some(item) = queue.iter().find(|q| q.album_id == Some(album)).or(queue.first()) else {
                    if self.arrived(album, job.files_before).await? {
                        job.step = Step::Rescan;
                        return Ok(Flow::Next);
                    }
                    let gone = gone.unwrap_or(t);
                    // A failed download makes way for another release, which takes a moment to grab.
                    if t.saturating_sub(gone) >= p.grab {
                        public.state = DownloadState::NotFound;
                        public.note = Some("The download fell through; Lidarr keeps looking".into());
                        job.step = Step::End;
                    } else {
                        public.state = DownloadState::Searching;
                        job.step = Step::Queue { gone: Some(gone) };
                    }
                    return Ok(Flow::Wait);
                };
                job.step = Step::Queue { gone: None };
                public.progress = item.progress();
                public.note = None;
                match item.tracked_download_state.as_deref().unwrap_or("downloading") {
                    "imported" => {
                        job.step = Step::Rescan;
                        return Ok(Flow::Next);
                    }
                    "importPending" | "importing" => public.state = DownloadState::Importing,
                    "importBlocked" | "importFailed" => {
                        public.state = DownloadState::Importing;
                        let problem = item.problem().unwrap_or_else(|| "it can't be imported".into());
                        public.note = Some(format!("Lidarr needs a hand: {problem}"));
                    }
                    "downloadFailed" | "downloadFailedPending" => {
                        public.state = DownloadState::Searching;
                        public.note = Some("Trying another release".into());
                    }
                    _ if matches!(item.status.as_str(), "queued" | "paused" | "delay") => {
                        public.state = DownloadState::Waiting;
                    }
                    _ => public.state = DownloadState::Downloading,
                }
                Ok(Flow::Wait)
            }
            Step::Soulseek => {
                let slskdpp = self.slskdpp.as_ref().context("slskdpp isn't set up")?;
                public.state = DownloadState::Searching;
                public.progress = None;
                let query = pick::words(&format!("{} {}", public.artist, public.title)).join(" ");
                let search = slskdpp.search(&query).await?;
                public.note = None;
                job.step = Step::Answers { search: search.id, since: t };
                Ok(Flow::Wait)
            }
            Step::Answers { search, since } => {
                let slskdpp = self.slskdpp.as_ref().context("slskdpp isn't set up")?;
                let waited = t.saturating_sub(since);
                if waited < p.answers {
                    return Ok(Flow::Wait);
                }
                let status = slskdpp.search_status(search).await?;
                let hits = slskdpp.hits(search, &self.formats).await?;
                let want = Want {
                    artist: &public.artist,
                    album: &public.album_title,
                    title: &public.title,
                    duration_ms: job.duration_ms,
                    formats: &self.formats,
                };
                let found = candidates(&hits, &want);
                if found.is_empty() {
                    if status.active && waited < p.search {
                        return Ok(Flow::Wait);
                    }
                    let _ = slskdpp.forget_search(search).await;
                    public.state = DownloadState::NotFound;
                    public.note = Some(format!("Nobody on Soulseek has it as {} right now", self.formats.join(" or ")));
                    job.step = Step::End;
                    return Ok(Flow::Wait);
                }
                let _ = slskdpp.forget_search(search).await;
                job.step = Step::Fetch { candidates: found, next: 0 };
                Ok(Flow::Next)
            }
            Step::Fetch { candidates, next } => {
                let slskdpp = self.slskdpp.as_ref().context("slskdpp isn't set up")?;
                let Some(candidate) = candidates.get(next).filter(|_| next < MAX_SOURCES) else {
                    fail(job, "Nobody on Soulseek would send it".into());
                    return Ok(Flow::Wait);
                };
                let download = slskdpp.download(&candidate.user, &candidate.path, candidate.size).await?;
                public.state = DownloadState::Waiting;
                public.progress = None;
                job.step = Step::Transfer { download: download.id, candidates, next, since: t };
                Ok(Flow::Next)
            }
            Step::Transfer { download, candidates, next, since } => {
                let slskdpp = self.slskdpp.as_ref().context("slskdpp isn't set up")?;
                let downloads = slskdpp.downloads().await?;
                let Some(d) = downloads.into_iter().find(|d| d.id == download) else {
                    job.step = Step::Fetch { candidates, next: next + 1 };
                    return Ok(Flow::Next);
                };
                match d.state.as_str() {
                    "completed" => {
                        public.progress = Some(1.0);
                        let file = paths::downloaded(&self.downloads_for_lidarr, &d.folder, &d.name);
                        job.step = Step::Import { file };
                        Ok(Flow::Next)
                    }
                    "failed" | "cancelled" => {
                        public.note = d.error;
                        job.step = Step::Fetch { candidates, next: next + 1 };
                        Ok(Flow::Next)
                    }
                    "transferring" => {
                        public.state = DownloadState::Downloading;
                        public.progress = d.progress();
                        public.note = None;
                        Ok(Flow::Wait)
                    }
                    _ if t.saturating_sub(since) >= p.queue => {
                        // Waited long enough in this queue: someone else may be quicker.
                        let _ = slskdpp.cancel(download).await;
                        job.step = Step::Fetch { candidates, next: next + 1 };
                        Ok(Flow::Next)
                    }
                    _ => {
                        public.state = DownloadState::Waiting;
                        public.note =
                            d.queue_place.filter(|&n| n > 0).map(|n| format!("{} in their queue", ordinal(n)));
                        Ok(Flow::Wait)
                    }
                }
            }
            Step::Import { file } => {
                let album = job.album.context("no album")?;
                let track = public.track.context("no song")?;
                let found = self.lidarr.import_candidates(paths::parent(&file)).await?;
                let Some(item) = found.into_iter().find(|i| i.path == file) else {
                    bail!("Lidarr can't see the download at {file}; check [slskdpp] downloads in Virgil's config");
                };
                let release = match item.album_release_id {
                    0 => self.lidarr.album(album).await?.monitored_release().context("Lidarr has no release")?,
                    release => release,
                };
                let artist = match job.artist {
                    Some(artist) => artist,
                    None => self.lidarr.album(album).await?.artist_id,
                };
                let file = ImportFile { path: file, artist, album, release, track, quality: item.quality };
                let command = self.lidarr.import(&file).await?;
                public.state = DownloadState::Importing;
                public.note = None;
                job.step = Step::Importing { command: command.id };
                Ok(Flow::Wait)
            }
            Step::Importing { command } => {
                let status = self.lidarr.command(command).await?;
                if !status.finished() {
                    return Ok(Flow::Wait);
                }
                if !status.succeeded() {
                    fail(job, status.message.unwrap_or_else(|| "Lidarr couldn't import it".into()));
                    return Ok(Flow::Wait);
                }
                job.step = Step::Rescan;
                Ok(Flow::Next)
            }
            Step::Rescan => {
                public.state = DownloadState::Importing;
                public.progress = None;
                let album = job.album.context("no album")?;
                let root = self.placement().await?.root;
                let files = self.lidarr.track_files(&[album]).await?;
                let file = match public.track {
                    Some(track) => {
                        let tracks = self.lidarr.tracks(album).await?;
                        let track = tracks.iter().find(|t| t.id == track);
                        if !track.is_some_and(|t| t.has_file) {
                            fail(job, "Lidarr didn't take the file".into());
                            return Ok(Flow::Wait);
                        }
                        let path = files.iter().find(|f| Some(f.id) == track.map(|t| t.track_file_id));
                        path.or(files.first())
                    }
                    None => files.first(),
                };
                let Some(uri) = file.and_then(|f| paths::in_library(&root, &f.path)) else {
                    public.state = DownloadState::Done;
                    public.note = Some("It's in, outside Virgil's music folder".into());
                    job.step = Step::End;
                    return Ok(Flow::Wait);
                };
                host.rescan(paths::artist_folder(&uri)).await?;
                job.step = Step::Link { uri, since: t };
                Ok(Flow::Wait)
            }
            Step::Link { uri, since } => {
                if let Some(album_id) = host.album_of(&uri) {
                    public.album_id = Some(album_id);
                } else if t.saturating_sub(since) < p.library {
                    return Ok(Flow::Wait);
                }
                public.state = DownloadState::Done;
                public.progress = None;
                public.note = None;
                info!("download {}: {} – {} is in", public.id, public.artist, public.title);
                job.step = Step::End;
                Ok(Flow::Wait)
            }
            Step::End => Ok(Flow::Wait),
        }
    }

    /// Lidarr has every track of the album.
    /// Track files Lidarr has of an album, and its tracks.
    async fn files(&self, album: u32) -> anyhow::Result<(u32, u32)> {
        let albums = self.lidarr.albums(&[album]).await?;
        Ok(albums.first().and_then(|a| a.statistics).map_or((0, 0), |s| (s.track_file_count, s.track_count)))
    }

    /// Music came in: Lidarr has more of the album than before the search, or all of it. A release
    /// can come in short of a track or two, which is still worth rescanning for.
    async fn arrived(&self, album: u32, before: Option<u32>) -> anyhow::Result<bool> {
        let (have, total) = self.files(album).await?;
        Ok(total > 0 && (have >= total || before.is_some_and(|b| have > b)))
    }

    fn save(&self, jobs: &[Job]) {
        let Some(path) = &self.path else { return };
        let write = || -> std::io::Result<()> {
            let tmp = path.with_extension("json.tmp");
            let mut file =
                std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
            file.write_all(&serde_json::to_vec(jobs).expect("downloads serialize"))?;
            file.sync_all()?;
            std::fs::rename(&tmp, path)
        };
        if let Err(e) = write() {
            warn!("saving {}: {e}", path.display());
        }
    }
}

fn fail(job: &mut Job, why: String) {
    job.public.state = DownloadState::Failed;
    job.public.progress = None;
    job.public.note = Some(why);
    job.step = Step::End;
}

fn load(path: &std::path::Path) -> Vec<Job> {
    let mut jobs: Vec<Job> = match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            warn!("{}: {e}; starting the download list afresh", path.display());
            Vec::new()
        }),
        Err(_) => Vec::new(),
    };
    prune(&mut jobs);
    jobs
}

/// Forgets downloads finished more than a week ago.
fn prune(jobs: &mut Vec<Job>) {
    let t = now();
    jobs.retain(|j| !j.public.state.finished() || t.saturating_sub(j.public.updated) < KEEP);
}

/// "1st", "2nd", "23rd"…
fn ordinal(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (1, x) if x != 11 => "st",
        (2, x) if x != 12 => "nd",
        (3, x) if x != 13 => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// Moves downloads on: every few seconds while any is under way, else when one starts.
pub async fn run(app: crate::app::App) {
    let Some(fetcher) = app.fetch.as_ref() else { return };
    app.set_downloads(fetcher.downloads());
    loop {
        let busy = fetcher.round(&app).await;
        app.set_downloads(fetcher.downloads());
        if busy {
            tokio::select! {
                () = tokio::time::sleep(Duration::from_secs(3)) => {}
                () = fetcher.wake.notified() => {}
            }
        } else {
            fetcher.wake.notified().await;
        }
    }
}

impl Host for crate::app::App {
    async fn rescan(&self, folder: &str) -> anyhow::Result<()> {
        self.mpd.update_path(folder).await.map_err(|e| anyhow!("rescanning {folder}: {e}"))
    }

    fn album_of(&self, uri: &str) -> Option<String> {
        self.library.lookup(uri).map(|(album, _)| album)
    }
}

#[cfg(test)]
mod tests;
