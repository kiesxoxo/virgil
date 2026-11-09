use std::ops::Deref;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use tokio::sync::{Mutex, broadcast};
use tracing::{info, warn};
use virgil_core::{
    AddMode, Added, HomekitStatus, LOCAL_SOURCE, Outputs, Party, PlayState, PlayerState, Queue, QueueItem, ServerEvent,
    SignalPath, SourceState,
};
use virgil_engine_mpd::MpdEngine;
use virgil_homekit::Switch;
use virgil_library::Library;
use virgil_radio::Nts;

use crate::api::ApiError;
use crate::config::Config;
use crate::party::{Caller, PartyStore};
use crate::signal::{Hw, card_label, parse_cards, read_hw};
use crate::turns;

/// Latest state of everything the UI shows; `hello` sends it whole.
#[derive(Default)]
struct Snapshot {
    player: PlayerState,
    queue: Queue,
    signal: SignalPath,
    source: SourceState,
    outputs: Outputs,
    homekit: Option<HomekitStatus>,
    party: Party,
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
    pub radio: Nts,
    /// The HomeKit accessory, when enabled and running.
    pub homekit: Option<Switch>,
    pub events: broadcast::Sender<ServerEvent>,
    /// The request line: people, who picked what, votes.
    pub party: PartyStore,
    snapshot: RwLock<Snapshot>,
    arbiter: Mutex<Arbiter>,
    /// Held across every change to the queue, so requests from several phones at once don't
    /// work from positions another one just shifted, and picks are recorded before the queue
    /// is read back.
    queue_edit: Mutex<()>,
}

/// Where a request goes.
#[derive(Clone, Copy, PartialEq)]
enum Place {
    /// Instead of everything (the queue is cleared first).
    Clear,
    /// Right after the song playing.
    Next,
    /// In the requester's turn.
    Turn,
    End,
}

impl Deref for App {
    type Target = Inner;
    fn deref(&self) -> &Inner {
        &self.0
    }
}

impl App {
    pub fn new(config: Config, library: Library, radio: Nts, homekit: Option<Switch>) -> Self {
        let source = SourceState { active: LOCAL_SOURCE.into(), available: config.sources.keys().cloned().collect() };
        let party = PartyStore::load(config.data_dir.join("party.json"));
        let snapshot = Snapshot { source, party: party.party(), ..Snapshot::default() };
        App(Arc::new(Inner {
            mpd: MpdEngine::new(&config.mpd),
            library,
            radio,
            homekit,
            events: broadcast::channel(256).0,
            party,
            snapshot: RwLock::new(snapshot),
            arbiter: Mutex::default(),
            queue_edit: Mutex::default(),
            config,
        }))
    }

