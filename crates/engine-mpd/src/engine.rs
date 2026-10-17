use hifi_core::{AddMode, AudioFormat, ModesRequest, PlayState, PlayerState, QueueItem};
use tokio::sync::mpsc;

use crate::client::{self, Event, Mpd};
use crate::proto::{Error, Result, quote};

/// Artwork larger than this is ignored rather than buffered.
const MAX_PICTURE: usize = 32 << 20;

/// A song as MPD describes it in `lsinfo`, `playlistinfo` and friends.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Song {
    pub uri: String,
    /// Queue id and position (queue listings only).
    pub id: Option<u32>,
    pub pos: Option<u32>,
    pub title: Option<String>,
    /// Multi-valued tags are joined with ", ".
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub track: Option<u32>,
    pub disc: Option<u32>,
    pub date: Option<String>,
    pub original_date: Option<String>,
    pub genre: Option<String>,
    pub duration: f64,
    pub format: Option<AudioFormat>,
    /// RFC 3339, UTC.
    pub last_modified: Option<String>,
    pub mb_album_id: Option<String>,
}

impl Song {
    fn apply(&mut self, key: &str, value: &str) {
        match key {
            "Id" => self.id = value.parse().ok(),
            "Pos" => self.pos = value.parse().ok(),
            "Title" => set_first(&mut self.title, value),
            "Artist" => append(&mut self.artist, value),
            "AlbumArtist" => append(&mut self.album_artist, value),
            "Album" => set_first(&mut self.album, value),
            "Track" => self.track = leading_number(value),
            "Disc" => self.disc = leading_number(value),
            "Date" => set_first(&mut self.date, value),
            "OriginalDate" => set_first(&mut self.original_date, value),
            "Genre" => set_first(&mut self.genre, value),
            "duration" => self.duration = value.parse().unwrap_or(self.duration),
            "Time" if self.duration == 0.0 => self.duration = value.parse().unwrap_or(0.0),
            "Format" => self.format = AudioFormat::parse_mpd(value),
            "Last-Modified" => self.last_modified = Some(value.to_string()),
            "MUSICBRAINZ_ALBUMID" => set_first(&mut self.mb_album_id, value),
            _ => {}
        }
    }

    /// The title tag, or the file name without extension.
    pub fn display_title(&self) -> String {
        if let Some(title) = &self.title {
            return title.clone();
        }
        let name = self.uri.rsplit('/').next().unwrap_or(&self.uri);
        name.rsplit_once('.').map_or(name, |(stem, _)| stem).to_string()
    }
}

fn set_first(slot: &mut Option<String>, value: &str) {
    if slot.is_none() && !value.is_empty() {
        *slot = Some(value.to_string());
    }
}

fn append(slot: &mut Option<String>, value: &str) {
    if value.is_empty() {
        return;
    }
    match slot {
        Some(existing) => {
            existing.push_str(", ");
            existing.push_str(value);
        }
        None => *slot = Some(value.to_string()),
    }
}

/// `3`, `3/12` and ` 03` all give 3.
fn leading_number(value: &str) -> Option<u32> {
    let digits: String = value.trim().chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    Directory(String),
    Song(Box<Song>),
    Playlist(String),
}

/// Splits a listing into entries; each starts at a `file`, `directory` or `playlist` key.
pub fn parse_entries(pairs: Vec<(String, String)>) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut song: Option<Box<Song>> = None;
    for (key, value) in pairs {
        match key.as_str() {
            "file" => {
                entries.extend(song.take().map(Entry::Song));
                song = Some(Box::new(Song { uri: value, ..Song::default() }));
            }
            "directory" => {
                entries.extend(song.take().map(Entry::Song));
                entries.push(Entry::Directory(value));
            }
            "playlist" => {
                entries.extend(song.take().map(Entry::Song));
                entries.push(Entry::Playlist(value));
            }
            _ => {
                if let Some(song) = song.as_mut() {
                    song.apply(&key, &value);
                }
            }
        }
    }
    entries.extend(song.map(Entry::Song));
    entries
}

