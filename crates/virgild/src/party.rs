//! The request line: who is who, who picked what, who is around, the votes on the song playing,
//! and the set list of the night. Kept in `data_dir/party.json`; presence and votes live in memory.

use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tracing::warn;
use virgil_core::{Aircheck, Party, Person, Pick, Played, ProfileRequest, QueueItem, Rules, Votes};

/// Someone counts as here this long after they last used Virgil, so a phone that locks its
/// screen stays in the room.
const HERE_FOR: Duration = Duration::from_secs(15 * 60);

/// How many call-sign colours there are.
pub const COLORS: u8 = 8;

const MAX_NAME: usize = 24;
const MAX_NOTE: usize = 80;

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Saved {
    people: Vec<Member>,
    picks: Vec<Picked>,
    next_request: u32,
    open: bool,
    rules: Rules,
    /// The night in progress or, while the line is closed, the last one.
    aircheck: Option<Aircheck>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Member {
    person: Person,
    /// The devices that are this person.
    tokens: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Picked {
    /// MPD's queue id.
    id: u32,
    uri: String,
    /// Where it was in the queue when last seen: MPD restores its queue in order, so after a
    /// restart this tells two copies of one song apart.
    #[serde(default)]
    pos: Option<u32>,
    pick: Pick,
}

#[derive(Default)]
struct Presence {
    sockets: u32,
    seen: Option<Instant>,
}

struct State {
    saved: Saved,
    presence: HashMap<String, Presence>,
    votes: Votes,
    /// Queue id of the song last written to the aircheck.
    logged: Option<u32>,
    /// MPD may have renumbered its queue (it restarted): match picks by URI too.
    rematch: bool,
}

/// Who is asking, and what they may do.
#[derive(Debug, Clone)]
pub struct Caller {
    pub person: Option<Person>,
    /// Hosts keep every control; with the line closed, everyone is one.
    pub host: bool,
    pub open: bool,
    pub rules: Rules,
}

impl Caller {
    pub fn id(&self) -> Option<&str> {
        self.person.as_ref().map(|p| p.id.as_str())
    }
}

pub struct PartyStore {
    /// None in tests: nothing is saved.
    path: Option<PathBuf>,
    state: Mutex<State>,
}

/// Skip votes it takes with `here` people around: half of them, at least two, but never more than
/// there are.
pub fn votes_needed(here: usize) -> u32 {
    let here = here.max(1) as u32;
    here.div_ceil(2).max(2).min(here)
}

pub fn now() -> u32 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as u32)
}

/// A dedication as stored: trimmed, at most 80 characters, or none.
pub fn clean_note(note: Option<String>) -> Option<String> {
    let note = note?.split_whitespace().collect::<Vec<_>>().join(" ");
    (!note.is_empty()).then(|| note.chars().take(MAX_NOTE).collect())
}

fn clean_name(name: &str) -> String {
    let name: String = name.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_NAME).collect();
    if name.is_empty() { "Guest".into() } else { name }
}

/// Up to three letters or digits, in capitals; the name's initials when none are given.
fn clean_sign(sign: &str, name: &str) -> String {
    let sign: String = sign.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_uppercase).take(3).collect();
    if !sign.is_empty() {
        return sign;
    }
    let initials: String =
        name.split_whitespace().filter_map(|w| w.chars().next()).flat_map(char::to_uppercase).take(3).collect();
    if initials.is_empty() { "?".into() } else { initials }
}

