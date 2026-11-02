//! Advertises `_hap._tcp` through the host's avahi-daemon over D-Bus. A second
//! mDNS responder on the same host would contend with avahi for port 5353
//! and the host name. Registers again when avahi restarts, and takes avahi's
//! alternative name ("Virgil #2") if another accessory has the name.

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use tracing::{info, warn};
use zbus::proxy::{Builder, CacheProperties};
use zbus::zvariant::OwnedObjectPath;
use zbus::{Connection, Proxy};

use crate::Shared;

const AVAHI: &str = "org.freedesktop.Avahi";
const SERVICE_TYPE: &str = "_hap._tcp";
/// `AVAHI_IF_UNSPEC`, `AVAHI_PROTO_UNSPEC`: every interface, IPv4 and IPv6.
const ANY: i32 = -1;
/// The entry group's `StateChanged` values.
const ESTABLISHED: i32 = 2;
const COLLISION: i32 = 3;
const FAILURE: i32 = 4;

pub async fn run(shared: Arc<Shared>) {
    let mut warned = false;
    loop {
        let result = advertise(&shared).await;
        shared.set_advertised(false);
        match result {
            Ok(()) => info!("HomeKit: avahi went away; registering again"),
            Err(e) if !warned => {
                warn!("HomeKit: can't advertise through avahi-daemon (retrying): {e}");
                warned = true;
            }
            Err(_) => {}
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

async fn proxy<'a>(conn: &Connection, path: OwnedObjectPath, interface: &'static str) -> zbus::Result<Proxy<'a>> {
    Builder::new(conn)
        .destination(AVAHI)?
        .path(path)?
        .interface(interface)?
        .cache_properties(CacheProperties::No)
        .build()
        .await
}

/// Registers the service and keeps it current. Returns when avahi restarts.
async fn advertise(shared: &Shared) -> zbus::Result<()> {
    let conn = Connection::system().await?;
    let dbus = zbus::fdo::DBusProxy::new(&conn).await?;
    let mut owner_changes = dbus.receive_name_owner_changed_with_args(&[(0, AVAHI)]).await?;
    let server = proxy(&conn, OwnedObjectPath::try_from("/")?, "org.freedesktop.Avahi.Server").await?;
    let path: OwnedObjectPath = server.call("EntryGroupNew", &()).await?;
    let group = proxy(&conn, path, "org.freedesktop.Avahi.EntryGroup").await?;
    let mut states = group.receive_signal("StateChanged").await?;
    let mut pairings = shared.pairings.subscribe();

    let mut name = shared.config.name.clone();
    add(&group, &name, shared).await?;
    loop {
        tokio::select! {
            Some(message) = states.next() => {
                let (state, error): (i32, String) = message.body().deserialize()?;
                match state {
                    ESTABLISHED => {
                        info!("HomeKit: advertised as {name:?}");
                        shared.set_advertised(true);
                    }
                    COLLISION => {
                        let taken = std::mem::take(&mut name);
                        name = server.call("GetAlternativeServiceName", &(taken.as_str(),)).await?;
                        info!("HomeKit: {taken:?} is taken on the network, trying {name:?}");
                        group.call::<_, _, ()>("Reset", &()).await?;
                        add(&group, &name, shared).await?;
                    }
                    FAILURE => return Err(zbus::Error::Failure(error)),
                    _ => {}
                }
            }
            Ok(()) = pairings.changed() => {
                pairings.borrow_and_update();
                let txt = txt(shared);
                group.call::<_, _, ()>("UpdateServiceTxt", &(ANY, ANY, 0u32, name.as_str(), SERVICE_TYPE, "", txt)).await?;
            }
            Some(_) = owner_changes.next() => return Ok(()),
            else => return Ok(()),
        }
    }
}

async fn add(group: &Proxy<'_>, name: &str, shared: &Shared) -> zbus::Result<()> {
    let port = shared.config.port;
    group.call::<_, _, ()>("AddService", &(ANY, ANY, 0u32, name, SERVICE_TYPE, "", "", port, txt(shared))).await?;
    group.call::<_, _, ()>("Commit", &()).await
}

/// HAP's TXT record. `sf` (status flags) is 1 while the accessory can be paired.
fn txt(shared: &Shared) -> Vec<Vec<u8>> {
    let store = shared.store();
    [
        format!("c#={}", store.config_number()),
        "ff=0".into(),
        format!("id={}", store.device_id()),
        format!("md={}", shared.config.model),
        "pv=1.1".into(),
        "s#=1".into(),
        format!("sf={}", u8::from(!store.paired())),
        format!("ci={}", crate::CATEGORY_SWITCH),
        format!("sh={}", crate::setup_hash(store.setup_id(), store.device_id())),
    ]
    .into_iter()
    .map(String::into_bytes)
    .collect()
}
