use std::ops::Deref;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use tokio::sync::{Mutex, broadcast};
use tracing::{info, warn};
use virgil_core::{
    LOCAL_SOURCE, Outputs, PlayState, PlayerState, Queue, QueueItem, ServerEvent, SignalPath, SourceState,
};
use virgil_engine_mpd::MpdEngine;
use virgil_library::Library;

use crate::api::ApiError;
use crate::config::Config;
use crate::desktop::{self, Desktop};
use crate::signal::{Hw, card_label, parse_cards, read_hw};

/// Latest state of everything the UI shows; `hello` sends it whole.
#[derive(Default)]
struct Snapshot {
    player: PlayerState,
    queue: Queue,
    signal: SignalPath,
    source: SourceState,
    outputs: Outputs,
}

/// Serializes hand-overs of the DAC between local playback and external sources.
#[derive(Default)]
struct Arbiter {
    /// Where local playback was when an external source took over.
    resume: Option<(u32, f64)>,
}

#[derive(Clone)]
pub struct App(Arc<Inner>);

pub struct Inner {
    pub config: Config,
    pub mpd: MpdEngine,
    pub library: Library,
    pub events: broadcast::Sender<ServerEvent>,
    /// Set in desktop mode (Virgil.exe), which runs MPD itself.
    pub desktop: Option<Arc<Desktop>>,
    snapshot: RwLock<Snapshot>,
    arbiter: Mutex<Arbiter>,
}

impl Deref for App {
    type Target = Inner;
    fn deref(&self) -> &Inner {
        &self.0
    }
}

impl App {
    pub fn new(config: Config, library: Library, desktop: Option<Arc<Desktop>>) -> Self {
        let source = SourceState { active: LOCAL_SOURCE.into(), available: config.sources.keys().cloned().collect() };
        App(Arc::new(Inner {
            mpd: MpdEngine::new(&config.mpd),
            library,
            events: broadcast::channel(256).0,
            desktop,
            snapshot: RwLock::new(Snapshot { source, ..Snapshot::default() }),
            arbiter: Mutex::default(),
            config,
        }))
    }

    pub fn hello(&self) -> ServerEvent {
        let s = self.snapshot.read().expect("snapshot lock");
        ServerEvent::Hello {
            player: s.player.clone(),
            queue: s.queue.clone(),
            signal: s.signal.clone(),
            source: s.source.clone(),
            library: self.library.status(),
            outputs: s.outputs.clone(),
        }
    }

    /// Re-reads MPD's outputs, names ALSA cards like `aplay -l` does, and
    /// notes cards that have no output yet (plugged in after MPD started).
    pub async fn refresh_outputs(&self) {
        let mut outputs = match self.mpd.outputs().await {
            Ok(outputs) => outputs,
            Err(e) => return warn!("mpd outputs: {e}"),
        };
        let dir = &self.config.asound_dir;
        let cards = tokio::fs::read_to_string(dir.join("cards")).await.map(|t| parse_cards(&t)).unwrap_or_default();
        for output in outputs.iter_mut().filter(|o| o.plugin == "alsa") {
            if dir.join(&output.name).is_dir() {
                output.label = card_label(dir, &output.name, &cards).await;
            }
        }
        // Devices connected since MPD started: on a PC from Windows' list, on
        // Linux where outputs are named after ALSA cards (the container image).
        let new_cards = if self.desktop.is_some() {
            let devices = tokio::task::spawn_blocking(desktop::devices).await.unwrap_or_default();
            devices.into_iter().map(|d| d.name).filter(|name| !outputs.iter().any(|o| o.name == *name)).collect()
        } else if outputs.iter().any(|o| o.plugin == "alsa" && cards.iter().any(|(id, _)| *id == o.name)) {
            cards
                .into_iter()
                .filter(|(id, _)| !outputs.iter().any(|o| o.name == *id))
                .map(|(id, description)| if description.is_empty() { id } else { description })
                .collect()
        } else {
            Vec::new()
        };
        self.update(|s| &mut s.outputs, Outputs { outputs, new_cards }, ServerEvent::Outputs);
    }

    pub async fn select_output(&self, id: u32) -> Result<(), ApiError> {
        let was_playing = self.mpd.status().await?.state == PlayState::Play;
        self.mpd.select_output(id).await?;
        // Switching can still leave MPD paused (e.g. the new device was busy a moment); carry on.
        if was_playing && self.mpd.status().await?.state == PlayState::Pause {
            self.mpd.pause(false).await?;
        }
        self.refresh_outputs().await;
        Ok(())
    }

    pub fn enabled_output_label(&self) -> Option<String> {
        let s = self.snapshot.read().expect("snapshot lock");
        s.outputs.outputs.iter().find(|o| o.enabled).map(|o| o.label.clone())
    }

    /// The ALSA card being played to: the enabled output's, if it names a card, else `alsa_card`.
    pub fn active_card(&self) -> String {
        let s = self.snapshot.read().expect("snapshot lock");
        s.outputs
            .outputs
            .iter()
            .find(|o| o.enabled && o.plugin == "alsa" && self.config.asound_dir.join(&o.name).is_dir())
            .map_or_else(|| self.config.alsa_card.clone(), |o| o.name.clone())
    }

    pub fn player(&self) -> PlayerState {
        self.snapshot.read().expect("snapshot lock").player.clone()
    }

    pub fn queue(&self) -> Queue {
        self.snapshot.read().expect("snapshot lock").queue.clone()
    }

    pub fn source(&self) -> SourceState {
        self.snapshot.read().expect("snapshot lock").source.clone()
    }

