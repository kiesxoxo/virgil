//! A controller doing what iOS does, over a real connection: pair-setup,
//! pair-verify, then reading, writing and subscribing to the switch.

use std::time::Duration;

use ring::agreement::{self, EphemeralPrivateKey, X25519};
use ring::rand::SystemRandom;
use ring::signature::{ED25519, Ed25519KeyPair, KeyPair, UnparsedPublicKey};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::server::parse;
use crate::session::Session;
use crate::{Config, Switch, crypto, srp, tlv};

struct Client {
    stream: TcpStream,
    session: Option<Session>,
    buf: Vec<u8>,
}

impl Client {
    async fn connect(port: u16) -> Self {
        Self { stream: TcpStream::connect(("127.0.0.1", port)).await.unwrap(), session: None, buf: Vec::new() }
    }

    async fn send(&mut self, method: &str, path: &str, body: &[u8]) {
        let head = format!("{method} {path} HTTP/1.1\r\nHost: test\r\nContent-Length: {}\r\n\r\n", body.len());
        let bytes = [head.as_bytes(), body].concat();
        let bytes = match &mut self.session {
            Some(session) => session.encrypt(&bytes),
            None => bytes,
        };
        self.stream.write_all(&bytes).await.unwrap();
    }

    /// The next response or event: its first line's first word, status and body.
    async fn receive(&mut self) -> (String, u16, Vec<u8>) {
        let mut plain = Vec::new();
        loop {
            match &mut self.session {
                Some(session) => session.decrypt(&mut self.buf, &mut plain).unwrap(),
                None => plain.append(&mut self.buf),
            }
            // The request parser reads `HTTP/1.1 200 OK` as method and path.
            if let Some((message, used)) = parse(&plain).unwrap() {
                assert_eq!(used, plain.len());
                return (message.method, message.path.parse().unwrap(), message.body);
            }
            let read = tokio::time::timeout(Duration::from_secs(5), self.stream.read_buf(&mut self.buf)).await;
            assert!(read.unwrap().unwrap() > 0, "connection closed");
        }
    }

    async fn call(&mut self, method: &str, path: &str, body: &[u8]) -> (u16, Vec<u8>) {
        self.send(method, path, body).await;
        let (_, status, body) = self.receive().await;
        (status, body)
    }

    async fn tlv(&mut self, path: &str, items: &[(u8, &[u8])]) -> std::collections::BTreeMap<u8, Vec<u8>> {
        let (status, body) = self.call("POST", path, &tlv::encode(items)).await;
        assert_eq!(status, 200);
        let items = tlv::decode(&body).unwrap();
        assert!(!items.contains_key(&tlv::ERROR), "{path} failed: {items:?}");
        items
    }
}

const CONTROLLER: &[u8] = b"8D1B5B3E-0D0A-4C4A-9C4B-6B7C3B1E2F00";

/// Pair-setup M1–M6; returns the accessory's long-term public key.
async fn pair(client: &mut Client, code: &str, keys: &Ed25519KeyPair) -> Vec<u8> {
    let m2 = client.tlv("/pair-setup", &[(tlv::STATE, &[1]), (tlv::METHOD, &[0])]).await;
    let (salt, b_pub) = (&m2[&tlv::SALT], &m2[&tlv::PUBLIC_KEY]);
    let (a_pub, proof, srp_key) = srp::tests::client(salt, code.as_bytes(), &[0x17; 32], b_pub);
    let m4 = client.tlv("/pair-setup", &[(tlv::STATE, &[3]), (tlv::PUBLIC_KEY, &a_pub), (tlv::PROOF, &proof)]).await;
    assert_eq!(m4[&tlv::STATE], [4]);

    let key = crypto::hkdf(&srp_key, "Pair-Setup-Encrypt-Salt", "Pair-Setup-Encrypt-Info");
    let x = crypto::hkdf(&srp_key, "Pair-Setup-Controller-Sign-Salt", "Pair-Setup-Controller-Sign-Info");
    let ltpk = keys.public_key().as_ref();
    let signature = keys.sign(&[&x[..], CONTROLLER, ltpk].concat());
    let sub =
        tlv::encode(&[(tlv::IDENTIFIER, CONTROLLER), (tlv::PUBLIC_KEY, ltpk), (tlv::SIGNATURE, signature.as_ref())]);
    let sealed = crypto::seal(&key, crypto::label_nonce(b"PS-Msg05"), &[], &sub);
    let m6 = client.tlv("/pair-setup", &[(tlv::STATE, &[5]), (tlv::ENCRYPTED_DATA, &sealed)]).await;

    let plain = crypto::open(&key, crypto::label_nonce(b"PS-Msg06"), &[], &m6[&tlv::ENCRYPTED_DATA]).unwrap();
    let sub = tlv::decode(&plain).unwrap();
    let x = crypto::hkdf(&srp_key, "Pair-Setup-Accessory-Sign-Salt", "Pair-Setup-Accessory-Sign-Info");
    let (id, accessory_ltpk) = (&sub[&tlv::IDENTIFIER], &sub[&tlv::PUBLIC_KEY]);
    UnparsedPublicKey::new(&ED25519, accessory_ltpk)
        .verify(&[&x[..], id, accessory_ltpk].concat(), &sub[&tlv::SIGNATURE])
        .expect("accessory's signature");
    accessory_ltpk.clone()
}

