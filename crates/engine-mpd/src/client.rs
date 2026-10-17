use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};
use tracing::{debug, info, warn};

use crate::proto::{Connection, Error, Response, Result};

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
            if slot.is_none() {
                *slot = Some(self.open().await?);
            }
            let conn = slot.as_mut().expect("connected above");
            let result = match request {
                Request::One(command) => conn.run(command).await,
                Request::List(commands) => conn.run_list(commands).await,
            };
            match result {
                Err(e) if e.is_connection() => {
                    *slot = None;
                    if retried {
                        return Err(e);
                    }
                    debug!("mpd connection lost ({e}), reconnecting");
                    retried = true;
                }
                other => return other,
            }
        }
    }

    async fn open(&self) -> Result<Connection> {
        let mut conn = Connection::connect(&self.addr).await?;
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