/// Device tokens are random strings the web app makes; anything else is ignored.
fn valid_token(token: &str) -> bool {
    (16..=64).contains(&token.len()) && token.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

impl State {
    fn member(&self, token: &str) -> Option<&Member> {
        self.saved.people.iter().find(|m| m.tokens.iter().any(|t| t == token))
    }

    fn person(&self, id: &str) -> Option<&Person> {
        self.saved.people.iter().map(|m| &m.person).find(|p| p.id == id)
    }

    fn here(&self) -> Vec<String> {
        let now = Instant::now();
        let mut here: Vec<String> = self
            .presence
            .iter()
            .filter(|(id, p)| {
                self.person(id).is_some()
                    && (p.sockets > 0 || p.seen.is_some_and(|seen| now.duration_since(seen) < HERE_FOR))
            })
            .map(|(id, _)| id.clone())
            .collect();
        here.sort();
        here
    }

    fn party(&self) -> Party {
        let here = self.here();
        let votes = Votes { need: votes_needed(here.len()), ..self.votes.clone() };
        Party {
            open: self.saved.open,
            rules: self.saved.rules,
            people: self.saved.people.iter().map(|m| m.person.clone()).collect(),
            here,
            votes,
        }
    }

    fn new_id(&self) -> String {
        loop {
            let n = std::collections::hash_map::RandomState::new().hash_one(SystemTime::now());
            let id = format!("{:08x}", n as u32);
            if self.person(&id).is_none() {
                return id;
            }
        }
    }

    /// The aircheck entry of the song playing, if it was logged.
    fn logged_entry(&mut self) -> Option<&mut Played> {
        if self.logged.is_none() || self.logged != self.votes.song_id {
            return None;
        }
        self.saved.aircheck.as_mut()?.played.last_mut()
    }
}

impl PartyStore {
    pub fn load(path: PathBuf) -> Self {
        let saved = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                warn!("{}: {e}; starting the request line afresh", path.display());
                Saved::default()
            }),
            Err(_) => Saved::default(),
        };
        Self::with(Some(path), saved)
    }

    #[cfg(test)]
    fn in_memory() -> Self {
        Self::with(None, Saved::default())
    }

    fn with(path: Option<PathBuf>, saved: Saved) -> Self {
        let state = State { saved, presence: HashMap::new(), votes: Votes::default(), logged: None, rematch: true };
        Self { path, state: Mutex::new(state) }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().expect("party lock")
    }

    fn save(&self, state: &State) {
        let Some(path) = &self.path else { return };
        let write = || -> std::io::Result<()> {
            let tmp = path.with_extension("json.tmp");
            let mut file =
                std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
            file.write_all(&serde_json::to_vec(&state.saved).expect("party state serializes"))?;
            file.sync_all()?;
            std::fs::rename(&tmp, path)
        };
        if let Err(e) = write() {
            warn!("saving {}: {e}", path.display());
        }
    }

    pub fn party(&self) -> Party {
        self.lock().party()
    }

    pub fn is_open(&self) -> bool {
        self.lock().saved.open
    }

    /// Who `token` belongs to, noting that they are around. Also returns whether the room changed.
    pub fn caller(&self, token: Option<&str>) -> (Caller, bool) {
        let mut state = self.lock();
        let person = token.filter(|t| valid_token(t)).and_then(|t| state.member(t)).map(|m| m.person.clone());
        let mut changed = false;
        if let Some(person) = &person {
            let before = state.here();
            state.presence.entry(person.id.clone()).or_default().seen = Some(Instant::now());
            changed = state.here() != before;
        }
        let open = state.saved.open;
        let host = !open || person.as_ref().is_some_and(|p| p.host);
        (Caller { person, host, open, rules: state.saved.rules }, changed)
    }

    /// Creates or updates the person behind `token`. People first seen while the line is closed
    /// are hosts (the household's own devices); those who call in while it is open are guests.
    pub fn register(&self, token: &str, profile: &ProfileRequest) -> Option<Person> {
        if !valid_token(token) {
            return None;
        }
        let mut state = self.lock();
        let name = clean_name(&profile.name);
        let sign = clean_sign(&profile.sign, &name);
        let color = profile.color % COLORS;
        let person = if let Some(member) = state.saved.people.iter_mut().find(|m| m.tokens.iter().any(|t| t == token)) {
            member.person.name = name;
            member.person.sign = sign;
            member.person.color = color;
            member.person.clone()
        } else {
            let person = Person { id: state.new_id(), name, sign, color, host: !state.saved.open };
            state.saved.people.push(Member { person: person.clone(), tokens: vec![token.to_string()] });
            person
        };
        state.presence.entry(person.id.clone()).or_default().seen = Some(Instant::now());
        self.save(&state);
        Some(person)
    }

    /// Makes the device behind `token` one of `id`'s devices. Only a host may become a host this way.
    pub fn link(&self, token: &str, id: &str, caller: &Caller) -> Result<Person, &'static str> {
        if !valid_token(token) {
            return Err("this device has no token");
        }
        let mut state = self.lock();
        let target = state.person(id).cloned().ok_or("no such person")?;
        if target.host && !caller.host {
            return Err("only a host can link a device to a host");
        }
        for member in &mut state.saved.people {
            member.tokens.retain(|t| t != token);
        }
        let picked: HashSet<String> = state.saved.picks.iter().map(|p| p.pick.by.clone()).collect();
        state.saved.people.retain(|m| !m.tokens.is_empty() || m.person.id == id || picked.contains(&m.person.id));
        let member = state.saved.people.iter_mut().find(|m| m.person.id == id).expect("target kept");
        member.tokens.push(token.to_string());
        self.save(&state);
        Ok(target)
    }

    pub fn set_host(&self, id: &str, host: bool) -> bool {
        let mut state = self.lock();
        let Some(member) = state.saved.people.iter_mut().find(|m| m.person.id == id) else { return false };
        member.person.host = host;
        self.save(&state);
        true
    }

    /// Forgets a person and their devices. What they requested stays in the queue, unattributed.
    pub fn forget(&self, id: &str) -> bool {
        let mut state = self.lock();
        let before = state.saved.people.len();
        state.saved.people.retain(|m| m.person.id != id);
        state.saved.picks.retain(|p| p.pick.by != id);
        state.presence.remove(id);
        let forgot = state.saved.people.len() != before;
        if forgot {
            self.save(&state);
        }
        forgot
    }

    /// Opens or closes the line, or changes its rules. Opening starts a new aircheck; closing ends it.
    pub fn set(&self, open: Option<bool>, rules: Option<Rules>) {
        let mut state = self.lock();
        if let Some(rules) = rules {
            state.saved.rules = rules;
        }
        match open {
            Some(true) if !state.saved.open => {
                state.saved.open = true;
                state.saved.aircheck = Some(Aircheck { opened: now(), ..Aircheck::default() });
                state.logged = None;
            }
            Some(false) if state.saved.open => {
                state.saved.open = false;
                if let Some(aircheck) = &mut state.saved.aircheck {
                    aircheck.closed = Some(now());
                }
                state.votes = Votes { song_id: state.votes.song_id, ..Votes::default() };
            }
            _ => {}
        }
        self.save(&state);
    }

    pub fn aircheck(&self) -> Option<Aircheck> {
        self.lock().saved.aircheck.clone()
    }

    /// Notes that `by` requested the songs just added as `ids` (with `uris`); returns the request.
    pub fn record(&self, ids: &[u32], uris: &[String], by: &str, note: Option<String>) -> u32 {
        let mut state = self.lock();
        state.saved.next_request = state.saved.next_request.max(1);
        let request = state.saved.next_request;
        state.saved.next_request += 1;
        let pick = Pick { by: by.to_string(), request, at: now(), note };
        for (id, uri) in ids.iter().zip(uris) {
            state.saved.picks.push(Picked { id: *id, uri: uri.clone(), pos: None, pick: pick.clone() });
        }
        self.save(&state);
        request
    }

    /// MPD reconnected: it may have restarted and renumbered its queue.
    pub fn mpd_reconnected(&self) {
        self.lock().rematch = true;
    }

    /// Keeps picks in step with MPD's queue: those whose song left it are dropped. After MPD
    /// restarts (new ids for the same songs), the rest are matched by where they were, and failing
    /// that by URI, in queue order.
    pub fn reconcile(&self, items: &[QueueItem]) {
        let mut state = self.lock();
        let rematch = std::mem::take(&mut state.rematch);
        let by_id: HashMap<u32, &QueueItem> = items.iter().map(|i| (i.id, i)).collect();
        let (mut kept, mut rest): (Vec<Picked>, Vec<Picked>) = std::mem::take(&mut state.saved.picks)
            .into_iter()
            .partition(|p| by_id.get(&p.id).is_some_and(|i| i.uri == p.uri));
        let mut changed = !rest.is_empty();
        if rematch && !rest.is_empty() {
            let mut picked: HashSet<u32> = kept.iter().map(|p| p.id).collect();
            // The same place first, then the first copy of the song nobody has.
            rest.retain(|pick| {
                let same = pick.pos.and_then(|pos| items.get(pos as usize)).filter(|item| item.uri == pick.uri);
                match same.filter(|item| !picked.contains(&item.id)) {
                    Some(item) => {
                        picked.insert(item.id);
                        kept.push(Picked { id: item.id, ..pick.clone() });
                        false
                    }
                    None => true,
                }
            });
            for item in items.iter().filter(|i| !picked.contains(&i.id)) {
                if let Some(at) = rest.iter().position(|p| p.uri == item.uri) {
                    let mut pick = rest.remove(at);
                    pick.id = item.id;
                    kept.push(pick);
                }
            }
        }
        let positions: HashMap<u32, u32> = items.iter().enumerate().map(|(i, item)| (item.id, i as u32)).collect();
        for pick in &mut kept {
            let pos = positions.get(&pick.id).copied();
            changed |= pick.pos != pos;
            pick.pos = pos;
        }
        state.saved.picks = kept;
        if changed {
            self.save(&state);
        }
    }

    pub fn annotate(&self, items: &mut [QueueItem]) {
        let state = self.lock();
        let picks: HashMap<u32, &Pick> = state.saved.picks.iter().map(|p| (p.id, &p.pick)).collect();
        for item in items {
            item.pick = picks.get(&item.id).map(|&p| p.clone());
        }
    }

    pub fn connect(&self, id: &str) -> bool {
        let mut state = self.lock();
        let before = state.here();
        let presence = state.presence.entry(id.to_string()).or_default();
        presence.sockets += 1;
        presence.seen = Some(Instant::now());
        state.here() != before
    }

    pub fn disconnect(&self, id: &str) -> bool {
        let mut state = self.lock();
        let before = state.here();
        if let Some(presence) = state.presence.get_mut(id) {
            presence.sockets = presence.sockets.saturating_sub(1);
            presence.seen = Some(Instant::now());
        }
        state.here() != before
    }

    /// The song playing changed or started: votes start over, and while the line is open the
    /// song goes on the aircheck. Returns whether the party changed.
    pub fn now_playing(&self, current: Option<&QueueItem>, playing: bool) -> bool {
        let mut state = self.lock();
        let id = current.map(|c| c.id);
        let mut changed = false;
        if state.votes.song_id != id {
            state.votes = Votes { song_id: id, ..Votes::default() };
            changed = true;
        }
        if let (true, true, Some(item)) = (state.saved.open, playing, current)
            && state.logged != Some(item.id)
        {
            state.logged = Some(item.id);
            let person = item.pick.as_ref().and_then(|p| state.person(&p.by)).cloned();
            let aircheck =
                state.saved.aircheck.get_or_insert_with(|| Aircheck { opened: now(), ..Aircheck::default() });
            if let Some(person) = person {
                aircheck.people.retain(|p| p.id != person.id);
                aircheck.people.push(person);
            }
            aircheck.played.push(Played {
                uri: item.uri.clone(),
                title: item.title.clone(),
                artist: item.artist.clone(),
                album: item.album.clone(),
                album_id: item.album_id.clone(),
                art: item.art.clone(),
                duration: item.duration,
                at: now(),
                pick: item.pick.clone(),
                cheers: 0,
                skip_votes: 0,
                skipped: false,
            });
            self.save(&state);
        }
        changed
    }

    /// Toggles `who`'s skip vote on song `id`; returns the votes for it and how many it takes.
    pub fn vote_skip(&self, who: &str, id: u32) -> (u32, u32) {
        let mut state = self.lock();
        if state.votes.song_id != Some(id) {
            state.votes = Votes { song_id: Some(id), ..Votes::default() };
        }
        toggle(&mut state.votes.skip, who);
        let votes = state.votes.skip.len() as u32;
        if let Some(entry) = state.logged_entry() {
            entry.skip_votes = votes;
        }
        (votes, votes_needed(state.here().len()))
    }

    pub fn cheer(&self, who: &str, id: u32) {
        let mut state = self.lock();
        if state.votes.song_id != Some(id) {
            state.votes = Votes { song_id: Some(id), ..Votes::default() };
        }
        toggle(&mut state.votes.cheer, who);
        let cheers = state.votes.cheer.len() as u32;
        if let Some(entry) = state.logged_entry() {
            entry.cheers = cheers;
        }
        self.save(&state);
    }

    /// The song playing is being skipped (the aircheck says so).
    pub fn skipping(&self) {
        let mut state = self.lock();
        if let Some(entry) = state.logged_entry() {
            entry.skipped = true;
            self.save(&state);
        }
    }

    pub fn person(&self, id: &str) -> Option<Person> {
        self.lock().person(id).cloned()
    }
}

