//! HAP's HTTP: plain until pair-verify, then inside the encrypted session.
//! Hand-written rather than hyper because the accessory also pushes
//! unsolicited `EVENT/1.0` messages down the same connection.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Semaphore, oneshot};
use tracing::{debug, info, warn};

use crate::accessory::{self, ON, status};
use crate::pairing::{self, Setup, Verify};
use crate::session::Session;
use crate::{Shared, Write};

/// Home hubs, phones and Macs each keep a connection open; HAP asks for at least 8.
const MAX_CONNECTIONS: usize = 32;
const MAX_HEAD: usize = 8 * 1024;
const MAX_BODY: usize = 64 * 1024;
/// How long a controller waits for Virgil to act on a write.
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

const JSON: &str = "application/hap+json";
const TLV: &str = "application/pairing+tlv8";

pub async fn run(shared: Arc<Shared>, listener: TcpListener) {
    let slots = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    let ids = AtomicU64::new(0);
    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(accepted) => accepted,
            Err(e) => {
                warn!("HomeKit: accept: {e}");
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        };
        let Ok(slot) = slots.clone().try_acquire_owned() else {
            debug!("HomeKit: too many connections, dropping {peer}");
            continue;
        };
        // Notice controllers that vanished (a hub unplugged) rather than keep their slot.
        let keepalive =
            socket2::TcpKeepalive::new().with_time(Duration::from_secs(60)).with_interval(Duration::from_secs(10));
        let _ = socket2::SockRef::from(&stream).set_tcp_keepalive(&keepalive);
        let _ = stream.set_nodelay(true);
        let id = ids.fetch_add(1, Ordering::Relaxed);
        let shared = shared.clone();
        tokio::spawn(async move {
            debug!("HomeKit: {peer} connected");
            let mut conn = Connection::new(shared, id);
            conn.serve(stream).await;
            pairing::release(&conn.shared, id);
            debug!("HomeKit: {peer} disconnected");
            drop(slot);
        });
    }
}

pub(crate) struct Request {
    pub method: String,
    pub path: String,
    query: String,
    pub body: Vec<u8>,
}

struct Response {
    status: u16,
    content_type: &'static str,
    body: Vec<u8>,
}

impl Response {
    fn json(status: u16, body: &Value) -> Self {
        Self { status, content_type: JSON, body: body.to_string().into_bytes() }
    }

    fn tlv(body: Vec<u8>) -> Self {
        Self { status: 200, content_type: TLV, body }
    }

    fn empty(status: u16) -> Self {
        Self { status, content_type: JSON, body: Vec::new() }
    }

    fn to_bytes(&self, protocol: &str) -> Vec<u8> {
        let reason = match self.status {
            200 => "OK",
            204 => "No Content",
            207 => "Multi-Status",
            400 => "Bad Request",
            404 => "Not Found",
            470 => "Connection Authorization Required",
            _ => "Error",
        };
        let mut head = format!("{protocol} {} {reason}\r\n", self.status);
        if !self.body.is_empty() {
            head += &format!("Content-Type: {}\r\n", self.content_type);
        }
        head += &format!("Content-Length: {}\r\n\r\n", self.body.len());
        [head.into_bytes(), self.body.clone()].concat()
    }
}

/// A request from the front of `buf`, and how many bytes it took. `Err` if
/// the bytes can't be HTTP (or are too big): the connection is then closed.
pub(crate) fn parse(buf: &[u8]) -> Result<Option<(Request, usize)>, ()> {
    let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") else {
        return if buf.len() > MAX_HEAD { Err(()) } else { Ok(None) };
    };
    let head = std::str::from_utf8(&buf[..end]).map_err(|_| ())?;
    let mut lines = head.split("\r\n");
    let mut first = lines.next().ok_or(())?.split(' ');
    let (Some(method), Some(target)) = (first.next(), first.next()) else { return Err(()) };
    let mut length = 0;
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                length = value.trim().parse().map_err(|_| ())?;
            }
        }
    }
    if length > MAX_BODY {
        return Err(());
    }
    let start = end + 4;
    let Some(body) = buf.get(start..start + length) else { return Ok(None) };
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let request =
        Request { method: method.to_string(), path: path.to_string(), query: query.to_string(), body: body.to_vec() };
    Ok(Some((request, start + length)))
}

struct Connection {
    shared: Arc<Shared>,
    id: u64,
    setup: Option<Setup>,
    verify: Option<Verify>,
    session: Option<Session>,
    /// The controller this connection verified as.
    controller: Option<String>,
    /// Asked for events on the switch.
    subscribed: bool,
    /// The switch's value as this controller last heard it (from a read,
    /// an event or its own write), so it isn't told what it already knows.
    known: Option<bool>,
}