/// Pair-verify M1–M4; the connection is encrypted afterwards.
async fn verify(client: &mut Client, keys: &Ed25519KeyPair, accessory_ltpk: &[u8]) {
    let private = EphemeralPrivateKey::generate(&X25519, &SystemRandom::new()).unwrap();
    let public = private.compute_public_key().unwrap().as_ref().to_vec();
    let m2 = client.tlv("/pair-verify", &[(tlv::STATE, &[1]), (tlv::PUBLIC_KEY, &public)]).await;
    let accessory_key = m2[&tlv::PUBLIC_KEY].clone();
    let shared = agreement::agree_ephemeral(
        private,
        &agreement::UnparsedPublicKey::new(&X25519, &accessory_key),
        <[u8]>::to_vec,
    )
    .unwrap();
    let key = crypto::hkdf(&shared, "Pair-Verify-Encrypt-Salt", "Pair-Verify-Encrypt-Info");
    let plain = crypto::open(&key, crypto::label_nonce(b"PV-Msg02"), &[], &m2[&tlv::ENCRYPTED_DATA]).unwrap();
    let sub = tlv::decode(&plain).unwrap();
    UnparsedPublicKey::new(&ED25519, accessory_ltpk)
        .verify(&[&accessory_key[..], &sub[&tlv::IDENTIFIER], &public].concat(), &sub[&tlv::SIGNATURE])
        .expect("accessory's signature");

    let signature = keys.sign(&[&public[..], CONTROLLER, &accessory_key].concat());
    let sub = tlv::encode(&[(tlv::IDENTIFIER, CONTROLLER), (tlv::SIGNATURE, signature.as_ref())]);
    let sealed = crypto::seal(&key, crypto::label_nonce(b"PV-Msg03"), &[], &sub);
    client.tlv("/pair-verify", &[(tlv::STATE, &[3]), (tlv::ENCRYPTED_DATA, &sealed)]).await;
    client.session = Some(Session::controller(&shared));
}

fn json(body: &[u8]) -> Value {
    serde_json::from_slice(body).unwrap()
}

