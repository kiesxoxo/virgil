//! Pair-setup (the setup code proves the controller is in the room), pair-verify
//! (each connection proves both sides hold their long-term keys) and pairing
//! management (the Home app adding the home hub and other members' devices).

use std::collections::BTreeMap;

use ring::agreement::{self, EphemeralPrivateKey, X25519};
use ring::rand::SystemRandom;
use ring::signature::{ED25519, UnparsedPublicKey};
use tracing::{info, warn};

use crate::crypto::{self, Key};
use crate::session::Session;
use crate::store::Pairing;
use crate::tlv::{self, Error};
use crate::{Shared, srp};

/// HAP's limit on wrong setup codes before pair-setup refuses for good.
const MAX_FAILURES: u32 = 100;

/// One pair-setup at a time, and a count of wrong setup codes.
#[derive(Default)]
pub struct SetupGuard {
    owner: Option<u64>,
    failures: u32,
}

/// Pair-setup in progress on a connection.
pub struct Setup {
    srp: Option<srp::Server>,
    /// SRP's K, once the controller's proof checked out.
    key: Option<Vec<u8>>,
}

/// Pair-verify in progress on a connection.
pub struct Verify {
    shared_secret: Vec<u8>,
    accessory_key: Vec<u8>,
    controller_key: Vec<u8>,
    key: Key,
}

type Items = BTreeMap<u8, Vec<u8>>;

fn state(items: &Items) -> u8 {
    items.get(&tlv::STATE).and_then(|s| s.first().copied()).unwrap_or(0)
}

fn signed_by(public_key: &[u8], message: &[u8], signature: &[u8]) -> bool {
    UnparsedPublicKey::new(&ED25519, public_key).verify(message, signature).is_ok()
}

/// The connection went away: let another controller pair.
pub fn release(shared: &Shared, conn: u64) {
    let mut guard = shared.setup.lock().expect("setup lock");
    if guard.owner == Some(conn) {
        guard.owner = None;
    }
}

/// `POST /pair-setup`.
pub async fn setup(shared: &Shared, conn: u64, setup: &mut Option<Setup>, body: &[u8]) -> Vec<u8> {
    let Some(items) = tlv::decode(body) else { return tlv::error(2, Error::Unknown) };
    match state(&items) {
        1 => {
            if shared.store().paired() {
                return tlv::error(2, Error::Unavailable);
            }
            {
                let mut guard = shared.setup.lock().expect("setup lock");
                if guard.failures >= MAX_FAILURES {
                    return tlv::error(2, Error::MaxTries);
                }
                if guard.owner.is_some_and(|owner| owner != conn) {
                    return tlv::error(2, Error::Busy);
                }
                guard.owner = Some(conn);
            }
            let salt = crypto::random::<16>();
            let code = shared.store().setup_code().to_string();
            let server =
                tokio::task::spawn_blocking(move || srp::Server::new(&salt, code.as_bytes(), &crypto::random::<32>()))
                    .await
                    .expect("SRP setup");
            let response =
                tlv::encode(&[(tlv::STATE, &[2]), (tlv::SALT, &salt), (tlv::PUBLIC_KEY, &server.public_key())]);
            *setup = Some(Setup { srp: Some(server), key: None });
            response
        }
        3 => {
            let Some(server) = setup.as_mut().and_then(|s| s.srp.take()) else { return tlv::error(4, Error::Unknown) };
            let (Some(a_pub), Some(proof)) = (items.get(&tlv::PUBLIC_KEY).cloned(), items.get(&tlv::PROOF).cloned())
            else {
                return tlv::error(4, Error::Unknown);
            };
            let verified =
                tokio::task::spawn_blocking(move || server.verify(&a_pub, &proof)).await.expect("SRP verify");
            let Some(verified) = verified else {
                warn!("HomeKit: pairing attempt with a wrong setup code");
                *setup = None;
                let mut guard = shared.setup.lock().expect("setup lock");
                guard.failures += 1;
                guard.owner = None;
                return tlv::error(4, Error::Authentication);
            };
            let response = tlv::encode(&[(tlv::STATE, &[4]), (tlv::PROOF, &verified.proof)]);
            if let Some(setup) = setup {
                setup.key = Some(verified.key);
            }
            response
        }
        5 => {
            let Some(srp_key) = setup.take().and_then(|s| s.key) else { return tlv::error(6, Error::Unknown) };
            release(shared, conn);
            match exchange(shared, &srp_key, &items) {
                Ok(response) => {
                    shared.pairings_changed();
                    response
                }
                Err(e) => tlv::error(6, e),
            }
        }
        _ => tlv::error(2, Error::Unknown),
    }
}