impl Connection {
    fn new(shared: Arc<Shared>, id: u64) -> Self {
        Self { shared, id, setup: None, verify: None, session: None, controller: None, subscribed: false, known: None }
    }

    async fn serve(&mut self, mut stream: TcpStream) {
        let mut on = self.shared.on.subscribe();
        let mut pairings = self.shared.pairings.subscribe();
        let (mut raw, mut plain) = (Vec::with_capacity(4096), Vec::new());
        loop {
            tokio::select! {
                read = stream.read_buf(&mut raw) => {
                    if !matches!(read, Ok(n) if n > 0) {
                        return;
                    }
                    match &mut self.session {
                        Some(session) => {
                            if session.decrypt(&mut raw, &mut plain).is_err() {
                                return debug!("HomeKit: undecryptable frame, closing");
                            }
                        }
                        None => plain.append(&mut raw),
                    }
                    loop {
                        let (request, used) = match parse(&plain) {
                            Ok(Some(parsed)) => parsed,
                            Ok(None) => break,
                            Err(()) => return debug!("HomeKit: malformed request, closing"),
                        };
                        plain.drain(..used);
                        let (response, upgrade) = self.handle(request).await;
                        if self.send(&mut stream, &response.to_bytes("HTTP/1.1")).await.is_err() {
                            return;
                        }
                        if let Some((session, controller)) = upgrade {
                            debug!("HomeKit: verified {controller}");
                            self.session = Some(session);
                            self.controller = Some(controller);
                        }
                    }
                }
                Ok(()) = on.changed(), if self.subscribed => {
                    let value = *on.borrow_and_update();
                    if self.known != Some(value) {
                        self.known = Some(value);
                        let body = json!({ "characteristics": [{ "aid": 1, "iid": ON, "value": value }] });
                        if self.send(&mut stream, &Response::json(200, &body).to_bytes("EVENT/1.0")).await.is_err() {
                            return;
                        }
                    }
                }
                Ok(()) = pairings.changed(), if self.controller.is_some() => {
                    pairings.borrow_and_update();
                    let controller = self.controller.as_deref().unwrap_or_default();
                    if self.shared.store().pairing(controller).is_none() {
                        return debug!("HomeKit: {controller} was removed, closing");
                    }
                }
            }
        }
    }

    async fn send(&mut self, stream: &mut TcpStream, bytes: &[u8]) -> std::io::Result<()> {
        match &mut self.session {
            Some(session) => stream.write_all(&session.encrypt(bytes)).await,
            None => stream.write_all(bytes).await,
        }
    }

    async fn handle(&mut self, request: Request) -> (Response, Option<(Session, String)>) {
        let shared = self.shared.clone();
        match (request.method.as_str(), request.path.as_str()) {
            ("POST", "/pair-setup") => {
                (Response::tlv(pairing::setup(&shared, self.id, &mut self.setup, &request.body).await), None)
            }
            ("POST", "/pair-verify") => {
                let (body, upgrade) = pairing::verify(&shared, &mut self.verify, &request.body);
                (Response::tlv(body), upgrade)
            }
            ("POST", "/identify") if !shared.store().paired() => {
                info!("HomeKit: identify");
                (Response::empty(204), None)
            }
            ("POST", "/identify") => (Response::json(400, &json!({ "status": -70401 })), None),
            _ if self.controller.is_none() => (Response::json(470, &json!({ "status": -70401 })), None),
            ("GET", "/accessories") => {
                self.known = Some(*shared.on.borrow());
                (Response::json(200, &accessory::database(&shared)), None)
            }
            ("GET", "/characteristics") => (self.read(&request.query), None),
            ("PUT", "/characteristics") => (self.write(&request.body).await, None),
            ("POST", "/pairings") => {
                let controller = self.controller.clone().unwrap_or_default();
                (Response::tlv(pairing::manage(&shared, &controller, &request.body)), None)
            }
            _ => (Response::empty(404), None),
        }
    }

