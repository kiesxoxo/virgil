use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// HTTP listen address for the UI and API.
    pub listen: SocketAddr,
    /// MPD: `host:port`, or the path of its Unix socket.
    pub mpd: String,
    /// Library database and artwork cache.
    pub data_dir: PathBuf,
    /// ALSA card id of the DAC, as in `/proc/asound/<id>`.
    pub alsa_card: String,
    /// Where ALSA's procfs tree is visible. Docker hides `/proc/asound`, so the
    /// container image bind-mounts the host's copy elsewhere.
    pub asound_dir: PathBuf,
    /// External sources that share the DAC (AirPlay, Spotify…), by name.
    pub sources: BTreeMap<String, SourceConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceConfig {
    /// Shell command that makes the source let go of the DAC when local playback starts.
    pub preempt: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen: ([0, 0, 0, 0], 8080).into(),
            mpd: "127.0.0.1:6600".into(),
            data_dir: "data".into(),
            alsa_card: "sndrpihifiberry".into(),
            asound_dir: "/proc/asound".into(),
            sources: BTreeMap::new(),
        }
    }
}

impl Config {
    /// Loads `path`, or the defaults (fine for a dev box) when there is none.
    pub fn load(path: Option<&Path>) -> anyhow::Result<Self> {
        let Some(path) = path else { return Ok(Self::default()) };
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }

    /// What the DAC is being fed right now: `closed`, or format/rate/channels.
    pub fn hw_params_path(&self) -> PathBuf {
        self.asound_dir.join(&self.alsa_card).join("pcm0p/sub0/hw_params")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deploy_config_parses() {
        let text = include_str!("../../../deploy/hifid.toml");
        let config: Config = toml::from_str(text).unwrap();
        assert_eq!(config.mpd, "/run/mpd/socket");
        assert!(config.sources["airplay"].preempt.is_some());
    }
}