/// M5 → M6: the controller's long-term key for ours, both signed.
fn exchange(shared: &Shared, srp_key: &[u8], items: &Items) -> Result<Vec<u8>, Error> {
    let key = crypto::hkdf(srp_key, "Pair-Setup-Encrypt-Salt", "Pair-Setup-Encrypt-Info");
    let sealed = items.get(&tlv::ENCRYPTED_DATA).ok_or(Error::Unknown)?;
    let plain = crypto::open(&key, crypto::label_nonce(b"PS-Msg05"), &[], sealed).ok_or(Error::Authentication)?;
    let sub = tlv::decode(&plain).ok_or(Error::Unknown)?;
    let (Some(id), Some(ltpk), Some(signature)) =
        (sub.get(&tlv::IDENTIFIER), sub.get(&tlv::PUBLIC_KEY), sub.get(&tlv::SIGNATURE))
    else {
        return Err(Error::Unknown);
    };
    let x = crypto::hkdf(srp_key, "Pair-Setup-Controller-Sign-Salt", "Pair-Setup-Controller-Sign-Info");
    if !signed_by(ltpk, &[&x[..], id, ltpk].concat(), signature) {
        return Err(Error::Authentication);
    }

    let mut store = shared.store();
    store.add(Pairing::new(id, ltpk, true))?;
    info!("HomeKit: paired with {}", String::from_utf8_lossy(id));
    let x = crypto::hkdf(srp_key, "Pair-Setup-Accessory-Sign-Salt", "Pair-Setup-Accessory-Sign-Info");
    let device_id = store.device_id().as_bytes().to_vec();
    let public_key = store.public_key().to_vec();
    let signature = store.sign(&[&x[..], &device_id, &public_key].concat());
    let sub =
        tlv::encode(&[(tlv::IDENTIFIER, &device_id), (tlv::PUBLIC_KEY, &public_key), (tlv::SIGNATURE, &signature)]);
    let sealed = crypto::seal(&key, crypto::label_nonce(b"PS-Msg06"), &[], &sub);
    Ok(tlv::encode(&[(tlv::STATE, &[6]), (tlv::ENCRYPTED_DATA, &sealed)]))
}

/// `POST /pair-verify`. After M4, the connection continues encrypted as the
/// returned session, on behalf of the returned controller.
pub fn verify(shared: &Shared, verify: &mut Option<Verify>, body: &[u8]) -> (Vec<u8>, Option<(Session, String)>) {
    let Some(items) = tlv::decode(body) else { return (tlv::error(2, Error::Unknown), None) };
    match state(&items) {
        1 => match start_verify(shared, &items) {
            Ok((response, state)) => {
                *verify = Some(state);
                (response, None)
            }
            Err(e) => (tlv::error(2, e), None),
        },
        3 => {
            let Some(state) = verify.take() else { return (tlv::error(4, Error::Authentication), None) };
            match finish_verify(shared, &state, &items) {
                Ok(controller) => {
                    (tlv::encode(&[(tlv::STATE, &[4])]), Some((Session::new(&state.shared_secret), controller)))
                }
                Err(e) => (tlv::error(4, e), None),
            }
        }
        _ => (tlv::error(2, Error::Unknown), None),
    }
}

