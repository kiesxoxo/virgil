//! The encrypted connection after pair-verify: frames of at most 1024 bytes,
//! each a 2-byte little-endian length (also the AAD), the ciphertext and a
//! 16-byte tag, with a nonce counter per direction.

use crate::crypto::{self, Key};

const MAX_FRAME: usize = 1024;
const TAG: usize = 16;

pub struct Session {
    /// Controller to accessory.
    read_key: Key,
    /// Accessory to controller.
    write_key: Key,
    read_count: u64,
    write_count: u64,
}

impl Session {
    /// From pair-verify's shared secret.
    pub fn new(shared: &[u8]) -> Self {
        Self {
            read_key: crypto::hkdf(shared, "Control-Salt", "Control-Write-Encryption-Key"),
            write_key: crypto::hkdf(shared, "Control-Salt", "Control-Read-Encryption-Key"),
            read_count: 0,
            write_count: 0,
        }
    }

    pub fn encrypt(&mut self, plaintext: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(plaintext.len() + plaintext.len().div_ceil(MAX_FRAME) * (2 + TAG));
        for chunk in plaintext.chunks(MAX_FRAME) {
            let len = (chunk.len() as u16).to_le_bytes();
            out.extend(len);
            out.extend(crypto::seal(&self.write_key, crypto::counter_nonce(self.write_count), &len, chunk));
            self.write_count += 1;
        }
        out
    }

    /// Moves every complete frame from `buf` into `plaintext`. `Err` if one
    /// fails to authenticate: the connection must then be closed.
    pub fn decrypt(&mut self, buf: &mut Vec<u8>, plaintext: &mut Vec<u8>) -> Result<(), ()> {
        let mut at = 0;
        while let Some(len) = buf.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize) {
            if len > MAX_FRAME {
                return Err(());
            }
            let Some(sealed) = buf.get(at + 2..at + 2 + len + TAG) else { break };
            let frame = crypto::open(&self.read_key, crypto::counter_nonce(self.read_count), &buf[at..at + 2], sealed)
                .ok_or(())?;
            plaintext.extend(frame);
            self.read_count += 1;
            at += 2 + len + TAG;
        }
        buf.drain(..at);
        Ok(())
    }

    /// The same session as the controller sees it: the keys swapped.
    #[cfg(test)]
    pub fn controller(shared: &[u8]) -> Self {
        let accessory = Self::new(shared);
        Self { read_key: accessory.write_key, write_key: accessory.read_key, read_count: 0, write_count: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip_across_reads() {
        let mut accessory = Session::new(&[9; 32]);
        let mut ios = Session::controller(&[9; 32]);
        let request: Vec<u8> = (0..2500u32).map(|i| i as u8).collect();
        let wire = ios.encrypt(&request);
        assert_eq!(wire.len(), request.len() + 3 * (2 + TAG));

        // Arrives in two pieces, the first ending mid-frame.
        let (mut buf, mut plain) = (wire[..1500].to_vec(), Vec::new());
        accessory.decrypt(&mut buf, &mut plain).unwrap();
        assert_eq!(plain.len(), 1024);
        buf.extend(&wire[1500..]);
        accessory.decrypt(&mut buf, &mut plain).unwrap();
        assert_eq!(plain, request);
        assert!(buf.is_empty());

        let reply = accessory.encrypt(b"HTTP/1.1 204 No Content\r\n\r\n");
        let (mut buf, mut plain) = (reply, Vec::new());
        ios.decrypt(&mut buf, &mut plain).unwrap();
        assert_eq!(plain, b"HTTP/1.1 204 No Content\r\n\r\n");
    }

    #[test]
    fn tampering_is_detected() {
        let mut accessory = Session::new(&[9; 32]);
        let mut wire = Session::controller(&[9; 32]).encrypt(b"GET /accessories HTTP/1.1\r\n\r\n");
        wire[5] ^= 1;
        assert!(accessory.decrypt(&mut wire, &mut Vec::new()).is_err());
    }
}
