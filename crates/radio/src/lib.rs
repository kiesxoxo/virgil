//! Internet radio from NTS (nts.live): the two live channels, with what's on
//! air from NTS's schedule, and the Infinite Mixtapes. MPD plays the streams;
//! this crate knows the stations and what's on, and relays the mixtapes' AAC
//! streams (HLS) as plain streams MPD can play.

mod hls;

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Context;
use serde::Deserialize;
use tokio::sync::{Mutex, mpsc};
use tracing::warn;
use virgil_core::{Broadcast, Radio, Station};

const USER_AGENT: &str = concat!("Virgil/", env!("CARGO_PKG_VERSION"), " ( https://github.com/kiesxoxo/virgil )");
const LIVE_URL: &str = "https://www.nts.live/api/v2/live";
const MIXTAPES_URL: &str = "https://www.nts.live/api/v2/mixtapes";
/// The live channels: their name in NTS's schedule, Virgil's id, and stream.
const CHANNELS: [(&str, &str, &str); 2] = [
    ("1", "nts1", "https://stream-relay-geo.ntslive.net/stream"),
    ("2", "nts2", "https://stream-relay-geo.ntslive.net/stream2"),
];
/// The mixtape list hardly ever changes.
const MIXTAPES_TTL: Duration = Duration::from_secs(6 * 3600);
/// The schedule is reused until the earliest show on air ends, within these bounds.
const LIVE_TTL_MIN: Duration = Duration::from_secs(30);
const LIVE_TTL_MAX: Duration = Duration::from_secs(10 * 60);
/// After a failed fetch the last good data is served this long before trying again.
const RETRY: Duration = Duration::from_secs(60);

#[derive(Deserialize)]
struct LiveResponse {
    results: Vec<Channel>,
}

#[derive(Deserialize)]
struct Channel {
    channel_name: String,
    now: Slot,
    next: Option<Slot>,
}

#[derive(Deserialize)]
struct Slot {
    broadcast_title: String,
    start_timestamp: String,
    end_timestamp: String,
    #[serde(default)]
    embeds: SlotEmbeds,
}

#[derive(Deserialize, Default)]
struct SlotEmbeds {
    details: Option<Details>,
}

#[derive(Deserialize)]
struct Details {
    description: Option<String>,
    location_long: Option<String>,
    #[serde(default)]
    genres: Vec<Genre>,
    #[serde(default)]
    media: Media,
    show_alias: Option<String>,
    episode_alias: Option<String>,
}

#[derive(Deserialize)]
struct Genre {
    value: String,
}

#[derive(Deserialize, Default)]
struct Media {
    picture_medium_large: Option<String>,
    picture_small: Option<String>,
    icon_black: Option<String>,
    icon_white: Option<String>,
}

#[derive(Deserialize)]
struct MixtapesResponse {
    results: Vec<Mixtape>,
}

#[derive(Deserialize)]
struct Mixtape {
    mixtape_alias: String,
    title: String,
    #[serde(default)]
    subtitle: String,
    description: Option<String>,
    audio_stream_endpoint: String,
    audio_stream_endpoint_hls_aac: Option<String>,
    #[serde(default)]
    media: Media,
}

/// None for a missing or blank string.
fn text(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Days from 1970-01-01 to a date in the proleptic Gregorian calendar.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
    era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468
}

