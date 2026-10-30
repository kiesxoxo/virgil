//! Settings for online services, kept server-side in `data_dir/online.json`.
//! Today that's the Discogs token: it raises Discogs' rate limit for tag
//! lookups, and is never sent back to the browser.

use std::path::{Path, PathBuf};
use std::sync::RwLock;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use virgil_core::OnlineSettings;

#[derive(Default, Serialize, Deserialize)]
struct Stored {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    discogs_token: Option<String>,
}

pub struct Online {
    path: PathBuf,
    discogs_token: RwLock<Option<String>>,
    /// From virgild.toml: used when no token was set in Settings.
    configured: Option<String>,
}

impl Online {
    pub fn load(data_dir: &Path, configured: Option<String>) -> Self {
        let path = data_dir.join("online.json");
        let stored: Stored =
            std::fs::read(&path).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default();
        let configured = configured.filter(|t| !t.trim().is_empty());
        Self { path, discogs_token: RwLock::new(stored.discogs_token), configured }
    }

    pub fn discogs_token(&self) -> Option<String> {
        self.discogs_token.read().expect("online lock").clone().or_else(|| self.configured.clone())
    }

    pub fn settings(&self) -> OnlineSettings {
        OnlineSettings { discogs_token: self.discogs_token().is_some() }
    }

    /// Sets the Discogs token; an empty one removes it.
    pub fn set_discogs_token(&self, token: &str) -> anyhow::Result<()> {
        let token = Some(token.trim().to_string()).filter(|t| !t.is_empty());
        if token.as_ref().is_some_and(|t| !t.bytes().all(|b| b.is_ascii_alphanumeric())) {
            anyhow::bail!("a Discogs token is letters and digits only");
        }
        let json = serde_json::to_vec_pretty(&Stored { discogs_token: token.clone() })?;
        write_private(&self.path, &json).with_context(|| format!("saving {}", self.path.display()))?;
        *self.discogs_token.write().expect("online lock") = token;
        Ok(())
    }
}

/// Writes a file only its owner can read: it holds a credential.
fn write_private(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    std::io::Write::write_all(&mut options.open(&tmp)?, data)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_the_token_privately_and_never_reports_it() {
        let dir = std::env::temp_dir().join(format!("virgil-online-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let online = Online::load(&dir, None);
        assert_eq!(online.settings(), OnlineSettings { discogs_token: false });

        online.set_discogs_token(" abcDEF123 ").unwrap();
        assert_eq!(online.discogs_token().as_deref(), Some("abcDEF123"));
        assert!(!serde_json::to_string(&online.settings()).unwrap().contains("abc"), "the token stays on the server");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.join("online.json")).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        assert_eq!(Online::load(&dir, None).discogs_token().as_deref(), Some("abcDEF123"), "kept across restarts");

        assert!(online.set_discogs_token("not a token!").is_err());
        online.set_discogs_token("").unwrap();
        assert_eq!(online.discogs_token(), None);
        assert_eq!(Online::load(&dir, Some("fromconfig".into())).discogs_token().as_deref(), Some("fromconfig"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
