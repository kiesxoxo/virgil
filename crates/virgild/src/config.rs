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
    /// No longer used: Virgil only reads the music, through MPD. Still accepted, so that older
    /// configs load; virgild says it can go.
    #[serde(rename = "music_dir")]
    pub old_music_dir: Option<serde::de::IgnoredAny>,
    /// ALSA card id of the DAC, as in `/proc/asound/<id>`.
    pub alsa_card: String,
    /// Where ALSA's procfs tree is visible. Docker hides `/proc/asound`, so the
    /// container image bind-mounts the host's copy elsewhere.
    pub asound_dir: PathBuf,
    /// External sources that share the DAC (AirPlay, Spotify…), by name.
    pub sources: BTreeMap<String, SourceConfig>,
    /// Appear in Apple Home as a switch, so Siri can stop playback. Off without this section.
    pub homekit: Option<HomekitConfig>,
    /// Lidarr, to find and download albums the library doesn't have. Off without this section.
    pub lidarr: Option<LidarrConfig>,
    /// slskdpp, to download single songs from Soulseek; Lidarr files them into their album, so
    /// this needs `[lidarr]` too.
    pub slskdpp: Option<SlskdppConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceConfig {
    /// Shell command that makes the source let go of the DAC when local playback starts.
    pub preempt: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HomekitConfig {
    /// The switch's name in the Home app, and what Siri answers to ("turn off Virgil").
    pub name: String,
    /// HAP's TCP port. Controllers find it through mDNS; 51826 is Homebridge's.
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LidarrConfig {
    /// Where Lidarr answers, e.g. `http://nas:8686`.
    pub url: String,
    /// Settings › General › API Key in Lidarr.
    pub api_key: String,
    /// Lidarr's root folder for this library, which must be the folder MPD plays from. Defaults to
    /// Lidarr's only root folder.
    #[serde(default)]
    pub root: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlskdppConfig {
    /// Where slskdpp answers, e.g. `http://nas:5030`.
    pub url: String,
    /// An API key from slskdpp's Settings › API keys.
    pub api_key: String,
    /// slskdpp's downloads directory as Lidarr sees it (its remote path mapping for slskdpp).
    pub downloads: String,
    /// File types to take, best first.
    #[serde(default = "default_formats")]
    pub formats: Vec<String>,
}

fn default_formats() -> Vec<String> {
    vec!["flac".into()]
}

impl Default for HomekitConfig {
    fn default() -> Self {
        Self { name: "Virgil".into(), port: 51827 }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen: ([0, 0, 0, 0], 8080).into(),
            mpd: "127.0.0.1:6600".into(),
            data_dir: "data".into(),
            old_music_dir: None,
            alsa_card: "sndrpihifiberry".into(),
            asound_dir: "/proc/asound".into(),
            sources: BTreeMap::new(),
            homekit: None,
            lidarr: None,
            slskdpp: None,
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

    /// What `card` is being fed right now: `closed`, or format/rate/channels.
    pub fn hw_params_path(&self, card: &str) -> PathBuf {
        self.asound_dir.join(card).join("pcm0p/sub0/hw_params")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deploy_config_parses() {
        let text = include_str!("../../../deploy/virgild.toml");
        let config: Config = toml::from_str(text).unwrap();
        assert_eq!(config.mpd, "/run/mpd/socket");
        assert!(config.sources["airplay"].preempt.is_some());
        let homekit = config.homekit.expect("deploy config enables HomeKit");
        assert_eq!((homekit.name.as_str(), homekit.port), ("Virgil", 51827));
        assert!(config.old_music_dir.is_none());
    }

    #[test]
    fn reads_lidarr_and_slskdpp() {
        let text = "[lidarr]\nurl = \"http://nas:8686\"\napi_key = \"k\"\n\n[slskdpp]\nurl = \"http://nas:5030\"\napi_key = \"s\"\ndownloads = \"/data/Downloads/slskdpp\"\n";
        let config: Config = toml::from_str(text).unwrap();
        let lidarr = config.lidarr.unwrap();
        assert_eq!((lidarr.url.as_str(), lidarr.root), ("http://nas:8686", None));
        let slskdpp = config.slskdpp.unwrap();
        assert_eq!(slskdpp.downloads, "/data/Downloads/slskdpp");
        assert_eq!(slskdpp.formats, ["flac"]);
        assert!(toml::from_str::<Config>("[lidarr]\nurl = \"http://nas:8686\"\n").is_err(), "the key is needed");
    }

    #[test]
    fn configs_naming_the_music_dir_still_load() {
        let config: Config = toml::from_str("music_dir = \"/music\"\n").unwrap();
        assert!(config.old_music_dir.is_some());
    }
}
