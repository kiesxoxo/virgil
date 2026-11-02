//! The accessory side of SRP-6a as pair-setup uses it: the 3072-bit group
//! from RFC 5054, g = 5, SHA-512, user name `Pair-Setup`, the setup code as
//! password. Proofs follow RFC 2945 (M1 = H(H(N) xor H(g), H(I), s, A, B, K))
//! with K = H(S); padded only where SRP-6a says so (k and u).

use num_bigint::BigUint;
use ring::digest::{Context, SHA512};

const N_HEX: &[u8] = b"\
FFFFFFFFFFFFFFFFC90FDAA22168C234C4C6628B80DC1CD129024E088A67CC74020BBEA63B139B22514A08798E3404DD\
EF9519B3CD3A431B302B0A6DF25F14374FE1356D6D51C245E485B576625E7EC6F44C42E9A637ED6B0BFF5CB6F406B7ED\
EE386BFB5A899FA5AE9F24117C4B1FE649286651ECE45B3DC2007CB8A163BF0598DA48361C55D39A69163FA8FD24CF5F\
83655D23DCA3AD961C62F356208552BB9ED529077096966D670C354E4ABC9804F1746C08CA18217C32905E462E36CE3B\
E39E772C180E86039B2783A2EC07A28FB5C55DF06F4C52C9DE2BCBF6955817183995497CEA956AE515D2261898FA0510\
15728E5A8AAAC42DAD33170D04507A33A85521ABDF1CBA64ECFB850458DBEF0A8AEA71575D060C7DB3970F85A6E1E4C7\
ABF5AE8CDB0933D71E8C94E04A25619DCEE3D2261AD2EE6BF12FFA06D98A0864D87602733EC86A64521F2B18177B200C\
BBE117577A615D6C770988C0BAD946E208E24FA074E5AB3143DB5BFCE0FD108E4B82D120A93AD2CAFFFFFFFFFFFFFFFF";
const G: u32 = 5;
/// Bytes in N.
const LEN: usize = 384;
pub const USER: &[u8] = b"Pair-Setup";

fn n() -> BigUint {
    BigUint::parse_bytes(N_HEX, 16).expect("N is hex")
}

fn hash(parts: &[&[u8]]) -> Vec<u8> {
    let mut ctx = Context::new(&SHA512);
    for part in parts {
        ctx.update(part);
    }
    ctx.finish().as_ref().to_vec()
}

fn pad(x: &BigUint) -> Vec<u8> {
    let bytes = x.to_bytes_be();
    let mut out = vec![0; LEN.saturating_sub(bytes.len())];
    out.extend(bytes);
    out
}

fn int(bytes: &[u8]) -> BigUint {
    BigUint::from_bytes_be(bytes)
}

/// x = H(s, H(I ":" P)).
fn private_key(salt: &[u8], password: &[u8]) -> BigUint {
    int(&hash(&[salt, &hash(&[USER, b":", password])]))
}

/// k = H(N, PAD(g)).
fn multiplier(n: &BigUint) -> BigUint {
    int(&hash(&[&n.to_bytes_be(), &pad(&BigUint::from(G))]))
}

/// M1 = H(H(N) xor H(g), H(I), s, A, B, K).
fn client_proof(n: &BigUint, salt: &[u8], a_pub: &BigUint, b_pub: &BigUint, key: &[u8]) -> Vec<u8> {
    let h_n = hash(&[&n.to_bytes_be()]);
    let h_g = hash(&[&BigUint::from(G).to_bytes_be()]);
    let xor: Vec<u8> = h_n.iter().zip(&h_g).map(|(a, b)| a ^ b).collect();
    hash(&[&xor, &hash(&[USER]), salt, &a_pub.to_bytes_be(), &b_pub.to_bytes_be(), key])
}

/// After M1/M2: the server's ephemeral key pair and the verifier.
pub struct Server {
    n: BigUint,
    salt: Vec<u8>,
    verifier: BigUint,
    secret: BigUint,
    public: BigUint,
}

/// A controller that proved it knows the setup code.
pub struct Verified {
    /// K, the shared session key.
    pub key: Vec<u8>,
    /// M2, the accessory's proof for the controller.
    pub proof: Vec<u8>,
}

impl Server {
    /// `secret` is b: 32 random bytes.
    pub fn new(salt: &[u8], password: &[u8], secret: &[u8]) -> Self {
        let n = n();
        let verifier = BigUint::from(G).modpow(&private_key(salt, password), &n);
        let secret = int(secret);
        let public = (multiplier(&n) * &verifier + BigUint::from(G).modpow(&secret, &n)) % &n;
        Self { n, salt: salt.to_vec(), verifier, secret, public }
    }

    /// B, sent with the salt in M2.
    pub fn public_key(&self) -> Vec<u8> {
        self.public.to_bytes_be()
    }

    /// Checks the controller's A and M1 (from M3); `None` if the proof is wrong.
    pub fn verify(&self, a_pub: &[u8], proof: &[u8]) -> Option<Verified> {
        let a_pub = int(a_pub);
        if (&a_pub % &self.n) == BigUint::ZERO {
            return None;
        }
        let u = int(&hash(&[&pad(&a_pub), &pad(&self.public)]));
        let shared = (&a_pub * self.verifier.modpow(&u, &self.n)).modpow(&self.secret, &self.n);
        let key = hash(&[&shared.to_bytes_be()]);
        let expected = client_proof(&self.n, &self.salt, &a_pub, &self.public, &key);
        // Compared in constant time: how far a guess matched mustn't show.
        let difference = expected.iter().zip(proof).fold(0, |acc, (a, b)| acc | (a ^ b));
        if difference != 0 || proof.len() != expected.len() {
            return None;
        }
        let proof = hash(&[&a_pub.to_bytes_be(), &expected, &key]);
        Some(Verified { key, proof })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The controller's side, as iOS computes it: A, M1 and K.
    pub(crate) fn client(salt: &[u8], password: &[u8], a: &[u8], b_pub: &[u8]) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let n = n();
        let g = BigUint::from(G);
        let a = int(a);
        let a_pub = g.modpow(&a, &n);
        let b_pub = int(b_pub);
        let u = int(&hash(&[&pad(&a_pub), &pad(&b_pub)]));
        let x = private_key(salt, password);
        let kgx = (multiplier(&n) * g.modpow(&x, &n)) % &n;
        let base = (&b_pub + &n - kgx) % &n;
        let shared = base.modpow(&(a + u * x), &n);
        let key = hash(&[&shared.to_bytes_be()]);
        let proof = client_proof(&n, salt, &a_pub, &b_pub, &key);
        (a_pub.to_bytes_be(), proof, key)
    }

    #[test]
    fn controller_with_the_code_is_verified() {
        let salt = [0xbe; 16];
        let server = Server::new(&salt, b"482-91-736", &[0x42; 32]);
        let (a_pub, proof, key) = client(&salt, b"482-91-736", &[0x17; 32], &server.public_key());
        let verified = server.verify(&a_pub, &proof).expect("proof accepted");
        assert_eq!(verified.key, key);
        assert_eq!(verified.proof, hash(&[&a_pub, &proof, &key]));
    }

    #[test]
    fn wrong_code_is_rejected() {
        let salt = [0xbe; 16];
        let server = Server::new(&salt, b"482-91-736", &[0x42; 32]);
        let (a_pub, proof, _) = client(&salt, b"482-91-737", &[0x17; 32], &server.public_key());
        assert!(server.verify(&a_pub, &proof).is_none());
        assert!(server.verify(&n().to_bytes_be(), &proof).is_none());
    }
}
