//! TLV8, the type-length-value encoding of HAP's pairing messages. A value
//! longer than 255 bytes is split into consecutive items of the same type.

use std::collections::BTreeMap;

pub const METHOD: u8 = 0x00;
pub const IDENTIFIER: u8 = 0x01;
pub const SALT: u8 = 0x02;
pub const PUBLIC_KEY: u8 = 0x03;
pub const PROOF: u8 = 0x04;
pub const ENCRYPTED_DATA: u8 = 0x05;
pub const STATE: u8 = 0x06;
pub const ERROR: u8 = 0x07;
pub const SIGNATURE: u8 = 0x0a;
pub const PERMISSIONS: u8 = 0x0b;
pub const SEPARATOR: u8 = 0xff;

/// Values of the `ERROR` item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Error {
    Unknown = 1,
    Authentication = 2,
    MaxPeers = 4,
    MaxTries = 5,
    Unavailable = 6,
    Busy = 7,
}

pub fn encode(items: &[(u8, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    for &(kind, value) in items {
        if value.is_empty() {
            out.extend([kind, 0]);
        }
        for chunk in value.chunks(255) {
            out.extend([kind, chunk.len() as u8]);
            out.extend_from_slice(chunk);
        }
    }
    out
}

/// Decodes a message whose types each appear once (fragments joined), as
/// every request HAP sends to an accessory does. `None` if it is truncated.
pub fn decode(data: &[u8]) -> Option<BTreeMap<u8, Vec<u8>>> {
    let mut items: BTreeMap<u8, Vec<u8>> = BTreeMap::new();
    let mut rest = data;
    while let [kind, len, tail @ ..] = rest {
        let value = tail.get(..*len as usize)?;
        items.entry(*kind).or_default().extend_from_slice(value);
        rest = &tail[*len as usize..];
    }
    rest.is_empty().then_some(items)
}

/// A pairing response that only reports `error` at `state`.
pub fn error(state: u8, error: Error) -> Vec<u8> {
    encode(&[(STATE, &[state]), (ERROR, &[error as u8])])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_long_values() {
        let key = vec![7u8; 384];
        let encoded = encode(&[(STATE, &[2]), (PUBLIC_KEY, &key), (SALT, &[1, 2, 3])]);
        // 384 bytes take two items: 255 + 129.
        assert_eq!(encoded.len(), 3 + (2 + 255) + (2 + 129) + 5);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded[&STATE], [2]);
        assert_eq!(decoded[&PUBLIC_KEY], key);
        assert_eq!(decoded[&SALT], [1, 2, 3]);
    }

    #[test]
    fn rejects_truncated_input() {
        assert!(decode(&[STATE, 2, 1]).is_none());
        assert!(decode(&[STATE]).is_none());
        assert_eq!(decode(&[]).unwrap().len(), 0);
    }
}