pub fn parse_status(pairs: &[(String, String)]) -> PlayerState {
    let mut s = PlayerState { online: true, ..PlayerState::default() };
    for (key, value) in pairs {
        let value = value.as_str();
        match key.as_str() {
            "state" => {
                s.state = match value {
                    "play" => PlayState::Play,
                    "pause" => PlayState::Pause,
                    _ => PlayState::Stop,
                }
            }
            "songid" => s.song_id = value.parse().ok(),
            "song" => s.song_pos = value.parse().ok(),
            "elapsed" => s.elapsed = value.parse().unwrap_or(0.0),
            "duration" => s.duration = value.parse().unwrap_or(0.0),
            "bitrate" => s.bitrate = value.parse().ok().filter(|&kbps| kbps > 0),
            "audio" => s.audio = AudioFormat::parse_mpd(value),
            // -1 means "no mixer".
            "volume" => s.volume = value.parse::<i32>().ok().and_then(|v| u8::try_from(v).ok()),
            "repeat" => s.repeat = value == "1",
            "random" => s.random = value == "1",
            "single" => s.single = value != "0",
            "consume" => s.consume = value != "0",
            "playlist" => s.queue_version = value.parse().unwrap_or(0),
            "playlistlength" => s.queue_length = value.parse().unwrap_or(0),
            "error" => s.error = Some(value.to_string()),
            _ => {}
        }
    }
    s
}

/// Typed MPD commands. Cheap to clone; clones share one connection.
#[derive(Clone)]
pub struct MpdEngine {
    mpd: Mpd,
}

impl MpdEngine {
    pub fn new(addr: &str) -> Self {
        Self { mpd: Mpd::new(addr) }
    }

    /// A separate connection tuned for bulk work (library walks, artwork).
    pub fn bulk(addr: &str) -> Self {
        Self { mpd: Mpd::new(addr).with_binary_limit(1 << 20) }
    }

    pub fn watch(&self) -> mpsc::Receiver<Event> {
        client::watch(self.mpd.addr())
    }

    async fn cmd(&self, command: &str) -> Result<()> {
        self.mpd.run(command).await.map(drop)
    }

    pub async fn status(&self) -> Result<PlayerState> {
        Ok(parse_status(&self.mpd.run("status").await?.pairs))
    }

    pub async fn queue(&self) -> Result<Vec<QueueItem>> {
        let entries = parse_entries(self.mpd.run("playlistinfo").await?.pairs);
        Ok(entries
            .into_iter()
            .filter_map(|entry| match entry {
                Entry::Song(song) => Some(QueueItem {
                    id: song.id?,
                    title: song.display_title(),
                    artist: song.artist.or(song.album_artist).unwrap_or_default(),
                    album: song.album.unwrap_or_default(),
                    duration: song.duration,
                    format: song.format,
                    uri: song.uri,
                    album_id: None,
                    art: None,
                }),
                _ => None,
            })
            .collect())
    }

    pub async fn play(&self) -> Result<()> {
        self.cmd("play").await
    }

    pub async fn play_pos(&self, pos: u32) -> Result<()> {
        self.cmd(&format!("play {pos}")).await
    }

    pub async fn play_id(&self, id: u32) -> Result<()> {
        self.cmd(&format!("playid {id}")).await
    }

    pub async fn pause(&self, paused: bool) -> Result<()> {
        self.cmd(if paused { "pause 1" } else { "pause 0" }).await
    }

    pub async fn stop(&self) -> Result<()> {
        self.cmd("stop").await
    }

    pub async fn next(&self) -> Result<()> {
        self.cmd("next").await
    }

    pub async fn previous(&self) -> Result<()> {
        self.cmd("previous").await
    }

    pub async fn seek(&self, seconds: f64) -> Result<()> {
        self.cmd(&format!("seekcur {:.3}", seconds.max(0.0))).await
    }

    /// Seeks within a queue item; starts playback if stopped.
    pub async fn seek_id(&self, id: u32, seconds: f64) -> Result<()> {
        self.cmd(&format!("seekid {id} {:.3}", seconds.max(0.0))).await
    }

