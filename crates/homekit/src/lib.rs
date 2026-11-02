//! Virgil as a HomeKit switch: HAP over IP for one accessory with one
//! on/off characteristic, so Siri and the Home app can stop playback.
//!
//! Pairing, the encrypted sessions and the switch's events are served here;
//! the accessory is advertised through the host's avahi-daemon (over D-Bus)
//! rather than a second mDNS responder. What "on" means, and what turning
//! it on or off does, is up to the caller.

mod accessory;
#[cfg(test)]
mod controller;
mod crypto;
mod mdns;
mod pairing;
mod server;
mod session;
mod srp;
mod store;
mod tlv;

use std::net::{Ipv6Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::Context;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot, watch};
use tracing::info;

use crate::store::Store;

/// The accessory category shown while pairing.
const CATEGORY_SWITCH: u64 = 8;

#[derive(Debug, Clone)]
pub struct Config {
    /// Shown in the Home app and used by Siri ("turn off Virgil").
    pub name: String,
    pub port: u16,
    /// Where the accessory's keys and pairings are kept.
    pub store: PathBuf,
    pub manufacturer: String,
    pub model: String,
    /// `x.y.z`.
    pub firmware: String,
}

/// A controller asked to turn the switch on or off. Answer on `done` whether
/// that worked; the Home app shows an error if it didn't.
#[derive(Debug)]
pub struct Write {
    pub on: bool,
    pub done: oneshot::Sender<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    /// Added to a home; nobody else can pair until it is removed (or reset).
    pub paired: bool,
    /// Registered with avahi, so controllers can find it.
    pub advertised: bool,
    /// `XXX-XX-XXX`, for adding it by hand.
    pub setup_code: String,
    /// `X-HM://…`: opens the Home app's pairing on an iPhone.
    pub setup_uri: String,
}

/// Handle to the running accessory.
#[derive(Clone)]
pub struct Switch(Arc<Shared>);

pub(crate) struct Shared {
    config: Config,
    store: Mutex<Store>,
    on: watch::Sender<bool>,
    status: watch::Sender<Status>,
    /// Bumped whenever the controllers change.
    pairings: watch::Sender<u64>,
    writes: mpsc::Sender<Write>,
    setup: Mutex<pairing::SetupGuard>,
}

impl Switch {
    /// Loads (or creates) the accessory's identity, listens on `config.port`
    /// and starts advertising. Writes from controllers arrive on the receiver.
    pub async fn start(config: Config) -> anyhow::Result<(Self, mpsc::Receiver<Write>)> {
        let store = Store::open(&config.store, &accessory::signature(&config.name))?;
        let listener = listen(config.port).with_context(|| format!("HomeKit: listening on port {}", config.port))?;
        let status = status(&store, false);
        info!("HomeKit: {:?} on port {}, setup code {}", config.name, config.port, status.setup_code);
        let (writes, receiver) = mpsc::channel(4);
        let shared = Arc::new(Shared {
            config,
            store: Mutex::new(store),
            on: watch::Sender::new(false),
            status: watch::Sender::new(status),
            pairings: watch::Sender::new(0),
            writes,
            setup: Mutex::default(),
        });
        tokio::spawn(server::run(shared.clone(), listener));
        tokio::spawn(mdns::run(shared.clone()));
        Ok((Self(shared), receiver))
    }

    /// Publishes the switch's state; subscribed controllers hear of changes.
    pub fn set_on(&self, on: bool) {
        self.0.on.send_if_modified(|value| std::mem::replace(value, on) != on);
    }

    pub fn status(&self) -> watch::Receiver<Status> {
        self.0.status.subscribe()
    }

    /// Forgets every controller, so the accessory can be added again (after it
    /// was removed from the Home app while it was offline, say).
    pub fn reset(&self) -> anyhow::Result<()> {
        self.0.store().reset()?;
        info!("HomeKit: pairings reset");
        self.0.pairings_changed();
        Ok(())
    }
}

impl Shared {
    fn store(&self) -> MutexGuard<'_, Store> {
        self.store.lock().expect("homekit store lock")
    }

    fn pairings_changed(&self) {
        self.pairings.send_modify(|n| *n += 1);
        let store = self.store();
        self.status.send_if_modified(|s| {
            let new = status(&store, s.advertised);
            std::mem::replace(s, new) != *s
        });
    }

    fn set_advertised(&self, advertised: bool) {
        self.status.send_if_modified(|s| std::mem::replace(&mut s.advertised, advertised) != advertised);
    }
}

fn status(store: &Store, advertised: bool) -> Status {
    Status {
        paired: store.paired(),
        advertised,
        setup_code: store.setup_code().to_string(),
        setup_uri: setup_uri(store.setup_code(), store.setup_id()),
    }
}

/// Both IPv4 and IPv6 where the host has IPv6 (controllers use either).
fn listen(port: u16) -> std::io::Result<TcpListener> {
    let v6 = || -> std::io::Result<std::net::TcpListener> {
        let socket = socket2::Socket::new(socket2::Domain::IPV6, socket2::Type::STREAM, None)?;
        socket.set_only_v6(false)?;
        socket.set_reuse_address(true)?;
        socket.bind(&SocketAddr::from((Ipv6Addr::UNSPECIFIED, port)).into())?;
        socket.listen(32)?;
        Ok(socket.into())
    };
    let listener = v6().or_else(|_| std::net::TcpListener::bind(("0.0.0.0", port)))?;
    listener.set_nonblocking(true)?;
    TcpListener::from_std(listener)
}

/// The setup payload the Home app reads from a QR code or link: category,
/// "IP accessory" flag and setup code in base 36, then the setup id.
fn setup_uri(code: &str, setup_id: &str) -> String {
    let code: u64 = code.replace('-', "").parse().unwrap_or(0);
    let mut payload = CATEGORY_SWITCH << 31 | 2 << 27 | code;
    let mut digits = [b'0'; 9];
    for digit in digits.iter_mut().rev() {
        *digit = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"[(payload % 36) as usize];
        payload /= 36;
    }
    format!("X-HM://{}{setup_id}", std::str::from_utf8(&digits).expect("ASCII"))
}

/// `sh` in the TXT record: lets the Home app match a setup URI to the accessory.
fn setup_hash(setup_id: &str, device_id: &str) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA512, format!("{setup_id}{device_id}").as_bytes());
    BASE64.encode(&digest.as_ref()[..4])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_uri_encodes_category_flags_and_code() {
        // 8 << 31 | 2 << 27 | 51808582 = 17_500_113_222 = "81F45S6" in base 36.
        assert_eq!(setup_uri("518-08-582", "1QJ8"), "X-HM://0081F45S61QJ8");
    }

    #[test]
    fn setup_hash_is_four_bytes_of_base64() {
        assert_eq!(setup_hash("1QJ8", "AA:BB:CC:DD:EE:FF").len(), 8);
    }
}