fn start_verify(shared: &Shared, items: &Items) -> Result<(Vec<u8>, Verify), Error> {
    let controller_key = items.get(&tlv::PUBLIC_KEY).ok_or(Error::Unknown)?.clone();
    let private = EphemeralPrivateKey::generate(&X25519, &SystemRandom::new()).map_err(|_| Error::Unknown)?;
    let accessory_key = private.compute_public_key().map_err(|_| Error::Unknown)?.as_ref().to_vec();
    let shared_secret = agreement::agree_ephemeral(
        private,
        &agreement::UnparsedPublicKey::new(&X25519, &controller_key),
        <[u8]>::to_vec,
    )
    .map_err(|_| Error::Authentication)?;

    let store = shared.store();
    let device_id = store.device_id().as_bytes();
    let signature = store.sign(&[&accessory_key[..], device_id, &controller_key].concat());
    let key = crypto::hkdf(&shared_secret, "Pair-Verify-Encrypt-Salt", "Pair-Verify-Encrypt-Info");
    let sub = tlv::encode(&[(tlv::IDENTIFIER, device_id), (tlv::SIGNATURE, &signature)]);
    let sealed = crypto::seal(&key, crypto::label_nonce(b"PV-Msg02"), &[], &sub);
    let response =
        tlv::encode(&[(tlv::STATE, &[2]), (tlv::PUBLIC_KEY, &accessory_key), (tlv::ENCRYPTED_DATA, &sealed)]);
    Ok((response, Verify { shared_secret, accessory_key, controller_key, key }))
}

fn finish_verify(shared: &Shared, state: &Verify, items: &Items) -> Result<String, Error> {
    let sealed = items.get(&tlv::ENCRYPTED_DATA).ok_or(Error::Authentication)?;
    let plain = crypto::open(&state.key, crypto::label_nonce(b"PV-Msg03"), &[], sealed).ok_or(Error::Authentication)?;
    let sub = tlv::decode(&plain).ok_or(Error::Authentication)?;
    let (Some(id), Some(signature)) = (sub.get(&tlv::IDENTIFIER), sub.get(&tlv::SIGNATURE)) else {
        return Err(Error::Authentication);
    };
    let id = String::from_utf8_lossy(id).into_owned();
    let pairing = shared.store().pairing(&id).cloned().ok_or(Error::Authentication)?;
    let message = [&state.controller_key[..], id.as_bytes(), &state.accessory_key].concat();
    if !signed_by(&pairing.public_key(), &message, signature) {
        return Err(Error::Authentication);
    }
    Ok(id)
}

/// `POST /pairings`, on a verified connection of `controller`.
pub fn manage(shared: &Shared, controller: &str, body: &[u8]) -> Vec<u8> {
    let Some(items) = tlv::decode(body) else { return tlv::error(2, Error::Unknown) };
    let mut store = shared.store();
    if !store.pairing(controller).is_some_and(|p| p.admin) {
        return tlv::error(2, Error::Authentication);
    }
    let method = items.get(&tlv::METHOD).and_then(|m| m.first().copied());
    let id = items.get(&tlv::IDENTIFIER).map(|id| String::from_utf8_lossy(id).into_owned());
    let result = match (method, id) {
        // Add.
        (Some(3), Some(id)) => {
            let Some(public_key) = items.get(&tlv::PUBLIC_KEY) else { return tlv::error(2, Error::Unknown) };
            let admin = items.get(&tlv::PERMISSIONS).is_some_and(|p| p.first() == Some(&1));
            store
                .add(Pairing::new(id.as_bytes(), public_key, admin))
                .map(|()| info!("HomeKit: {controller} added {id}"))
        }
        // Remove.
        (Some(4), Some(id)) => {
            store.remove(&id).map(|()| info!("HomeKit: {controller} removed {id}")).map_err(|_| Error::Unknown)
        }
        // List.
        (Some(5), _) => {
            let pairings: Vec<(String, Vec<u8>, u8)> =
                store.pairings().iter().map(|p| (p.id.clone(), p.public_key(), u8::from(p.admin))).collect();
            let mut items: Vec<(u8, &[u8])> = vec![(tlv::STATE, &[2])];
            for (i, (id, public_key, admin)) in pairings.iter().enumerate() {
                if i > 0 {
                    items.push((tlv::SEPARATOR, &[]));
                }
                items.extend([
                    (tlv::IDENTIFIER, id.as_bytes()),
                    (tlv::PUBLIC_KEY, public_key.as_slice()),
                    (tlv::PERMISSIONS, std::slice::from_ref(admin)),
                ]);
            }
            return tlv::encode(&items);
        }
        _ => Err(Error::Unknown),
    };
    drop(store);
    match result {
        Ok(()) => {
            shared.pairings_changed();
            tlv::encode(&[(tlv::STATE, &[2])])
        }
        Err(e) => tlv::error(2, e),
    }
}
