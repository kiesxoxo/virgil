//! Getting music the library doesn't have, without Virgil ever writing to it.
//!
//! [Lidarr](https://lidarr.audio) owns the library: it finds albums (through its indexers, which
//! may include Soulseek through slskdpp), tags, names and files them. It only knows whole albums,
//! so a single song comes from [slskdpp](https://github.com/kiesxoxo/slskdpp), a Soulseek client,
//! and Lidarr's manual import files it into its album.
//!
//! This crate is the two HTTP clients and the pure parts: choosing which Soulseek file is the
//! song, and mapping paths between Lidarr, slskdpp and MPD.

pub mod lidarr;
pub mod paths;
pub mod pick;
pub mod slskdpp;

use std::time::Duration;

use anyhow::anyhow;
use serde::de::DeserializeOwned;

const USER_AGENT: &str = concat!("Virgil/", env!("CARGO_PKG_VERSION"), " ( https://github.com/kiesxoxo/virgil )");

/// A client for services on the local network. Lidarr's lookups go out to its metadata server, so
/// they get a generous timeout.
fn client() -> anyhow::Result<reqwest::Client> {
    // reqwest is built without a bundled crypto provider; ring is pure Rust and cross-compiles cleanly.
    let _ = rustls::crypto::ring::default_provider().install_default();
    Ok(reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(45))
        .build()?)
}

/// The service's base URL without a trailing slash.
fn base(url: &str) -> String {
    url.trim_end_matches('/').to_string()
}

/// Reads a JSON answer, or turns a failed one into an error that says what the service said.
/// URLs are left out: they name hosts on the user's network, which the UI has no use for.
async fn answer<T: DeserializeOwned>(service: &str, response: reqwest::Result<reqwest::Response>) -> anyhow::Result<T> {
    let response = response.map_err(|e| anyhow!("{service} didn't answer ({})", e.without_url()))?;
    let status = response.status();
    let text = response.text().await.map_err(|e| anyhow!("{service}: {}", e.without_url()))?;
    if !status.is_success() {
        return Err(anyhow!("{service}: {}", error_message(&text).unwrap_or_else(|| status.to_string())));
    }
    // An empty body (202, 204) reads as null.
    let text = if text.trim().is_empty() { "null" } else { &text };
    serde_json::from_str(text).map_err(|e| anyhow!("{service} answered something unexpected ({e})"))
}

/// The message in an error body: Lidarr's `{"message"}` or list of validation failures
/// (`[{"errorMessage"}]`), slskdpp's `{"error"}`, or short plain text.
fn error_message(body: &str) -> Option<String> {
    let json: serde_json::Value = match serde_json::from_str(body) {
        Ok(json) => json,
        Err(_) => {
            let text = body.trim();
            return (!text.is_empty() && text.len() < 200 && !text.starts_with('<')).then(|| text.to_string());
        }
    };
    let field = |v: &serde_json::Value, key: &str| v.get(key).and_then(|m| m.as_str()).map(str::to_string);
    if let Some(list) = json.as_array() {
        let messages: Vec<String> = list.iter().filter_map(|v| field(v, "errorMessage")).collect();
        return (!messages.is_empty()).then(|| messages.join("; "));
    }
    field(&json, "message").or_else(|| field(&json, "error"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_error_bodies() {
        let lidarr = r#"{"message":"Search for 'x' failed. Unable to communicate with LidarrAPI.","description":"…"}"#;
        assert_eq!(error_message(lidarr).unwrap(), "Search for 'x' failed. Unable to communicate with LidarrAPI.");
        let invalid = r#"[{"propertyName":"Path","errorMessage":"Path is already configured"},{"errorMessage":"Bad"}]"#;
        assert_eq!(error_message(invalid).unwrap(), "Path is already configured; Bad");
        assert_eq!(
            error_message(r#"{"error":"not connected to the Soulseek server"}"#).unwrap(),
            "not connected to the Soulseek server"
        );
        assert_eq!(error_message("Unauthorized").unwrap(), "Unauthorized");
        assert_eq!(error_message("<html>…</html>"), None);
        assert_eq!(error_message(""), None);
    }
}
