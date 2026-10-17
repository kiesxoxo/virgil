//! Keeps the snapshot in step with MPD and the library.

use hifi_core::{PlayerState, ServerEvent};
use hifi_engine_mpd::Event;

use crate::app::App;

pub async fn run_mpd(app: App) {
    let mut events = app.mpd.watch();
    while let Some(event) = events.recv().await {
        match event {
            Event::Connected { .. } => {
                app.refresh(true).await;
                app.refresh_outputs().await;
                app.library.request_sync();
            }
            Event::Disconnected => app.set_player(PlayerState::default()),
            Event::Changed(changed) => {
                let has = |name: &str| changed.iter().any(|c| c == name);
                if has("database") {
                    app.library.request_sync();
                }
                if has("output") {
                    app.refresh_outputs().await;
                }
                if ["player", "mixer", "options", "output", "playlist"].into_iter().any(has) {
                    app.refresh(has("playlist")).await;
                }
            }
        }
    }
}

pub async fn run_library(app: App) {
    let mut status = app.library.subscribe();
    let mut revision = status.borrow().revision;
    while status.changed().await.is_ok() {
        let current = status.borrow_and_update().clone();
        if current.revision != revision {
            revision = current.revision;
            app.reenrich_queue();
        }
        let _ = app.events.send(ServerEvent::Library(current));
    }
}
