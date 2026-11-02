//! The one accessory: Accessory Information, Protocol Information and a
//! Switch. Instance ids are fixed, so controllers' caches stay valid.

use serde_json::{Value, json};

use crate::Shared;

pub const IDENTIFY: u64 = 2;
pub const ON: u64 = 11;

/// HAP status codes for characteristic requests.
pub mod status {
    pub const COMMUNICATION_FAILURE: i64 = -70402;
    pub const READ_ONLY: i64 = -70404;
    pub const WRITE_ONLY: i64 = -70405;
    pub const NO_NOTIFICATION: i64 = -70406;
    pub const NOT_FOUND: i64 = -70409;
    pub const INVALID_VALUE: i64 = -70410;
}

struct Characteristic {
    iid: u64,
    kind: &'static str,
    perms: &'static [&'static str],
    format: &'static str,
    /// None for write-only characteristics.
    value: Option<Value>,
}

struct Service {
    iid: u64,
    kind: &'static str,
    primary: bool,
    characteristics: Vec<Characteristic>,
}

fn text(iid: u64, kind: &'static str, value: &str) -> Characteristic {
    Characteristic { iid, kind, perms: &["pr"], format: "string", value: Some(json!(value)) }
}

fn services(shared: &Shared) -> Vec<Service> {
    let config = &shared.config;
    let serial = shared.store().device_id().to_string();
    vec![
        Service {
            iid: 1,
            kind: "3E",
            primary: false,
            characteristics: vec![
                Characteristic { iid: IDENTIFY, kind: "14", perms: &["pw"], format: "bool", value: None },
                text(3, "20", &config.manufacturer),
                text(4, "21", &config.model),
                text(5, "23", &config.name),
                text(6, "30", &serial),
                text(7, "52", &config.firmware),
            ],
        },
        Service { iid: 8, kind: "A2", primary: false, characteristics: vec![text(9, "37", "1.1.0")] },
        Service {
            iid: 10,
            kind: "49",
            primary: true,
            characteristics: vec![
                Characteristic {
                    iid: ON,
                    kind: "25",
                    perms: &["pr", "pw", "ev"],
                    format: "bool",
                    value: Some(json!(*shared.on.borrow())),
                },
                text(12, "23", &config.name),
            ],
        },
    ]
}

/// What changes `c#` when it changes. Bump the version when the services do.
pub fn signature(name: &str) -> String {
    format!("switch-1/{name}")
}

/// The body of `GET /accessories`.
pub fn database(shared: &Shared) -> Value {
    let services: Vec<Value> = services(shared)
        .into_iter()
        .map(|s| {
            let characteristics: Vec<Value> = s
                .characteristics
                .into_iter()
                .map(|c| {
                    let mut value = json!({ "iid": c.iid, "type": c.kind, "perms": c.perms, "format": c.format });
                    if let Some(v) = c.value {
                        value["value"] = v;
                    }
                    value
                })
                .collect();
            json!({ "iid": s.iid, "type": s.kind, "primary": s.primary, "characteristics": characteristics })
        })
        .collect();
    json!({ "accessories": [{ "aid": 1, "services": services }] })
}

pub fn read(shared: &Shared, iid: u64) -> Result<Value, i64> {
    let characteristic =
        services(shared).into_iter().flat_map(|s| s.characteristics).find(|c| c.iid == iid).ok_or(status::NOT_FOUND)?;
    characteristic.value.ok_or(status::WRITE_ONLY)
}

/// Whether the accessory has a characteristic `iid`.
pub fn exists(shared: &Shared, iid: u64) -> bool {
    services(shared).iter().flat_map(|s| &s.characteristics).any(|c| c.iid == iid)
}
