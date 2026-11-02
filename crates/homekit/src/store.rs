//! The accessory's identity and its paired controllers, kept in one JSON file.
//! It holds the accessory's private key, so it is written with mode 0600.

use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use anyhow::Context;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use ring::signature::{Ed25519KeyPair, KeyPair};
use serde::{Deserialize, Serialize};

use crate::{crypto, tlv};

/// HomeKit keeps at most 16 controllers per accessory.
const MAX_PAIRINGS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pairing {
    /// The controller's pairing identifier (a UUID string).
    pub id: String,
    /// Its Ed25519 long-term public key, base64.
    pub public_key: String,
    pub admin: bool,
}

impl Pairing {
    pub fn new(id: &[u8], public_key: &[u8], admin: bool) -> Self {
        Self { id: String::from_utf8_lossy(id).into_owned(), public_key: BASE64.encode(public_key), admin }
    }

    pub fn public_key(&self) -> Vec<u8> {
        BASE64.decode(&self.public_key).unwrap_or_default()
    }
}

#[derive(Serialize, Deserialize)]
struct Data {
    /// `AA:BB:CC:DD:EE:FF`, the accessory's pairing identifier.
    device_id: String,
    /// Ed25519 seed of the accessory's long-term key, base64.
    seed: String,
    /// `XXX-XX-XXX`.
    setup_code: String,
    /// Four characters that tie a setup URI to this accessory.
    setup_id: String,
    /// `c#`: goes up whenever the accessory's database changes.
    config_number: u32,
    /// What `c#` was computed for.
    signature: String,
    pairings: Vec<Pairing>,
}

pub struct Store {
    path: PathBuf,
    data: Data,
    keys: Ed25519KeyPair,
}

impl Store {
    /// Loads the store, creating a new identity if there is none. `signature`
    /// describes the accessory database; when it changes, `c#` goes up.
    pub fn open(path: &Path, signature: &str) -> anyhow::Result<Self> {
        let mut data = match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Data {
                device_id: crypto::random::<6>().map(|b| format!("{b:02X}")).join(":"),
                seed: BASE64.encode(crypto::random::<32>()),
                setup_code: new_setup_code(),
                setup_id: crypto::random::<4>()
                    .map(|b| char::from(b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"[b as usize % 36]))
                    .iter()
                    .collect(),
                config_number: 0,
                signature: String::new(),
                pairings: Vec::new(),
            },
            Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
        };
        let seed = BASE64.decode(&data.seed).context("homekit store: bad seed")?;
        let keys = Ed25519KeyPair::from_seed_unchecked(&seed).map_err(|e| anyhow::anyhow!("homekit store: {e}"))?;
        let changed = data.signature != signature;
        if changed {
            data.config_number = data.config_number % 65535 + 1;
            data.signature = signature.to_string();
        }
        let store = Self { path: path.to_path_buf(), data, keys };
        if changed {
            store.save()?;
        }
        Ok(store)
    }

    fn save(&self) -> anyhow::Result<()> {
        let tmp = self.path.with_extension("json.tmp");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)
            .with_context(|| format!("writing {}", tmp.display()))?;
        file.write_all(&serde_json::to_vec_pretty(&self.data)?)?;
        file.sync_all()?;
        std::fs::rename(&tmp, &self.path).with_context(|| format!("writing {}", self.path.display()))
    }

    pub fn device_id(&self) -> &str {
        &self.data.device_id
    }

    pub fn setup_code(&self) -> &str {
        &self.data.setup_code
    }

    pub fn setup_id(&self) -> &str {
        &self.data.setup_id
    }

    pub fn config_number(&self) -> u32 {
        self.data.config_number
    }

    pub fn public_key(&self) -> &[u8] {
        self.keys.public_key().as_ref()
    }

    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        self.keys.sign(message).as_ref().to_vec()
    }

    pub fn paired(&self) -> bool {
        !self.data.pairings.is_empty()
    }

    pub fn pairing(&self, id: &str) -> Option<&Pairing> {
        self.data.pairings.iter().find(|p| p.id == id)
    }

    pub fn pairings(&self) -> &[Pairing] {
        &self.data.pairings
    }

    /// Adds a controller, or updates the permissions of a known one.
    pub fn add(&mut self, pairing: Pairing) -> Result<(), tlv::Error> {
        if let Some(known) = self.data.pairings.iter_mut().find(|p| p.id == pairing.id) {
            if known.public_key != pairing.public_key {
                return Err(tlv::Error::Unknown);
            }
            known.admin = pairing.admin;
        } else if self.data.pairings.len() >= MAX_PAIRINGS {
            return Err(tlv::Error::MaxPeers);
        } else {
            self.data.pairings.push(pairing);
        }
        self.save().map_err(|_| tlv::Error::Unknown)
    }

    /// Removes a controller. Without an admin left nobody could manage the
    /// accessory, so then every pairing goes and it can be set up afresh.
    pub fn remove(&mut self, id: &str) -> anyhow::Result<()> {
        self.data.pairings.retain(|p| p.id != id);
        if !self.data.pairings.iter().any(|p| p.admin) {
            self.data.pairings.clear();
        }
        self.save()
    }

    pub fn reset(&mut self) -> anyhow::Result<()> {
        self.data.pairings.clear();
        self.save()
    }
}

/// Eight random digits, not one of the codes HAP forbids as too easy to guess.
fn new_setup_code() -> String {
    loop {
        let digits = format!("{:08}", u32::from_le_bytes(crypto::random()) % 100_000_000);
        let trivial = digits == "12345678" || digits == "87654321" || digits.bytes().all(|d| d == digits.as_bytes()[0]);
        if !trivial {
            return format!("{}-{}-{}", &digits[..3], &digits[3..5], &digits[5..]);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("virgil-homekit-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("homekit.json")
    }

    #[test]
    fn identity_survives_and_config_number_follows_the_database() {
        let path = temp("identity");
        let store = Store::open(&path, "Virgil").unwrap();
        let (id, key, code) =
            (store.device_id().to_string(), store.public_key().to_vec(), store.setup_code().to_string());
        assert_eq!(store.config_number(), 1);
        assert_eq!(code.len(), 10);
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);

        let store = Store::open(&path, "Virgil").unwrap();
        assert_eq!(
            (store.device_id(), store.public_key(), store.setup_code()),
            (id.as_str(), key.as_slice(), code.as_str())
        );
        assert_eq!(store.config_number(), 1);
        assert_eq!(Store::open(&path, "Kitchen").unwrap().config_number(), 2);
    }

    #[test]
    fn removing_the_last_admin_unpairs() {
        let mut store = Store::open(&temp("pairings"), "Virgil").unwrap();
        store.add(Pairing::new(b"phone", &[1; 32], true)).unwrap();
        store.add(Pairing::new(b"hub", &[2; 32], false)).unwrap();
        assert_eq!(store.add(Pairing::new(b"hub", &[3; 32], true)), Err(tlv::Error::Unknown));
        store.add(Pairing::new(b"hub", &[2; 32], true)).unwrap();
        store.remove("phone").unwrap();
        assert!(store.paired());
        store.add(Pairing::new(b"guest", &[4; 32], false)).unwrap();
        store.remove("hub").unwrap();
        assert!(!store.paired());
    }
}