    pub async fn set_modes(&self, modes: &ModesRequest) -> Result<()> {
        let flag = |b: bool| if b { 1 } else { 0 };
        let mut commands = Vec::new();
        for (name, value) in
            [("repeat", modes.repeat), ("random", modes.random), ("single", modes.single), ("consume", modes.consume)]
        {
            if let Some(value) = value {
                commands.push(format!("{name} {}", flag(value)));
            }
        }
        self.mpd.run_list(&commands).await.map(drop)
    }

    /// Adds `uris` in one command list and returns the queue position of the first.
    pub async fn add(&self, uris: &[String], mode: AddMode) -> Result<u32> {
        let status = self.status().await?;
        let mut commands = Vec::with_capacity(uris.len() + 1);
        let first = match (mode, status.song_pos) {
            (AddMode::Replace, _) => {
                commands.push("clear".to_string());
                commands.extend(uris.iter().map(|uri| format!("add {}", quote(uri))));
                0
            }
            // "+N" is relative to the current song (MPD 0.23+).
            (AddMode::Next, Some(current)) => {
                commands.extend(uris.iter().enumerate().map(|(i, uri)| format!("add {} +{i}", quote(uri))));
                current + 1
            }
            (AddMode::Append | AddMode::Next, _) => {
                commands.extend(uris.iter().map(|uri| format!("add {}", quote(uri))));
                status.queue_length
            }
        };
        self.mpd.run_list(&commands).await?;
        Ok(first)
    }

    pub async fn delete_id(&self, id: u32) -> Result<()> {
        self.cmd(&format!("deleteid {id}")).await
    }

    pub async fn move_id(&self, id: u32, to: u32) -> Result<()> {
        self.cmd(&format!("moveid {id} {to}")).await
    }

    pub async fn clear(&self) -> Result<()> {
        self.cmd("clear").await
    }

    /// Clears the queue except for the current song, which keeps playing.
    pub async fn clear_except_current(&self) -> Result<()> {
        let status = self.status().await?;
        let Some(pos) = status.song_pos.filter(|_| status.state != PlayState::Stop) else {
            return self.clear().await;
        };
        let mut commands = Vec::new();
        // Delete after the current song first so its position stays valid.
        if pos + 1 < status.queue_length {
            commands.push(format!("delete {}:{}", pos + 1, status.queue_length));
        }
        if pos > 0 {
            commands.push(format!("delete 0:{pos}"));
        }
        self.mpd.run_list(&commands).await.map(drop)
    }

    /// Asks MPD to rescan the music directory; `database` fires when done.
    pub async fn update(&self) -> Result<()> {
        self.cmd("update").await
    }

    pub async fn lsinfo(&self, dir: &str) -> Result<Vec<Entry>> {
        Ok(parse_entries(self.mpd.run(&format!("lsinfo {}", quote(dir))).await?.pairs))
    }

