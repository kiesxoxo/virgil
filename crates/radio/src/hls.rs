//! NTS serves its AAC streams as HLS: a playlist of 12-second ADTS chunks,
//! renewed as the stream goes on. MPD can't play HLS (FFmpeg 8 dropped the
//! protocol MPD used for it), so this joins the chunks into one continuous
//! ADTS stream, which MPD plays like any other internet radio.

use std::time::Duration;

use anyhow::{Context, bail};
use reqwest::Url;
use tokio::sync::mpsc;
use tracing::{debug, warn};

/// Chunks behind the live edge to start at, as players do: enough to ride out a slow download.
const START_BEHIND: usize = 3;
/// Chunks fetched ahead of what MPD has read.
const AHEAD: usize = 2;
/// Failed fetches in a row (of the playlist, or of a chunk) before the stream is given up.
const RETRIES: u32 = 3;
/// Master playlists followed to get to the chunks: one, in practice.
const MAX_HOPS: u32 = 4;

/// A media playlist: the chunks it lists and how often it is renewed.
#[derive(Debug, PartialEq)]
struct Media {
    /// Sequence number of the first chunk listed.
    sequence: u64,
    target: Duration,
    chunks: Vec<Url>,
    /// `#EXT-X-ENDLIST`: nothing more will be added.
    ended: bool,
}

#[derive(Debug, PartialEq)]
enum Playlist {
    /// A master playlist's first variant.
    Master(Url),
    Media(Media),
}

fn parse(base: &Url, text: &str) -> anyhow::Result<Playlist> {
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    if lines.next() != Some("#EXTM3U") {
        bail!("not an HLS playlist");
    }
    let (mut sequence, mut target, mut chunks, mut ended, mut variant) = (0, 10.0, Vec::new(), false, false);
    for line in lines {
        if let Some(n) = line.strip_prefix("#EXT-X-MEDIA-SEQUENCE:") {
            sequence = n.parse().context("bad media sequence")?;
        } else if let Some(n) = line.strip_prefix("#EXT-X-TARGETDURATION:") {
            target = n.parse().context("bad target duration")?;
        } else if line == "#EXT-X-ENDLIST" {
            ended = true;
        } else if line.starts_with("#EXT-X-STREAM-INF:") {
            variant = true;
        } else if !line.starts_with('#') {
            let url = base.join(line).with_context(|| format!("bad URI {line:?}"))?;
            if variant {
                return Ok(Playlist::Master(url));
            }
            chunks.push(url);
        }
    }
    // A negative or absurd duration is a playlist gone wrong, not a reason to panic.
    let target = Duration::try_from_secs_f64(target).context("bad target duration")?;
    Ok(Playlist::Media(Media { sequence, target, chunks, ended }))
}

/// Index in `media.chunks` of the next chunk to send: the one after `next`
/// (a sequence number) if it's still listed, else a few behind the live edge.
fn start_index(media: &Media, next: Option<u64>) -> usize {
    let listed = media.chunks.len();
    // How far into this playlist `next` is; None when it is behind it (or there is none yet).
    let ahead = next.and_then(|n| n.checked_sub(media.sequence)).map(|n| usize::try_from(n).unwrap_or(usize::MAX));
    match ahead {
        Some(ahead) if ahead <= listed => ahead,
        // A little past the end: an older copy of the playlist (a cache's), with nothing new in it.
        Some(ahead) if ahead <= listed * 2 => listed,
        // Far past it, the stream started over and numbers its chunks afresh: waiting for them to
        // count back up would be hours of silence. That, falling behind the playlist and just
        // starting all (re)join near the live edge.
        _ => listed.saturating_sub(START_BEHIND),
    }
}

/// The audio in a chunk: without the ID3 tag (a timestamp) HLS puts before
/// packed audio, which would be noise in the middle of an ADTS stream.
fn strip_id3(chunk: &[u8]) -> &[u8] {
    if chunk.len() >= 10 && chunk.starts_with(b"ID3") {
        let size = chunk[6..10].iter().fold(0usize, |n, &b| (n << 7) | usize::from(b & 0x7f));
        let footer = if chunk[5] & 0x10 != 0 { 10 } else { 0 };
        if let Some(audio) = chunk.get(10 + size + footer..) {
            return audio;
        }
    }
    chunk
}