/// Unix time from an RFC 3339 timestamp: `2026-10-30T11:00:00Z`, with an
/// optional fraction and `Z` or a `+01:00` offset.
fn unix(s: &str) -> Option<u32> {
    let num = |at: usize, len: usize| s.get(at..at + len)?.parse::<i64>().ok();
    let days = days_from_civil(num(0, 4)?, num(5, 2)?, num(8, 2)?);
    let seconds = num(11, 2)? * 3600 + num(14, 2)? * 60 + num(17, 2)?;
    let zone = s.get(19..)?.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    let offset = match zone {
        "" | "Z" => 0,
        z => {
            let sign = match z.as_bytes()[0] {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            sign * (z.get(1..3)?.parse::<i64>().ok()? * 3600 + z.get(4..6)?.parse::<i64>().ok()? * 60)
        }
    };
    u32::try_from(days * 86_400 + seconds - offset).ok()
}

fn broadcast(slot: Slot) -> Option<Broadcast> {
    let details = slot.embeds.details;
    let url = details.as_ref().and_then(|d| {
        let (show, episode) = (d.show_alias.as_deref()?, d.episode_alias.as_deref()?);
        Some(format!("https://www.nts.live/shows/{show}/episodes/{episode}"))
    });
    let (description, location, genres, media) = match details {
        Some(d) => (text(d.description), text(d.location_long), d.genres, d.media),
        None => (None, None, Vec::new(), Media::default()),
    };
    Some(Broadcast {
        title: slot.broadcast_title.trim().to_string(),
        start: unix(&slot.start_timestamp)?,
        end: unix(&slot.end_timestamp)?,
        location,
        description,
        genres: genres.into_iter().filter_map(|g| text(Some(g.value))).collect(),
        art: text(media.picture_medium_large),
        thumb: text(media.picture_small),
        url,
    })
}

/// NTS 1 and NTS 2 from `/api/v2/live`, in that order. A channel missing
/// from the schedule is still listed, with nothing on air.
fn parse_live(json: &str) -> anyhow::Result<Vec<Station>> {
    let mut response: LiveResponse = serde_json::from_str(json).context("reading NTS's schedule")?;
    Ok(CHANNELS
        .iter()
        .map(|&(channel, id, stream)| {
            let (now, next) = match response.results.iter().position(|c| c.channel_name == channel) {
                Some(i) => {
                    let c = response.results.swap_remove(i);
                    (broadcast(c.now), c.next.and_then(broadcast))
                }
                None => (None, None),
            };
            Station {
                id: id.to_string(),
                name: format!("NTS {channel}"),
                tagline: String::new(),
                description: None,
                stream: stream.to_string(),
                // NTS's schedule lists no AAC streams for the live channels.
                stream_aac: None,
                art: now.as_ref().and_then(|b| b.art.clone()),
                thumb: now.as_ref().and_then(|b| b.thumb.clone()),
                icon_black: None,
                icon_white: None,
                now,
                next,
            }
        })
        .collect())
}

/// The Infinite Mixtapes from `/api/v2/mixtapes`, in NTS's order.
fn parse_mixtapes(json: &str) -> anyhow::Result<Vec<Station>> {
    let response: MixtapesResponse = serde_json::from_str(json).context("reading NTS's mixtapes")?;
    Ok(response
        .results
        .into_iter()
        .map(|m| Station {
            id: m.mixtape_alias,
            name: m.title.trim().to_string(),
            tagline: m.subtitle.trim().to_string(),
            description: text(m.description),
            stream: m.audio_stream_endpoint,
            stream_aac: text(m.audio_stream_endpoint_hls_aac),
            art: text(m.media.picture_medium_large),
            thumb: text(m.media.picture_small),
            icon_black: text(m.media.icon_black),
            icon_white: text(m.media.icon_white),
            now: None,
            next: None,
        })
        .collect())
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Until the first show on air ends (a little after, so the schedule has moved on).
fn live_ttl(stations: &[Station]) -> Duration {
    let now = unix_now();
    stations
        .iter()
        .filter_map(|s| s.now.as_ref())
        .map(|b| Duration::from_secs(u64::from(b.end).saturating_sub(now) + 5))
        .min()
        .unwrap_or(LIVE_TTL_MIN)
        .clamp(LIVE_TTL_MIN, LIVE_TTL_MAX)
}

struct Cached {
    stations: Vec<Station>,
    fresh_until: Instant,
}

/// NTS's stations, fetched on demand and cached.
pub struct Nts {
    http: reqwest::Client,
    live: Mutex<Option<Cached>>,
    mixtapes: Mutex<Option<Cached>>,
}

impl Nts {
    pub fn new() -> anyhow::Result<Self> {
        // reqwest is built without a bundled crypto provider; ring is pure Rust and cross-compiles cleanly.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(15))
            .build()?;
        Ok(Self { http, live: Mutex::default(), mixtapes: Mutex::default() })
    }

    /// The live channels with what's on, and the mixtapes. When NTS can't be
    /// reached, the last data fetched is served.
    pub async fn radio(&self) -> anyhow::Result<Radio> {
        let (live, mixtapes) = tokio::join!(
            self.cached(&self.live, LIVE_URL, parse_live, live_ttl),
            self.cached(&self.mixtapes, MIXTAPES_URL, parse_mixtapes, |_| MIXTAPES_TTL),
        );
        Ok(Radio { live: live?, mixtapes: mixtapes? })
    }

    pub async fn station(&self, id: &str) -> anyhow::Result<Option<Station>> {
        let radio = self.radio().await?;
        Ok(radio.live.into_iter().chain(radio.mixtapes).find(|s| s.id == id))
    }

    /// The HLS stream at `playlist` (a station's `stream_aac`) as one ADTS
    /// stream, chunk by chunk, until the receiver is dropped.
    pub fn relay_aac(&self, playlist: &str) -> mpsc::Receiver<Vec<u8>> {
        hls::relay(self.http.clone(), playlist.to_string())
    }

    async fn cached(
        &self,
        slot: &Mutex<Option<Cached>>,
        url: &str,
        parse: fn(&str) -> anyhow::Result<Vec<Station>>,
        ttl: fn(&[Station]) -> Duration,
    ) -> anyhow::Result<Vec<Station>> {
        // Held across the fetch, so pages asking at once share one request.
        let mut slot = slot.lock().await;
        if let Some(cached) = slot.as_ref().filter(|c| Instant::now() < c.fresh_until) {
            return Ok(cached.stations.clone());
        }
        match self.fetch(url).await.and_then(|json| parse(&json)) {
            Ok(stations) => {
                *slot = Some(Cached { fresh_until: Instant::now() + ttl(&stations), stations: stations.clone() });
                Ok(stations)
            }
            Err(e) => match slot.as_mut() {
                Some(stale) => {
                    warn!("NTS: {e:#}; showing what was fetched before");
                    stale.fresh_until = Instant::now() + RETRY;
                    Ok(stale.stations.clone())
                }
                None => Err(e),
            },
        }
    }

    async fn fetch(&self, url: &str) -> anyhow::Result<String> {
        let response = self.http.get(url).send().await.context("cannot reach nts.live")?;
        Ok(response.error_for_status()?.text().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_timestamps() {
        assert_eq!(unix("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(unix("2000-03-01T00:00:00Z"), Some(951_868_800));
        assert_eq!(unix("2024-02-29T23:59:59Z"), Some(1_709_251_199));
        assert_eq!(unix("2026-10-30T11:00:00Z"), Some(1_793_358_000));
        assert_eq!(unix("2026-10-30T11:00:00.000Z"), Some(1_793_358_000));
        assert_eq!(unix("2026-10-30T11:00:00+00:00"), Some(1_793_358_000));
        assert_eq!(unix("2026-10-30T12:00:00+01:00"), Some(1_793_358_000));
        assert_eq!(unix("2026-10-30"), None);
        assert_eq!(unix("garbage"), None);
    }

    const LIVE: &str = r#"{"results": [
        {"channel_name": "2",
         "now": {"broadcast_title": "Vanishing Twin", "start_timestamp": "2026-10-30T12:00:00Z", "end_timestamp": "2026-10-30T13:00:00Z",
                 "embeds": {"details": {"description": "", "location_long": "London", "genres": [],
                                        "media": {"picture_medium_large": "https://media/vt-800.jpeg", "picture_small": "https://media/vt-200.jpeg"},
                                        "show_alias": "vanishing-twin", "episode_alias": "vanishing-twin-30th-october-2026"}}},
         "next": {"broadcast_title": "Orpheu The Wizard", "start_timestamp": "2026-10-30T13:00:00Z", "end_timestamp": "2026-10-30T15:00:00Z"}},
        {"channel_name": "1",
         "now": {"broadcast_title": " Soup To Nuts w/ Manara ", "start_timestamp": "2026-10-30T11:00:00Z", "end_timestamp": "2026-10-30T13:00:00Z",
                 "embeds": {"details": {"description": "Lunchtime.", "location_long": "London",
                                        "genres": [{"id": "genres-caribbean-dancehall", "value": "Dancehall"}],
                                        "media": {}}}}}
    ]}"#;

    #[test]
    fn reads_the_live_schedule() {
        let live = parse_live(LIVE).unwrap();
        assert_eq!(live.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["nts1", "nts2"]);

        let nts1 = &live[0];
        assert_eq!((nts1.name.as_str(), nts1.stream.as_str()), ("NTS 1", CHANNELS[0].2));
        assert_eq!(nts1.stream_aac, None);
        let now = nts1.now.as_ref().unwrap();
        assert_eq!(now.title, "Soup To Nuts w/ Manara");
        assert_eq!((now.start, now.end), (1_793_358_000, 1_793_365_200));
        assert_eq!(now.genres, ["Dancehall"]);
        assert_eq!((now.art.as_deref(), now.url.as_deref()), (None, None));
        assert!(nts1.next.is_none());

        let nts2 = &live[1];
        let now = nts2.now.as_ref().unwrap();
        assert_eq!(now.description, None, "blank text is no text");
        assert_eq!(nts2.art.as_deref(), Some("https://media/vt-800.jpeg"), "a channel shows its show's art");
        assert_eq!(
            now.url.as_deref(),
            Some("https://www.nts.live/shows/vanishing-twin/episodes/vanishing-twin-30th-october-2026")
        );
        assert_eq!(nts2.next.as_ref().unwrap().title, "Orpheu The Wizard");
    }

    #[test]
    fn lists_a_channel_missing_from_the_schedule() {
        let live = parse_live(r#"{"results": []}"#).unwrap();
        assert_eq!(live.len(), 2);
        assert!(live.iter().all(|s| s.now.is_none()));
    }

    #[test]
    fn reads_mixtapes() {
        let mixtapes = parse_mixtapes(
            r#"{"metadata": {}, "results": [
                {"mixtape_alias": "poolside", "title": "Poolside", "subtitle": "Balearic, boogie, and sophisti-pop.",
                 "description": "Whisk yourself away.", "audio_stream_endpoint": "https://stream-mixtape-geo.ntslive.net/mixtape4",
                 "audio_stream_endpoint_hls_aac": "https://streams.radiomast.io/e7bc1d1a/hls.m3u8",
                 "credits": [], "now_playing_topic": "",
                 "media": {"icon_black": "https://media/poolside-black.png", "icon_white": "https://media/poolside-white.png",
                           "picture_medium_large": "https://media/poolside-800.jpeg"}},
                {"mixtape_alias": "slow-focus", "title": "Slow Focus", "audio_stream_endpoint": "https://stream-mixtape-geo.ntslive.net/mixtape"}
            ]}"#,
        )
        .unwrap();
        assert_eq!(mixtapes.len(), 2);
        let poolside = &mixtapes[0];
        assert_eq!((poolside.id.as_str(), poolside.name.as_str()), ("poolside", "Poolside"));
        assert_eq!(poolside.stream, "https://stream-mixtape-geo.ntslive.net/mixtape4");
        assert_eq!(poolside.icon_white.as_deref(), Some("https://media/poolside-white.png"));
        assert_eq!(poolside.stream_aac.as_deref(), Some("https://streams.radiomast.io/e7bc1d1a/hls.m3u8"));
        assert_eq!(mixtapes[1].stream_aac, None);
        assert_eq!(mixtapes[1].tagline, "");
        assert_eq!(mixtapes[1].art, None);
    }

    #[test]
    fn reuses_the_schedule_until_a_show_ends() {
        let mut live = parse_live(LIVE).unwrap();
        let soon = u32::try_from(unix_now()).unwrap() + 120;
        live[0].now.as_mut().unwrap().end = soon;
        live[1].now.as_mut().unwrap().end = soon + 3600;
        let ttl = live_ttl(&live);
        assert!(ttl > Duration::from_secs(120) && ttl <= Duration::from_secs(126), "{ttl:?}");

        // A show that ended long ago (a stale schedule) and nothing on air both mean "ask again soon".
        live[0].now.as_mut().unwrap().end = 1_000;
        assert_eq!(live_ttl(&live), LIVE_TTL_MIN);
        assert_eq!(live_ttl(&[]), LIVE_TTL_MIN);
        // A long show is still looked at again within minutes, in case the schedule changes.
        live[0].now.as_mut().unwrap().end = soon + 86_400;
        live[1].now.as_mut().unwrap().end = soon + 86_400;
        assert_eq!(live_ttl(&live), LIVE_TTL_MAX);
    }
}
