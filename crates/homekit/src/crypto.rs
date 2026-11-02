//! HAP's primitives, all from ring: HKDF-SHA512, ChaCha20-Poly1305,
//! Ed25519 and X25519.

use ring::aead::{Aad, CHACHA20_POLY1305, LessSafeKey, Nonce, UnboundKey};
use ring::hkdf::{HKDF_SHA512, KeyType, Salt};
use ring::rand::{SecureRandom, SystemRandom};

pub type Key = [u8; 32];

struct Len32;

impl KeyType for Len32 {
    fn len(&self) -> usize {
        32
    }
}

pub fn hkdf(ikm: &[u8], salt: &str, info: &str) -> Key {
    let mut out = [0; 32];
    Salt::new(HKDF_SHA512, salt.as_bytes())
        .extract(ikm)
        .expand(&[info.as_bytes()], Len32)
        .and_then(|okm| okm.fill(&mut out))
        .expect("HKDF-SHA512 can produce 32 bytes");
    out
}

pub fn random<const N: usize>() -> [u8; N] {
    let mut out = [0; N];
    SystemRandom::new().fill(&mut out).expect("system random number generator");
    out
}

fn cipher(key: &Key) -> LessSafeKey {
    LessSafeKey::new(UnboundKey::new(&CHACHA20_POLY1305, key).expect("32-byte key"))
}

/// A pairing message's nonce: four zero bytes, then its label (`PS-Msg05`).
pub fn label_nonce(label: &[u8; 8]) -> [u8; 12] {
    let mut nonce = [0; 12];
    nonce[4..].copy_from_slice(label);
    nonce
}

/// A session frame's nonce: four zero bytes, then the little-endian counter.
pub fn counter_nonce(counter: u64) -> [u8; 12] {
    let mut nonce = [0; 12];
    nonce[4..].copy_from_slice(&counter.to_le_bytes());
    nonce
}

/// Ciphertext followed by the 16-byte tag.
pub fn seal(key: &Key, nonce: [u8; 12], aad: &[u8], plaintext: &[u8]) -> Vec<u8> {
    let mut data = plaintext.to_vec();
    cipher(key)
        .seal_in_place_append_tag(Nonce::assume_unique_for_key(nonce), Aad::from(aad), &mut data)
        .expect("ChaCha20-Poly1305 seals any message HAP sends");
    data
}

/// `None` if the tag doesn't match.
pub fn open(key: &Key, nonce: [u8; 12], aad: &[u8], sealed: &[u8]) -> Option<Vec<u8>> {
    let mut data = sealed.to_vec();
    let len = cipher(key).open_in_place(Nonce::assume_unique_for_key(nonce), Aad::from(aad), &mut data).ok()?.len();
    data.truncate(len);
    Some(data)
}