#[tokio::test]
async fn pair_verify_and_control_the_switch() {
    let dir = std::env::temp_dir().join(format!("virgil-homekit-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let config = Config {
        name: "Virgil".into(),
        port,
        store: dir.join("homekit.json"),
        manufacturer: "Virgil".into(),
        model: "Virgil".into(),
        firmware: "0.9.0".into(),
    };
    let (switch, mut writes) = Switch::start(config).await.unwrap();
    let status = switch.status().borrow().clone();
    assert!(!status.paired);

    // Not yet verified: only pairing is allowed.
    let mut stranger = Client::connect(port).await;
    assert_eq!(stranger.call("GET", "/accessories", b"").await.0, 470);

    let keys = Ed25519KeyPair::from_seed_unchecked(&[5; 32]).unwrap();
    let mut client = Client::connect(port).await;
    let wrong = client.tlv("/pair-setup", &[(tlv::STATE, &[1]), (tlv::METHOD, &[0])]).await;
    let (a_pub, proof, _) = srp::tests::client(&wrong[&tlv::SALT], b"000-00-001", &[1; 32], &wrong[&tlv::PUBLIC_KEY]);
    let (_, body) = client
        .call(
            "POST",
            "/pair-setup",
            &tlv::encode(&[(tlv::STATE, &[3]), (tlv::PUBLIC_KEY, &a_pub), (tlv::PROOF, &proof)]),
        )
        .await;
    assert_eq!(tlv::decode(&body).unwrap()[&tlv::ERROR], [tlv::Error::Authentication as u8]);

    let accessory_ltpk = pair(&mut client, &status.setup_code, &keys).await;
    assert!(switch.status().borrow().paired);
    // Paired: nobody else can pair now.
    let (_, body) =
        stranger.call("POST", "/pair-setup", &tlv::encode(&[(tlv::STATE, &[1]), (tlv::METHOD, &[0])])).await;
    assert_eq!(tlv::decode(&body).unwrap()[&tlv::ERROR], [tlv::Error::Unavailable as u8]);

    let mut client = Client::connect(port).await;
    verify(&mut client, &keys, &accessory_ltpk).await;
    let (status, body) = client.call("GET", "/accessories", b"").await;
    assert_eq!(status, 200);
    assert_eq!(json(&body)["accessories"][0]["services"][2]["characteristics"][0]["value"], false);

    let (status, _) =
        client.call("PUT", "/characteristics", br#"{"characteristics":[{"aid":1,"iid":11,"ev":true}]}"#).await;
    assert_eq!(status, 204);

    // Turning it on waits for Virgil's answer.
    let virgil = tokio::spawn(async move {
        let write = writes.recv().await.unwrap();
        assert!(write.on);
        write.done.send(true).unwrap();
        writes
    });
    let (status, _) =
        client.call("PUT", "/characteristics", br#"{"characteristics":[{"aid":1,"iid":11,"value":1}]}"#).await;
    assert_eq!(status, 204);
    let mut writes = virgil.await.unwrap();
    let (status, body) = client.call("GET", "/characteristics?id=1.11,1.5", b"").await;
    assert_eq!(status, 200);
    assert_eq!(json(&body)["characteristics"][0]["value"], true);
    assert_eq!(json(&body)["characteristics"][1]["value"], "Virgil");

    // Playback stopped some other way: the subscribed controller hears of it.
    switch.set_on(false);
    let (kind, _, body) = client.receive().await;
    assert_eq!(kind, "EVENT/1.0");
    assert_eq!(json(&body)["characteristics"][0], serde_json::json!({ "aid": 1, "iid": 11, "value": false }));

    // A write Virgil can't carry out is reported, not shown as done.
    let virgil = tokio::spawn(async move { writes.recv().await.unwrap().done.send(false).unwrap() });
    let (status, body) =
        client.call("PUT", "/characteristics", br#"{"characteristics":[{"aid":1,"iid":11,"value":true}]}"#).await;
    virgil.await.unwrap();
    assert_eq!(status, 207);
    assert_eq!(json(&body)["characteristics"][0]["status"], -70402);

    // The Home app adds the home hub, then lists the pairings.
    let hub = Ed25519KeyPair::from_seed_unchecked(&[6; 32]).unwrap();
    let added = [
        (tlv::STATE, &[1][..]),
        (tlv::METHOD, &[3]),
        (tlv::IDENTIFIER, b"hub"),
        (tlv::PUBLIC_KEY, hub.public_key().as_ref()),
        (tlv::PERMISSIONS, &[1]),
    ];
    client.tlv("/pairings", &added).await;
    let (status, body) =
        client.call("POST", "/pairings", &tlv::encode(&[(tlv::STATE, &[1]), (tlv::METHOD, &[5])])).await;
    assert_eq!(status, 200);
    let (mut ids, mut rest) = (Vec::new(), body.as_slice());
    while let [kind, len, tail @ ..] = rest {
        if *kind == tlv::IDENTIFIER {
            ids.push(&tail[..*len as usize]);
        }
        rest = &tail[*len as usize..];
    }
    assert_eq!(ids, [CONTROLLER, b"hub"]);

    // Removed from the Home app: the controller removes itself and the hub.
    client.tlv("/pairings", &[(tlv::STATE, &[1]), (tlv::METHOD, &[4]), (tlv::IDENTIFIER, b"hub")]).await;
    client.tlv("/pairings", &[(tlv::STATE, &[1]), (tlv::METHOD, &[4]), (tlv::IDENTIFIER, CONTROLLER)]).await;
    assert!(!switch.status().borrow().paired);
    let mut buf = [0; 16];
    let closed = tokio::time::timeout(Duration::from_secs(5), client.stream.read(&mut buf)).await.unwrap().unwrap();
    assert_eq!(closed, 0, "a removed controller's connection is closed");
}