    /// `hello` with the player read from MPD just now. The stored state only
    /// changes when MPD reports a change (play, pause, seek, next song), so
    /// mid-song its position is stale; a page connecting then must get where
    /// playback really is. If MPD can't be reached, the stored state is sent.
    pub async fn hello_live(&self) -> ServerEvent {
        self.refresh(false).await;
        self.hello()
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
            homekit: s.homekit.clone(),
            party: s.party.clone(),
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
        // Cards connected since MPD started, where outputs are named after ALSA
        // cards (the container image).
        let new_cards = if outputs.iter().any(|o| o.plugin == "alsa" && cards.iter().any(|(id, _)| *id == o.name)) {
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

    /// Broadcasts the request line if it changed (people, who is here, votes).
    pub fn refresh_party(&self) {
        self.update(|s| &mut s.party, self.party.party(), ServerEvent::Party);
    }

    /// Tells everyone something happened in the room.
    pub fn shout(&self, text: String) {
        let _ = self.events.send(ServerEvent::Shout { text });
    }

    /// Who `token` belongs to, noting they are around.
    pub fn caller(&self, token: Option<&str>) -> Caller {
        let (caller, changed) = self.party.caller(token);
        if changed {
            self.refresh_party();
        }
        caller
    }

    /// The queue item playing (or paused, or stopped at).
    pub fn current(&self) -> Option<QueueItem> {
        let s = self.snapshot.read().expect("snapshot lock");
        s.queue.items.iter().find(|item| Some(item.id) == s.player.song_id).cloned()
    }

    /// Lets the request line follow the song playing: votes start over, the aircheck logs it.
    fn follow_current(&self) {
        let playing = self.player().state == PlayState::Play;
        self.party.now_playing(self.current().as_ref(), playing);
        self.refresh_party();
    }

    pub fn set_homekit(&self, status: HomekitStatus) {
        self.update(|s| &mut s.homekit, Some(status), |s| ServerEvent::Homekit(s.expect("just set")));
    }

    fn set_active_source(&self, name: &str) {
        let mut source = self.source();
        source.active = name.to_string();
        self.update(|s| &mut s.source, source, ServerEvent::Source);
    }

    /// Adds what the library and the request line know: album, artwork, who asked for it.
    fn enrich(&self, mut items: Vec<QueueItem>) -> Vec<QueueItem> {
        for item in &mut items {
            if let Some((album_id, art)) = self.library.lookup(&item.uri) {
                item.album_id = Some(album_id);
                item.art = art;
            }
        }
        self.party.annotate(&mut items);
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
            let _edit = self.queue_edit.lock().await;
            match self.mpd.queue().await {
                Ok(items) => {
                    self.party.reconcile(&items);
                    self.set_queue(Queue { version, items: self.enrich(items) });
                }
                Err(e) => warn!("mpd queue: {e}"),
            }
        }
        self.follow_current();
    }

    /// Adds `uris` for `caller`. With the request line open nothing replaces the queue: guests'
    /// requests take their turn, and a host's "play" plays the songs next. `start` is the index
    /// of the song to play, if `play`.
    pub async fn request(
        &self,
        caller: &Caller,
        uris: &[String],
        mode: AddMode,
        play: bool,
        start: usize,
        note: Option<String>,
    ) -> Result<Added, ApiError> {
        let place = match (caller.open, caller.host, mode) {
            (false, _, AddMode::Replace) => Place::Clear,
            (false, _, AddMode::Next) | (true, true, AddMode::Replace | AddMode::Next) => Place::Next,
            (false, _, AddMode::Append) => Place::End,
            (true, _, _) if caller.rules.take_turns => Place::Turn,
            (true, _, _) => Place::End,
        };
        let play = play && caller.host;
        let start = if play { start } else { 0 };
        let edit = self.queue_edit.lock().await;
        let status = self.mpd.status().await?;
        let items = self.enrich(self.mpd.queue().await?);
        let current = status.song_pos.map(|p| p as usize).filter(|&p| p < items.len());
        // The queue played out while the line is open (stopped, nothing after the song MPD stopped
        // at): a request goes at the end, and the music starts again (unless AirPlay has the DAC).
        let idle = caller.open && status.state == PlayState::Stop && current.is_none_or(|p| p + 1 == items.len());
        let (place, play) = if idle && place != Place::Clear {
            (Place::End, play || self.source().active == LOCAL_SOURCE)
        } else {
            (place, play)
        };
        let base = if idle { items.len() } else { current.map_or(0, |p| p + 1) };
        let upcoming = &items[base..];
        let (at, index) = match place {
            Place::Clear => (None, 0),
            Place::Next if current.is_some() => (Some(base as u32), 0),
            Place::Next | Place::End => (None, upcoming.len()),
            Place::Turn => {
                let index = turns::place(upcoming, caller.id().unwrap_or_default());
                (Some((base + index) as u32), index)
            }
        };
        let ids = self.mpd.add_ids(uris, at, place == Place::Clear).await?;
        if let Some(person) = &caller.person {
            self.party.record(&ids, uris, &person.id, note);
        }
        drop(edit);
        self.reenrich_queue();

        let mut ahead: f64 = upcoming[..index].iter().map(|item| item.duration).sum();
        if current.is_some() && place != Place::Clear {
            ahead += (status.duration - status.elapsed).max(0.0);
        }
        let mut position = if place == Place::Clear { 1 } else { index as u32 + 1 };
        if play {
            self.claim_local().await;
            self.mpd.play_id(ids[start.min(ids.len() - 1)]).await?;
            (position, ahead) = (0, 0.0);
        }
        Ok(Added { ids, position, ahead })
    }

    /// Removes one queue item; guests may only remove what they asked for.
    pub async fn remove(&self, caller: &Caller, id: u32) -> Result<(), ApiError> {
        let _edit = self.queue_edit.lock().await;
        if !caller.host {
            let item = self.queue().items.into_iter().find(|i| i.id == id);
            let item = item.ok_or_else(|| ApiError::not_found("that song isn't in the queue"))?;
            self.check_owner(caller, &item)?;
        }
        Ok(self.mpd.delete_id(id).await?)
    }

    /// Removes what is left of a request: its songs from the one playing on. Removing the song
    /// playing skips to the next.
    pub async fn remove_request(&self, caller: &Caller, request: u32) -> Result<(), ApiError> {
        let _edit = self.queue_edit.lock().await;
        let queue = self.queue().items;
        let from = self.current().and_then(|c| queue.iter().position(|i| i.id == c.id)).unwrap_or(0);
        let songs: Vec<&QueueItem> =
            queue[from..].iter().filter(|i| i.pick.as_ref().is_some_and(|p| p.request == request)).collect();
        let first = songs.first().ok_or_else(|| ApiError::not_found("that request isn't in the queue"))?;
        if !caller.host {
            self.check_owner(caller, first)?;
        }
        if songs.iter().any(|i| Some(i.id) == self.player().song_id) {
            self.party.skipping();
        }
        for item in songs {
            self.mpd.delete_id(item.id).await?;
        }
        Ok(())
    }

    fn check_owner(&self, caller: &Caller, item: &QueueItem) -> Result<(), ApiError> {
        let by = item.pick.as_ref().map(|p| p.by.as_str());
        if by.is_some() && by == caller.id() {
            return Ok(());
        }
        let whose = by.and_then(|id| self.party.person(id)).map_or("someone else".into(), |p| p.name);
        Err(ApiError::forbidden(format!("That's {whose}'s request")))
    }

    pub async fn move_item(&self, id: u32, to: u32) -> Result<(), ApiError> {
        let _edit = self.queue_edit.lock().await;
        Ok(self.mpd.move_id(id, to).await?)
    }

    pub async fn clear(&self) -> Result<(), ApiError> {
        let _edit = self.queue_edit.lock().await;
        Ok(self.mpd.clear_except_current().await?)
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

    /// Pauses local playback. Radio stops instead: resuming then picks it up
    /// live rather than minutes behind, or not at all once the server hangs up.
    pub async fn pause_local(&self) -> Result<(), ApiError> {
        if self.playing_stream() {
            self.mpd.stop().await?;
        } else {
            self.mpd.pause(true).await?;
        }
        Ok(())
    }

    /// Silences whatever plays: ends an external source's session, pauses
    /// local playback. Where an external source interrupted local playback is
    /// kept, so playing again resumes there.
    pub async fn turn_off(&self) -> Result<(), ApiError> {
        {
            let _arbiter = self.arbiter.lock().await;
            self.end_external().await;
        }
        if self.mpd.status().await?.state == PlayState::Play {
            self.pause_local().await?;
        }
        Ok(())
    }

    /// The current queue item is an internet radio stream.
    pub fn playing_stream(&self) -> bool {
        let s = self.snapshot.read().expect("snapshot lock");
        s.queue.items.iter().any(|item| Some(item.id) == s.player.song_id && is_stream(&item.uri))
    }

    /// Plays a radio stream now, right after the current song, so the queue
    /// carries on with the next song after it. A station that was playing
    /// makes way for this one instead of piling up in the queue.
    pub async fn play_stream(&self, uri: &str) -> Result<(), ApiError> {
        self.claim_local().await;
        let status = self.mpd.status().await?;
        let current = self.queue().items.into_iter().find(|item| Some(item.id) == status.song_id);
        if let Some(current) = current.as_ref().filter(|item| item.uri == uri) {
            // Already the current item: (re)start it, live.
            return Ok(self.mpd.play_id(current.id).await?);
        }
        let _edit = self.queue_edit.lock().await;
        let at = status.song_pos.map(|pos| pos + 1);
        let ids = self.mpd.add_ids(&[uri.to_string()], at, false).await?;
        self.mpd.play_id(ids[0]).await?;
        if let Some(previous) = current.filter(|item| is_stream(&item.uri)) {
            self.mpd.delete_id(previous.id).await?;
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
        self.end_external().await;
        arbiter.resume.take()
    }

    /// Makes the active external source let go of the DAC (its `preempt`
    /// command) and waits for the device. Call with the arbiter locked.
    async fn end_external(&self) {
        let active = self.source().active;
        if active != LOCAL_SOURCE {
            info!("taking the DAC back from {active}");
            if let Some(command) = self.config.sources.get(&active).and_then(|s| s.preempt.as_deref()) {
                run_shell(command).await;
            }
            self.set_active_source(LOCAL_SOURCE);
            self.wait_until_dac_free(Duration::from_secs(3)).await;
        }
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

/// An internet stream rather than a file from the library.
pub fn is_stream(uri: &str) -> bool {
    uri.starts_with("http://") || uri.starts_with("https://")
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