    /// Cover art for a song: the folder image (`albumart`), else the embedded picture.
    pub async fn picture(&self, uri: &str) -> Result<Option<Vec<u8>>> {
        for command in ["albumart", "readpicture"] {
            match self.read_binary(command, uri).await {
                Ok(Some(data)) => return Ok(Some(data)),
                Ok(None) | Err(Error::Ack { .. }) => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(None)
    }

    async fn read_binary(&self, command: &str, uri: &str) -> Result<Option<Vec<u8>>> {
        let mut data = Vec::new();
        loop {
            let response = self.mpd.run(&format!("{command} {} {}", quote(uri), data.len())).await?;
            let size: usize = response.get("size").and_then(|s| s.parse().ok()).unwrap_or(0);
            if size > MAX_PICTURE {
                return Ok(None);
            }
            match response.binary {
                Some(chunk) if !chunk.is_empty() => data.extend_from_slice(&chunk),
                _ => break,
            }
            if data.len() >= size {
                break;
            }
        }
        Ok((!data.is_empty()).then_some(data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hifi_core::SampleFormat;

    fn pairs(text: &str) -> Vec<(String, String)> {
        text.lines()
            .map(|l| {
                let (k, v) = l.split_once(": ").unwrap();
                (k.to_string(), v.to_string())
            })
            .collect()
    }

    /// A one-connection fake MPD that answers `status` with `status` and
    /// records every other line until the command list ends.
    async fn fake_mpd(status: &'static str) -> (String, tokio::task::JoinHandle<Vec<String>>) {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let (read, mut write) = socket.into_split();
            let mut lines = BufReader::new(read).lines();
            write.write_all(b"OK MPD 0.24.0\n").await.unwrap();
            let mut seen = Vec::new();
            while let Some(line) = lines.next_line().await.unwrap() {
                match line.as_str() {
                    "status" => write.write_all(format!("{status}\nOK\n").as_bytes()).await.unwrap(),
                    "command_list_end" | "clear" => {
                        seen.push(line);
                        write.write_all(b"OK\n").await.unwrap();
                        break;
                    }
                    _ => seen.push(line),
                }
            }
            seen
        });
        (addr, server)
    }

    #[tokio::test]
    async fn clearing_keeps_the_playing_song() {
        let (addr, server) = fake_mpd("state: play\nsong: 2\nplaylistlength: 6").await;
        MpdEngine::new(&addr).clear_except_current().await.unwrap();
        assert_eq!(server.await.unwrap(), ["command_list_begin", "delete 3:6", "delete 0:2", "command_list_end"]);

        let (addr, server) = fake_mpd("state: pause\nsong: 0\nplaylistlength: 3").await;
        MpdEngine::new(&addr).clear_except_current().await.unwrap();
        assert_eq!(server.await.unwrap(), ["command_list_begin", "delete 1:3", "command_list_end"]);

        let (addr, server) = fake_mpd("state: stop\nsong: 1\nplaylistlength: 3").await;
        MpdEngine::new(&addr).clear_except_current().await.unwrap();
        assert_eq!(server.await.unwrap(), ["clear"]);
    }

    #[test]
    fn parses_status() {
        let s = parse_status(&pairs(
            "volume: -1\nrepeat: 0\nrandom: 1\nsingle: oneshot\nconsume: 0\nplaylist: 42\n\
             playlistlength: 12\nstate: pause\nsong: 3\nsongid: 17\nelapsed: 61.250\n\
             bitrate: 2304\nduration: 240.007\naudio: 96000:24:2",
        ));
        assert!(s.online);
        assert_eq!(s.state, PlayState::Pause);
        assert_eq!((s.song_pos, s.song_id), (Some(3), Some(17)));
        assert_eq!(s.volume, None);
        assert!(s.random && s.single && !s.repeat);
        assert_eq!((s.queue_version, s.queue_length), (42, 12));
        assert_eq!(s.audio.unwrap().format, SampleFormat::S24);
        assert!((s.elapsed - 61.25).abs() < 1e-9);
    }

    #[test]
    fn splits_listings() {
        let entries = parse_entries(pairs(
            "directory: Artist/Album\nLast-Modified: 2024-01-01T00:00:00Z\n\
             file: Artist/Album/01 Intro.flac\nLast-Modified: 2024-01-02T00:00:00Z\n\
             Format: 44100:16:2\nArtist: A\nArtist: B\nAlbum: Album\nTrack: 1/10\nTime: 61\nduration: 61.5\n\
             file: Artist/Album/02.flac\nTitle: Second\nDisc: 2/2\n\
             playlist: favourites\nLast-Modified: 2024-01-03T00:00:00Z",
        ));
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0], Entry::Directory("Artist/Album".into()));
        let Entry::Song(first) = &entries[1] else { panic!() };
        assert_eq!(first.artist.as_deref(), Some("A, B"));
        assert_eq!(first.track, Some(1));
        assert_eq!(first.duration, 61.5);
        assert_eq!(first.display_title(), "01 Intro");
        assert_eq!(first.last_modified.as_deref(), Some("2024-01-02T00:00:00Z"));
        let Entry::Song(second) = &entries[2] else { panic!() };
        assert_eq!((second.display_title().as_str(), second.disc), ("Second", Some(2)));
        assert_eq!(entries[3], Entry::Playlist("favourites".into()));
    }
}
