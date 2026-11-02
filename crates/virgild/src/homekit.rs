//! Virgil as a HomeKit switch: on while anything plays (the library, radio or
//! an external source such as AirPlay), so "Hey Siri, turn off Virgil" stops
//! it and "turn on Virgil" resumes.

use std::time::Duration;

use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{mpsc, watch};
use tokio::time::Instant;
use tracing::warn;
use virgil_core::{HomekitStatus, LOCAL_SOURCE, PlayState, ServerEvent};
use virgil_homekit::{Status, Switch, Write};

use crate::app::App;
use crate::config::Config;

/// A hand-over to AirPlay reports "stopped" and "AirPlay plays" separately:
/// let the state settle, or the Home tile flickers off and on.
const SETTLE: Duration = Duration::from_millis(300);
/// After a write, look again once playback had time to follow (playing an
/// empty queue doesn't), so the tile shows what really happened.
const CONFIRM: Duration = Duration::from_secs(1);

/// Starts the accessory if the config asks for it. HomeKit is a convenience:
/// if it can't start (its port is taken), Virgil runs without it.
pub async fn start(config: &Config) -> Option<(Switch, mpsc::Receiver<Write>)> {
    let homekit = config.homekit.as_ref()?;
    let started = Switch::start(virgil_homekit::Config {
        name: homekit.name.clone(),
        port: homekit.port,
        store: config.data_dir.join("homekit.json"),
        manufacturer: "Virgil".into(),
        model: "Virgil".into(),
        firmware: env!("CARGO_PKG_VERSION").into(),
    })
    .await;
    started.inspect_err(|e| warn!("{e:#}; carrying on without HomeKit")).ok()
}

fn is_on(state: PlayState, source: &str) -> bool {
    state == PlayState::Play || source != LOCAL_SOURCE
}

/// Keeps the switch in step with playback and carries out what controllers ask.
pub async fn run(app: App, mut writes: mpsc::Receiver<Write>) {
    let Some(switch) = app.homekit.clone() else { return };
    tokio::spawn(publish_status(app.clone(), switch.status()));
    let current = || is_on(app.player().state, &app.source().active);
    switch.set_on(current());
    let mut events = app.events.subscribe();
    let mut check: Option<Instant> = None;
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Ok(ServerEvent::Player(_) | ServerEvent::Source(_)) | Err(RecvError::Lagged(_)) => check_in(&mut check, SETTLE),
                Ok(_) => {}
                Err(RecvError::Closed) => return,
            },
            () = tokio::time::sleep_until(check.unwrap_or_else(Instant::now)), if check.is_some() => {
                check = None;
                switch.set_on(current());
            }
            Some(write) = writes.recv() => {
                let result = if write.on { app.play().await } else { app.turn_off().await };
                if let Err(e) = &result {
                    warn!("HomeKit: turning {} failed: {e:?}", if write.on { "on" } else { "off" });
                }
                let _ = write.done.send(result.is_ok());
                check_in(&mut check, CONFIRM);
            }
        }
    }
}

/// Schedules a look at playback `delay` from now, unless one is due later.
fn check_in(check: &mut Option<Instant>, delay: Duration) {
    let at = Instant::now() + delay;
    *check = Some(check.map_or(at, |c| c.max(at)));
}

/// Mirrors the accessory's pairing state into the snapshot, for Settings.
async fn publish_status(app: App, mut status: watch::Receiver<Status>) {
    loop {
        let Status { paired, advertised, setup_code, setup_uri } = status.borrow_and_update().clone();
        // The code is only of use (and only shown) while the switch can be added.
        let unpaired = |value: String| (!paired).then_some(value);
        app.set_homekit(HomekitStatus {
            paired,
            advertised,
            setup_code: unpaired(setup_code),
            setup_uri: unpaired(setup_uri),
        });
        if status.changed().await.is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_while_anything_plays() {
        assert!(is_on(PlayState::Play, LOCAL_SOURCE));
        assert!(!is_on(PlayState::Pause, LOCAL_SOURCE));
        assert!(!is_on(PlayState::Stop, LOCAL_SOURCE));
        // AirPlay holds the DAC: MPD is stopped, but music plays.
        assert!(is_on(PlayState::Stop, "airplay"));
    }
}