    /// Stores `value` and broadcasts it, unless it is unchanged.
    fn update<T: PartialEq + Clone>(
        &self,
        slot: impl FnOnce(&mut Snapshot) -> &mut T,
        value: T,
        event: fn(T) -> ServerEvent,
    ) {
        {
            let mut snapshot = self.snapshot.write().expect("snapshot lock");
            let slot = slot(&mut snapshot);
            if *slot == value {
                return;
            }
            *slot = value.clone();
        }
        // No subscribers is fine.
        let _ = self.events.send(event(value));
    }

    pub fn set_player(&self, player: PlayerState) {
        self.update(|s| &mut s.player, player, ServerEvent::Player);
    }

    pub fn set_queue(&self, queue: Queue) {
        self.update(|s| &mut s.queue, queue, ServerEvent::Queue);
    }

    pub fn set_signal(&self, signal: SignalPath) {
        self.update(|s| &mut s.signal, signal, ServerEvent::Signal);
    }

    fn set_active_source(&self, name: &str) {
        let mut source = self.source();
        source.active = name.to_string();
        self.update(|s| &mut s.source, source, ServerEvent::Source);
    }

    fn enrich(&self, mut items: Vec<QueueItem>) -> Vec<QueueItem> {
        for item in &mut items {
            if let Some((album_id, art)) = self.library.lookup(&item.uri) {
                item.album_id = Some(album_id);
                item.art = art;
            }
        }
        items
    }

    /// Re-reads MPD's status, and the queue if it may have changed.
    pub async fn refresh(&self, queue: bool) {
        let player = match self.mpd.status().await {
            Ok(player) => player,
            Err(e) => return warn!("mpd status: {e}"),
        };
        let version = player.queue_version;
        self.set_player(player);
        if queue {
            match self.mpd.queue().await {
                Ok(items) => self.set_queue(Queue { version, items: self.enrich(items) }),
                Err(e) => warn!("mpd queue: {e}"),
            }
        }
    }

    /// Picks up album ids and artwork the library learned since the queue was read.
    pub fn reenrich_queue(&self) {
        let queue = self.queue();
        self.set_queue(Queue { version: queue.version, items: self.enrich(queue.items) });
    }

    /// Starts or resumes local playback, taking the DAC back if needed.
    pub async fn play(&self) -> Result<(), ApiError> {
        let resume = self.claim_local().await;
        let status = self.mpd.status().await?;
        match status.state {
            PlayState::Pause => self.mpd.pause(false).await?,
            PlayState::Play => {}
            PlayState::Stop => {
                // Pick up where an external source interrupted us, if that song is still queued.
                if let Some((id, at)) = resume {
                    if self.mpd.seek_id(id, at).await.is_ok() {
                        return Ok(());
                    }
                }
                self.mpd.play().await?;
            }
        }
        Ok(())
    }

    /// An external source is about to open the DAC (called from its hook, e.g.
    /// shairport-sync's `run_this_before_play_begins`). Stops local playback
    /// and returns once ALSA reports the device closed.
    pub async fn acquire(&self, name: &str) -> Result<(), ApiError> {
        if !self.config.sources.contains_key(name) {
            return Err(ApiError::not_found(format!("unknown source {name:?}")));
        }
        let mut arbiter = self.arbiter.lock().await;
        if self.source().active == LOCAL_SOURCE {
            match self.mpd.status().await {
                Ok(status) if status.state != PlayState::Stop => {
                    arbiter.resume = status.song_id.map(|id| (id, status.elapsed));
                    self.mpd.stop().await?;
                }
                Ok(_) => {}
                // MPD down means it is not holding the device either.
                Err(e) => warn!("mpd unreachable while yielding the DAC: {e}"),
            }
        }
        info!("DAC handed to {name}");
        self.set_active_source(name);
        drop(arbiter);
        self.wait_until_dac_free(Duration::from_secs(2)).await;
        Ok(())
    }

    /// The external source finished (its `run_this_after_play_ends` hook).
    pub async fn release(&self, name: &str) {
        let _arbiter = self.arbiter.lock().await;
        if self.source().active == name {
            info!("DAC released by {name}");
            self.set_active_source(LOCAL_SOURCE);
        }
    }

    /// Takes the DAC back for local playback, running the active external
    /// source's `preempt` command. Returns where local playback was interrupted.
    pub async fn claim_local(&self) -> Option<(u32, f64)> {
        let mut arbiter = self.arbiter.lock().await;
        let active = self.source().active;
        if active != LOCAL_SOURCE {
            info!("taking the DAC back from {active}");
            if let Some(command) = self.config.sources.get(&active).and_then(|s| s.preempt.as_deref()) {
                run_shell(command).await;
            }
            self.set_active_source(LOCAL_SOURCE);
            self.wait_until_dac_free(Duration::from_secs(3)).await;
        }
        arbiter.resume.take()
    }

    async fn wait_until_dac_free(&self, timeout: Duration) {
        let path = self.config.hw_params_path(&self.active_card());
        let deadline = Instant::now() + timeout;
        while matches!(read_hw(&path).await, Hw::Open(_)) {
            if Instant::now() >= deadline {
                warn!("DAC still busy after {timeout:?}");
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

async fn run_shell(command: &str) {
    let status = tokio::process::Command::new("sh").arg("-c").arg(command).kill_on_drop(true).status();
    match tokio::time::timeout(Duration::from_secs(5), status).await {
        Ok(Ok(status)) if status.success() => {}
        Ok(Ok(status)) => warn!("`{command}` exited with {status}"),
        Ok(Err(e)) => warn!("`{command}` failed to start: {e}"),
        Err(_) => warn!("`{command}` timed out"),
    }
}