fn toggle(ids: &mut Vec<String>, id: &str) {
    if let Some(at) = ids.iter().position(|i| i == id) {
        ids.remove(at);
    } else {
        ids.push(id.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLA: &str = "ola-device-token-0001";
    const JAN: &str = "jan-device-token-0002";

    fn profile(name: &str) -> ProfileRequest {
        ProfileRequest { name: name.into(), sign: String::new(), color: 9, join: None }
    }

    fn item(id: u32, uri: &str) -> QueueItem {
        QueueItem {
            id,
            uri: uri.into(),
            title: uri.into(),
            artist: String::new(),
            album: String::new(),
            duration: 60.0,
            format: None,
            album_id: None,
            art: None,
            pick: None,
        }
    }

    #[test]
    fn cleans_profiles() {
        let store = PartyStore::in_memory();
        let ola = store.register(OLA, &profile("  Ola   Nowak ")).unwrap();
        assert_eq!((ola.name.as_str(), ola.sign.as_str(), ola.color), ("Ola Nowak", "ON", 1));
        let ola = store
            .register(OLA, &ProfileRequest { name: String::new(), sign: "dj.x!yz".into(), color: 3, join: None })
            .unwrap();
        assert_eq!((ola.name.as_str(), ola.sign.as_str(), ola.color), ("Guest", "DJX", 3));
        assert!(store.register("short", &profile("Nope")).is_none());
        assert_eq!(clean_note(Some("  for   Marcin ".into())).as_deref(), Some("for Marcin"));
        assert_eq!(clean_note(Some("   ".into())), None);
        assert_eq!(clean_note(Some("x".repeat(200))).unwrap().len(), 80);
    }

    #[test]
    fn hosts_are_the_devices_seen_before_the_line_opened() {
        let store = PartyStore::in_memory();
        let ola = store.register(OLA, &profile("Ola")).unwrap();
        assert!(ola.host);
        // Closed: everyone is a host, even without a token.
        assert!(store.caller(None).0.host);
        store.set(Some(true), None);
        let jan = store.register(JAN, &profile("Jan")).unwrap();
        assert!(!jan.host);
        assert!(store.caller(Some(OLA)).0.host);
        assert!(!store.caller(Some(JAN)).0.host);
        assert!(!store.caller(None).0.host);
        // A guest can't link their phone to a host, but a host can promote them.
        assert!(store.link(JAN, &ola.id, &store.caller(Some(JAN)).0).is_err());
        assert!(store.set_host(&jan.id, true));
        assert!(store.caller(Some(JAN)).0.host);
        store.set(Some(false), None);
        assert!(store.caller(None).0.host);
    }

    #[test]
    fn linking_moves_a_device_and_drops_the_empty_person() {
        let store = PartyStore::in_memory();
        let ola = store.register(OLA, &profile("Ola")).unwrap();
        store.register(JAN, &profile("Velvet Moth")).unwrap();
        let caller = store.caller(Some(JAN)).0;
        assert_eq!(store.link(JAN, &ola.id, &caller).unwrap().id, ola.id);
        assert_eq!(store.caller(Some(JAN)).0.id(), Some(ola.id.as_str()));
        assert_eq!(store.party().people.len(), 1);
    }

    #[test]
    fn picks_follow_the_queue_and_survive_renumbering() {
        let store = PartyStore::in_memory();
        let mut queue = vec![item(1, "a"), item(2, "b"), item(3, "c")];
        store.reconcile(&queue); // the startup pass
        let request = store.record(&[2, 3], &["b".into(), "c".into()], "ola", None);
        store.annotate(&mut queue);
        assert_eq!(queue[1].pick.as_ref().unwrap().request, request);
        assert!(queue[0].pick.is_none());

        // Song 2 is removed: its pick goes, song 3 keeps its own.
        let mut queue = vec![item(1, "a"), item(3, "c")];
        store.reconcile(&queue);
        store.annotate(&mut queue);
        assert_eq!(queue[1].pick.as_ref().map(|p| p.by.as_str()), Some("ola"));

        // A different song under an old id doesn't inherit the pick, and without a restart
        // nothing is matched by URI.
        let mut queue = vec![item(1, "a"), item(9, "c"), item(3, "x")];
        store.record(&[1], &["a".into()], "jan", None);
        store.reconcile(&queue);
        store.annotate(&mut queue);
        assert!(queue[1].pick.is_none() && queue[2].pick.is_none());
        assert_eq!(queue[0].pick.as_ref().map(|p| p.by.as_str()), Some("jan"));

        // MPD restarted and numbered its queue afresh: picks are found by URI.
        store.record(&[3], &["x".into()], "ola", None);
        let mut queue = vec![item(10, "a"), item(11, "x")];
        store.mpd_reconnected();
        store.reconcile(&queue);
        store.annotate(&mut queue);
        assert_eq!(queue[0].pick.as_ref().map(|p| p.by.as_str()), Some("jan"));
        assert_eq!(queue[1].pick.as_ref().map(|p| p.by.as_str()), Some("ola"));
    }

    #[test]
    fn after_a_restart_a_pick_stays_on_its_own_copy_of_a_song() {
        let store = PartyStore::in_memory();
        store.reconcile(&[]);
        let mut queue = vec![item(1, "a"), item(2, "b"), item(3, "a")];
        store.record(&[3], &["a".into()], "ola", None);
        store.reconcile(&queue);
        // MPD restarted: the same queue, new ids. Ola asked for the second "a", not the first.
        queue = vec![item(10, "a"), item(11, "b"), item(12, "a")];
        store.mpd_reconnected();
        store.reconcile(&queue);
        store.annotate(&mut queue);
        assert!(queue[0].pick.is_none());
        assert_eq!(queue[2].pick.as_ref().map(|p| p.by.as_str()), Some("ola"));
    }

    #[test]
    fn skip_votes_need_half_the_room() {
        assert_eq!([0, 1, 2, 3, 4, 5, 6].map(votes_needed), [1, 1, 2, 2, 2, 3, 3]);
        let store = PartyStore::in_memory();
        let ola = store.register(OLA, &profile("Ola")).unwrap();
        let jan = store.register(JAN, &profile("Jan")).unwrap();
        assert_eq!(store.vote_skip(&ola.id, 5), (1, 2));
        assert_eq!(store.vote_skip(&jan.id, 5), (2, 2));
        // Voting again takes the vote back; a new song starts over.
        assert_eq!(store.vote_skip(&jan.id, 5), (1, 2));
        assert_eq!(store.vote_skip(&jan.id, 6), (1, 2));
    }

    #[test]
    fn the_aircheck_logs_what_played_while_the_line_was_open() {
        let store = PartyStore::in_memory();
        let ola = store.register(OLA, &profile("Ola")).unwrap();
        let mut song = item(1, "a");
        song.pick = Some(Pick { by: ola.id.clone(), request: 1, at: 0, note: Some("for Jan".into()) });
        store.now_playing(Some(&song), true);
        assert!(store.aircheck().is_none(), "nothing is logged while the line is closed");

        store.set(Some(true), None);
        store.now_playing(Some(&song), true);
        store.now_playing(Some(&song), true);
        store.cheer(&ola.id, 1);
        store.skipping();
        store.now_playing(Some(&item(2, "b")), false);
        store.set(Some(false), None);

        let aircheck = store.aircheck().unwrap();
        assert!(aircheck.closed.is_some());
        assert_eq!(aircheck.people, [ola]);
        assert_eq!(aircheck.played.len(), 1, "a song is logged once, and only when it plays");
        let played = &aircheck.played[0];
        assert_eq!((played.cheers, played.skipped), (1, true));
        assert_eq!(played.pick.as_ref().unwrap().note.as_deref(), Some("for Jan"));
    }

    #[test]
    fn saves_and_loads() {
        let dir = std::env::temp_dir().join(format!("virgil-party-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("party.json");
        let store = PartyStore::load(path.clone());
        let ola = store.register(OLA, &profile("Ola")).unwrap();
        store.record(&[4], &["a".into()], &ola.id, Some("hi".into()));
        store.set(Some(true), Some(Rules { take_turns: false, ..Rules::default() }));

        let store = PartyStore::load(path.clone());
        let party = store.party();
        assert!(party.open && !party.rules.take_turns);
        assert_eq!(party.people.len(), 1);
        assert_eq!(party.people[0], ola);
        assert_eq!(store.caller(Some(OLA)).0.id(), Some(ola.id.as_str()));
        let mut queue = vec![item(4, "a")];
        store.annotate(&mut queue);
        assert_eq!(queue[0].pick.as_ref().unwrap().note.as_deref(), Some("hi"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
