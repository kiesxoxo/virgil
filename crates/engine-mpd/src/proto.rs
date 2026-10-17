//! The MPD text protocol: one command per line, `key: value` lines back,
//! terminated by `OK` or `ACK [code@index] {command} message`.
//! <https://mpd.readthedocs.io/en/latest/protocol.html>

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("mpd i/o: {0}")]
    Io(#[from] std::io::Error),
    #[error("mpd: {message} ({command}, code {code})")]
    Ack { code: u32, command: String, message: String },
    #[error("mpd protocol: {0}")]
    Protocol(String),
}

impl Error {
    /// The connection is unusable and must be reopened.
    pub fn is_connection(&self) -> bool {
        !matches!(self, Error::Ack { .. })
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Default)]
pub struct Response {
    pub pairs: Vec<(String, String)>,
    pub binary: Option<Vec<u8>>,
}

impl Response {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
}

/// Quotes a command argument.
pub fn quote(arg: &str) -> String {
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('"');
    for c in arg.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

pub(crate) trait Stream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Stream for T {}

pub struct Connection {
    io: BufReader<Box<dyn Stream>>,
    pub version: String,
}

impl Connection {
    /// `addr` is `host:port` or an absolute path to MPD's Unix socket. Only the
    /// socket lets clients queue arbitrary `file:///` URIs.
    pub async fn connect(addr: &str) -> Result<Self> {
        let stream: Box<dyn Stream> = if addr.starts_with('/') {
            connect_unix(addr).await?
        } else {
            let tcp = tokio::net::TcpStream::connect(addr).await?;
            tcp.set_nodelay(true)?;
            Box::new(tcp)
        };
        Self::handshake(stream).await
    }

    pub(crate) async fn handshake(stream: Box<dyn Stream>) -> Result<Self> {
        let mut io = BufReader::new(stream);
        let mut line = String::new();
        io.read_line(&mut line).await?;
        let version = line
            .trim_end()
            .strip_prefix("OK MPD ")
            .ok_or_else(|| Error::Protocol(format!("unexpected greeting {line:?}")))?
            .to_string();
        Ok(Self { io, version })
    }

    pub async fn run(&mut self, command: &str) -> Result<Response> {
        let mut buf = String::with_capacity(command.len() + 1);
        buf.push_str(command);
        buf.push('\n');
        self.io.get_mut().write_all(buf.as_bytes()).await?;
        self.read_response().await
    }

    /// Runs commands as one `command_list`: one round trip, applied atomically.
    pub async fn run_list(&mut self, commands: &[String]) -> Result<Response> {
        let mut buf = String::from("command_list_begin\n");
        for command in commands {
            buf.push_str(command);
            buf.push('\n');
        }
        buf.push_str("command_list_end\n");
        self.io.get_mut().write_all(buf.as_bytes()).await?;
        self.read_response().await
    }

    async fn read_response(&mut self) -> Result<Response> {
        let mut response = Response::default();
        let mut line = Vec::with_capacity(128);
        loop {
            line.clear();
            if self.io.read_until(b'\n', &mut line).await? == 0 {
                return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof).into());
            }
            let text = String::from_utf8_lossy(&line);
            let text = text.trim_end_matches('\n');
            if text == "OK" {
                return Ok(response);
            }
            if let Some(rest) = text.strip_prefix("ACK ") {
                return Err(parse_ack(rest));
            }
            let (key, value) =
                text.split_once(": ").ok_or_else(|| Error::Protocol(format!("malformed line {text:?}")))?;
            if key == "binary" {
                let len: usize = value.parse().map_err(|_| Error::Protocol(format!("bad binary length {value:?}")))?;
                let mut data = vec![0; len];
                self.io.read_exact(&mut data).await?;
                let mut newline = [0u8; 1];
                self.io.read_exact(&mut newline).await?;
                response.binary = Some(data);
            } else {
                response.pairs.push((key.to_string(), value.to_string()));
            }
        }
    }
}

#[cfg(unix)]
async fn connect_unix(path: &str) -> Result<Box<dyn Stream>> {
    Ok(Box::new(tokio::net::UnixStream::connect(path).await?))
}

#[cfg(not(unix))]
async fn connect_unix(_path: &str) -> Result<Box<dyn Stream>> {
    Err(Error::Protocol("unix sockets need a unix host; use host:port".into()))
}

/// Parses the part after `ACK `: `[50@0] {albumart} No file exists`.
fn parse_ack(rest: &str) -> Error {
    let code =
        rest.strip_prefix('[').and_then(|r| r.split_once('@')).and_then(|(code, _)| code.parse().ok()).unwrap_or(0);
    let (command, message) = match (rest.find('{'), rest.find('}')) {
        (Some(open), Some(close)) if open < close => {
            (rest[open + 1..close].to_string(), rest[close + 1..].trim().to_string())
        }
        _ => (String::new(), rest.to_string()),
    };
    Error::Ack { code, command, message }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn fake_server(script: &'static [u8]) -> Connection {
        let (client, mut server) = tokio::io::duplex(64 * 1024);
        tokio::spawn(async move {
            server.write_all(b"OK MPD 0.24.0\n").await.unwrap();
            server.write_all(script).await.unwrap();
            // Keep the pipe open until the client is done.
            let mut sink = Vec::new();
            let _ = server.read_to_end(&mut sink).await;
        });
        Connection::handshake(Box::new(client)).await.unwrap()
    }

    #[tokio::test]
    async fn reads_pairs_binary_and_ack() {
        let mut conn = fake_server(
            b"volume: -1\nstate: play\nOK\n\
              size: 5\ntype: image/jpeg\nbinary: 5\n\x00\x01\n\x02\x03\nOK\n\
              ACK [50@0] {albumart} No file exists\n",
        )
        .await;
        assert_eq!(conn.version, "0.24.0");

        let status = conn.run("status").await.unwrap();
        assert_eq!(status.get("state"), Some("play"));
        assert_eq!(status.get("volume"), Some("-1"));

        // Binary payloads may contain newlines; they are read by length.
        let art = conn.run("albumart x 0").await.unwrap();
        assert_eq!(art.get("size"), Some("5"));
        assert_eq!(art.binary.as_deref(), Some(&b"\x00\x01\n\x02\x03"[..]));

        match conn.run("albumart y 0").await {
            Err(Error::Ack { code, command, message }) => {
                assert_eq!((code, command.as_str(), message.as_str()), (50, "albumart", "No file exists"));
            }
            other => panic!("expected ACK, got {other:?}"),
        }
    }

    #[test]
    fn quotes_arguments() {
        assert_eq!(quote(r#"AC/DC "Live"\x.flac"#), r#""AC/DC \"Live\"\\x.flac""#);
    }
}