    /// `GET /characteristics?id=1.11,1.3`.
    fn read(&mut self, query: &str) -> Response {
        let ids = query.split('&').find_map(|p| p.strip_prefix("id=")).unwrap_or_default();
        let mut results = Vec::new();
        let mut failed = false;
        for id in ids.split(',').filter(|id| !id.is_empty()) {
            let (aid, iid) = id.split_once('.').unwrap_or((id, ""));
            let (aid, iid) = (aid.parse::<u64>().unwrap_or(0), iid.parse::<u64>().unwrap_or(0));
            let value = if aid == 1 { accessory::read(&self.shared, iid) } else { Err(status::NOT_FOUND) };
            match value {
                Ok(value) => {
                    if iid == ON {
                        self.known = value.as_bool();
                    }
                    results.push(json!({ "aid": aid, "iid": iid, "value": value }));
                }
                Err(status) => {
                    failed = true;
                    results.push(json!({ "aid": aid, "iid": iid, "status": status }));
                }
            }
        }
        if failed {
            // A multi-status response gives every item a status.
            for result in &mut results {
                if result.get("status").is_none() {
                    result["status"] = json!(0);
                }
            }
        }
        Response::json(if failed { 207 } else { 200 }, &json!({ "characteristics": results }))
    }

    /// `PUT /characteristics`: subscriptions and writes.
    async fn write(&mut self, body: &[u8]) -> Response {
        let Ok(request) = serde_json::from_slice::<Value>(body) else { return Response::empty(400) };
        let Some(items) = request["characteristics"].as_array() else { return Response::empty(400) };
        let mut results = Vec::new();
        for item in items {
            let (aid, iid) = (item["aid"].as_u64().unwrap_or(0), item["iid"].as_u64().unwrap_or(0));
            let status = if aid != 1 || !accessory::exists(&self.shared, iid) {
                status::NOT_FOUND
            } else if let Some(ev) = item.get("ev").and_then(as_bool) {
                if iid == ON {
                    self.subscribed = ev;
                    0
                } else {
                    status::NO_NOTIFICATION
                }
            } else {
                match (iid, item.get("value")) {
                    (ON, Some(value)) => match as_bool(value) {
                        Some(on) => self.switch(on).await,
                        None => status::INVALID_VALUE,
                    },
                    (accessory::IDENTIFY, Some(_)) => {
                        info!("HomeKit: identify");
                        0
                    }
                    (_, Some(_)) => status::READ_ONLY,
                    (_, None) => 0,
                }
            };
            results.push(json!({ "aid": aid, "iid": iid, "status": status }));
        }
        if results.iter().all(|r| r["status"] == 0) {
            Response::empty(204)
        } else {
            Response::json(207, &json!({ "characteristics": results }))
        }
    }

    /// Asks Virgil to turn playback on or off, and waits for it to answer.
    async fn switch(&mut self, on: bool) -> i64 {
        let (done, result) = oneshot::channel();
        if self.shared.writes.send(Write { on, done }).await.is_err() {
            return status::COMMUNICATION_FAILURE;
        }
        match tokio::time::timeout(WRITE_TIMEOUT, result).await {
            Ok(Ok(true)) => {
                // Shown at once; Virgil corrects it if playback doesn't follow.
                self.known = Some(on);
                self.shared.on.send_if_modified(|value| std::mem::replace(value, on) != on);
                0
            }
            _ => status::COMMUNICATION_FAILURE,
        }
    }
}

/// HAP sends booleans as `true`/`false` or `1`/`0`.
fn as_bool(value: &Value) -> Option<bool> {
    value.as_bool().or_else(|| value.as_u64().filter(|n| *n <= 1).map(|n| n == 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pipelined_requests() {
        let wire = b"PUT /characteristics HTTP/1.1\r\nHost: Virgil._hap._tcp.local\r\ncontent-length: 2\r\n\r\n{}GET /accessories HTTP/1.1\r\n\r\n";
        let (first, used) = parse(wire).unwrap().unwrap();
        assert_eq!(
            (first.method.as_str(), first.path.as_str(), first.body.as_slice()),
            ("PUT", "/characteristics", &b"{}"[..])
        );
        let (second, rest) = parse(&wire[used..]).unwrap().unwrap();
        assert_eq!(second.path, "/accessories");
        assert_eq!(used + rest, wire.len());
    }

    #[test]
    fn waits_for_the_whole_body() {
        assert!(parse(b"POST /pair-setup HTTP/1.1\r\nContent-Length: 6\r\n\r\n\x06\x01").unwrap().is_none());
        assert!(parse(b"GET /characteristics?id=1.11").unwrap().is_none());
        let (request, _) = parse(b"GET /characteristics?id=1.11,1.3 HTTP/1.1\r\n\r\n").unwrap().unwrap();
        assert_eq!(request.query, "id=1.11,1.3");
        assert!(parse(&[b'x'; MAX_HEAD + 1]).is_err());
    }

    #[test]
    fn reads_hap_booleans() {
        assert_eq!(as_bool(&json!(true)), Some(true));
        assert_eq!(as_bool(&json!(0)), Some(false));
        assert_eq!(as_bool(&json!(2)), None);
    }
}