/// Relays the HLS stream at `url` until the receiver is dropped (MPD stopped
/// reading), the stream ends, or it can't be fetched any more.
pub(crate) fn relay(http: reqwest::Client, url: String) -> mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = mpsc::channel(AHEAD);
    tokio::spawn(async move {
        match run(&http, &url, &tx).await {
            Ok(()) => debug!("HLS relay of {url} ended"),
            Err(e) => warn!("HLS relay of {url}: {e:#}"),
        }
    });
    rx
}

async fn fetch(http: &reqwest::Client, url: &Url) -> anyhow::Result<(Url, String)> {
    let response = http.get(url.clone()).send().await?.error_for_status()?;
    // Chunks are relative to where redirects led.
    let base = response.url().clone();
    Ok((base, response.text().await?))
}

async fn fetch_chunk(http: &reqwest::Client, url: &Url) -> anyhow::Result<Vec<u8>> {
    let bytes = http.get(url.clone()).send().await?.error_for_status()?.bytes().await?;
    Ok(strip_id3(&bytes).to_vec())
}

async fn run(http: &reqwest::Client, url: &str, tx: &mpsc::Sender<Vec<u8>>) -> anyhow::Result<()> {
    let mut playlist = Url::parse(url)?;
    let mut next: Option<u64> = None;
    let mut failures = 0;
    let mut hops = 0;
    loop {
        // One fetch that fails (the playlist's, or a chunk's) is tried again: the playlist is read
        // afresh, and carries on at the chunk it got to, or near live if that one has gone.
        let round = async {
            let (base, text) = fetch(http, &playlist).await?;
            let read = parse(&base, &text)?;
            if let Playlist::Media(media) = &read {
                for (index, chunk) in media.chunks.iter().enumerate().skip(start_index(media, next)) {
                    if tx.send(fetch_chunk(http, chunk).await?).await.is_err() {
                        break;
                    }
                    next = Some(media.sequence + index as u64 + 1);
                }
            }
            anyhow::Ok(read)
        };
        let media = match round.await {
            Ok(Playlist::Media(media)) => media,
            Ok(Playlist::Master(variant)) if hops < MAX_HOPS => {
                hops += 1;
                playlist = variant;
                continue;
            }
            Ok(Playlist::Master(_)) => bail!("playlists that only lead to more playlists"),
            Err(e) if failures < RETRIES => {
                failures += 1;
                debug!("HLS stream {playlist}: {e:#}; retrying");
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
            Err(e) => return Err(e),
        };
        failures = 0;
        if media.ended || tx.is_closed() {
            return Ok(());
        }
        // New chunks appear about once per target duration.
        tokio::time::sleep(media.target / 2).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Url {
        Url::parse("https://edge.example/hls/abc/hls.m3u8?lsid=1").unwrap()
    }

    #[test]
    fn follows_a_master_playlist_to_its_variant() {
        let master = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=256000,CODECS=\"mp4a.40.2\"\nhttps://audio-edge.example/abc/hls.m3u8?lsid=9\n";
        assert_eq!(
            parse(&base(), master).unwrap(),
            Playlist::Master(Url::parse("https://audio-edge.example/abc/hls.m3u8?lsid=9").unwrap())
        );
    }

    #[test]
    fn reads_a_media_playlist() {
        let text = "#EXTM3U\n#EXT-X-TARGETDURATION:12\n#EXT-X-VERSION:3\n#EXT-X-MEDIA-SEQUENCE:10869\n\
                    #EXTINF:12.30,\nhttps://edge.example:443/hls/abc/segment_10869.aac\n\
                    #EXTINF:12.28,\nsegment_10870.aac\n";
        let Playlist::Media(media) = parse(&base(), text).unwrap() else { panic!("not a media playlist") };
        assert_eq!((media.sequence, media.target, media.ended), (10_869, Duration::from_secs(12), false));
        assert_eq!(
            media.chunks.iter().map(Url::as_str).collect::<Vec<_>>(),
            ["https://edge.example/hls/abc/segment_10869.aac", "https://edge.example/hls/abc/segment_10870.aac"]
        );
        assert!(parse(&base(), "<html>").is_err());
    }

    #[test]
    fn starts_near_live_and_carries_on_where_it_left_off() {
        let chunks = (0..6).map(|i| base().join(&format!("s{i}.aac")).unwrap()).collect();
        let media = Media { sequence: 100, target: Duration::from_secs(12), chunks, ended: false };
        assert_eq!(start_index(&media, None), 3, "three chunks behind live");
        assert_eq!(start_index(&media, Some(104)), 4, "the chunk after the last one sent");
        assert_eq!(start_index(&media, Some(106)), 6, "nothing new yet");
        assert_eq!(start_index(&media, Some(90)), 3, "fell behind: rejoin near live");
        assert_eq!(start_index(&media, Some(108)), 6, "an older copy of the playlist: nothing new");
        assert_eq!(start_index(&media, Some(10_870)), 3, "the stream started over: rejoin near live");
    }

    #[test]
    fn a_playlist_gone_wrong_is_an_error() {
        for target in ["-1", "nan", "inf", "1e400", "twelve"] {
            let text = format!("#EXTM3U\n#EXT-X-TARGETDURATION:{target}\nsegment_1.aac\n");
            assert!(parse(&base(), &text).is_err(), "{target}");
        }
    }

    /// A one-request-per-connection HTTP server: a playlist of two chunks that has ended, the
    /// second of which fails the first time it is asked for.
    async fn flaky_stream() -> String {
        use std::sync::atomic::{AtomicBool, Ordering};
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

        static FAILED_ONCE: AtomicBool = AtomicBool::new(false);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let (read, mut write) = socket.into_split();
                    let mut lines = BufReader::new(read).lines();
                    let request = lines.next_line().await.unwrap().unwrap_or_default();
                    while lines.next_line().await.unwrap().is_some_and(|line| !line.is_empty()) {}
                    let (status, body) = match request.split(' ').nth(1).unwrap_or_default() {
                        "/hls.m3u8" => (
                            "200 OK",
                            "#EXTM3U\n#EXT-X-TARGETDURATION:1\n#EXT-X-MEDIA-SEQUENCE:7\na.aac\nb.aac\n#EXT-X-ENDLIST\n",
                        ),
                        "/a.aac" => ("200 OK", "first"),
                        "/b.aac" if FAILED_ONCE.swap(true, Ordering::Relaxed) => ("200 OK", "second"),
                        "/b.aac" => ("503 Service Unavailable", ""),
                        _ => ("404 Not Found", ""),
                    };
                    let head =
                        format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                    let _ = write.write_all(format!("{head}{body}").as_bytes()).await;
                });
            }
        });
        format!("http://{addr}/hls.m3u8")
    }

    #[tokio::test]
    async fn a_chunk_that_fails_is_asked_for_again() {
        let nts = crate::Nts::new().unwrap();
        let mut chunks = nts.relay_aac(&flaky_stream().await);
        assert_eq!(chunks.recv().await.as_deref(), Some(&b"first"[..]));
        assert_eq!(chunks.recv().await.as_deref(), Some(&b"second"[..]), "once, after the one before it");
        assert_eq!(chunks.recv().await, None, "the playlist has ended");
    }

    #[test]
    fn strips_the_id3_timestamp() {
        // "ID3", v4, no flags, a 63-byte tag (syncsafe size), then ADTS sync bytes.
        let mut chunk = b"ID3\x04\x00\x00\x00\x00\x00\x3f".to_vec();
        chunk.extend([0u8; 63]);
        chunk.extend([0xff, 0xf1, 0x50]);
        assert_eq!(strip_id3(&chunk), [0xff, 0xf1, 0x50]);
        assert_eq!(strip_id3(&[0xff, 0xf1]), [0xff, 0xf1], "no tag");
        assert_eq!(strip_id3(b"ID3\x04\x00\x00\x00\x00\x01\x00"), b"ID3\x04\x00\x00\x00\x00\x01\x00", "truncated tag");
    }
}
