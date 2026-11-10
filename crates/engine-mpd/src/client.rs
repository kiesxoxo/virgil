use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};
use tracing::{debug, info, warn};

use crate::proto::{Connection, Error, Response, Result};

/// How long MPD gets to accept a connection: every command waits behind one being opened, so a
/// host that doesn't answer mustn't hold them for the minutes TCP would.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// A shared, lazily (re)connecting command connection.
///
/// MPD drops idle clients after `connection_timeout`, so a failed command is
/// retried once on a fresh connection.
#[derive(Clone)]
pub struct Mpd {
    addr: Arc<str>,
    conn: Arc<Mutex<Option<Connection>>>,
    binary_limit: Option<usize>,
}

#[derive(Clone, Copy)]
enum Request<'a> {
    One(&'a str),
    List(&'a [String]),
}

impl Mpd {
    pub fn new(addr: &str) -> Self {
        Self { addr: addr.into(), conn: Arc::default(), binary_limit: None }
    }

    /// Raises MPD's per-response binary chunk size (default 8 KiB) so artwork
    /// arrives in a few round trips instead of hundreds.
    pub fn with_binary_limit(mut self, bytes: usize) -> Self {
        self.binary_limit = Some(bytes);
        self
    }

    pub fn addr(&self) -> &str {
        &self.addr
    }

    pub async fn run(&self, command: &str) -> Result<Response> {
        self.exec(Request::One(command)).await
    }

    pub async fn run_list(&self, commands: &[String]) -> Result<Response> {
        if commands.is_empty() {
            return Ok(Response::default());
        }
        self.exec(Request::List(commands)).await
    }

    async fn exec(&self, request: Request<'_>) -> Result<Response> {
        let mut slot = self.conn.lock().await;
        let mut retried = false;
        loop {
            // Out of the slot while a command runs on it: if this future is dropped half way (the
            // HTTP client that asked hung up), the connection goes with it, rather than answer the
            // next command with the rest of this one's response.
            let mut conn = match slot.take() {
                Some(conn) => conn,
                None => self.open().await?,
            };
            let result = match request {
                Request::One(command) => conn.run(command).await,
                Request::List(commands) => conn.run_list(commands).await,
            };
            match result {
                Err(e) if e.is_connection() => {
                    if retried {
                        return Err(e);
                    }
                    debug!("mpd connection lost ({e}), reconnecting");
                    retried = true;
                }
                other => {
                    *slot = Some(conn);
                    return other;
                }
            }
        }
    }

    async fn open(&self) -> Result<Connection> {
        let connecting = tokio::time::timeout(CONNECT_TIMEOUT, Connection::connect(&self.addr));
        let mut conn = connecting.await.map_err(|_| std::io::Error::from(std::io::ErrorKind::TimedOut))??;
        if let Some(limit) = self.binary_limit {
            // Needs MPD 0.22.4+; older servers just keep the 8 KiB default.
            if let Err(e @ Error::Ack { .. }) = conn.run(&format!("binarylimit {limit}")).await {
                debug!("binarylimit unsupported: {e}");
            }
        }
        Ok(conn)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// (Re)connected: everything may have changed.
    Connected {
        version: String,
    },
    Disconnected,
    /// Subsystems from `idle`, e.g. `player`, `playlist`, `database`.
    Changed(Vec<String>),
}

/// Watches MPD over a dedicated `idle` connection, reconnecting forever.
/// The task ends when the receiver is dropped.
pub fn watch(addr: &str) -> mpsc::Receiver<Event> {
    let addr = addr.to_string();
    let (tx, rx) = mpsc::channel(64);
    tokio::spawn(async move {
        let mut backoff = Duration::from_millis(500);
        loop {
            match Connection::connect(&addr).await {
                Ok(mut conn) => {
                    info!("connected to MPD {} at {addr}", conn.version);
                    backoff = Duration::from_millis(500);
                    if tx.send(Event::Connected { version: conn.version.clone() }).await.is_err() {
                        return;
                    }
                    loop {
                        match conn.run("idle database playlist player mixer options output").await {
                            Ok(response) => {
                                let changed = response
                                    .pairs
                                    .into_iter()
                                    .filter(|(k, _)| k == "changed")
                                    .map(|(_, v)| v)
                                    .collect();
                                if tx.send(Event::Changed(changed)).await.is_err() {
                                    return;
                                }
                            }
                            Err(e) => {
                                warn!("mpd idle connection: {e}");
                                break;
                            }
                        }
                    }
                    if tx.send(Event::Disconnected).await.is_err() {
                        return;
                    }
                }
                Err(e) => debug!("mpd at {addr} unreachable: {e}"),
            }
            if tx.is_closed() {
                return;
            }
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(Duration::from_secs(10));
        }
    });
    rx
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;

    use super::*;

    /// A command given up on half way (its caller went away) must not leave the rest of its
    /// response to be read as the next command's.
    #[tokio::test]
    async fn an_abandoned_command_takes_its_connection_with_it() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let server = tokio::spawn(async move {
            // The first connection answers half of `playlistinfo` and stalls.
            let (first, _) = listener.accept().await.unwrap();
            let (read, mut write) = first.into_split();
            let mut lines = BufReader::new(read).lines();
            write.write_all(b"OK MPD 0.24.0\n").await.unwrap();
            assert_eq!(lines.next_line().await.unwrap().as_deref(), Some("playlistinfo"));
            write.write_all(b"file: a.flac\n").await.unwrap();

            // The next command comes on a connection of its own.
            let (second, _) = listener.accept().await.unwrap();
            let (read, mut write) = second.into_split();
            let mut lines = BufReader::new(read).lines();
            write.write_all(b"OK MPD 0.24.0\n").await.unwrap();
            assert_eq!(lines.next_line().await.unwrap().as_deref(), Some("status"));
            write.write_all(b"state: play\nOK\n").await.unwrap();
            // Keep both open until the client is done.
            let _ = lines.next_line().await;
        });

        let mpd = Mpd::new(&addr);
        let abandoned = tokio::time::timeout(Duration::from_millis(200), mpd.run("playlistinfo")).await;
        assert!(abandoned.is_err(), "the command was still waiting for its response");
        let status = mpd.run("status").await.unwrap();
        assert_eq!(status.pairs, [("state".to_string(), "play".to_string())]);
        drop(mpd);
        server.await.unwrap();
    }
}
