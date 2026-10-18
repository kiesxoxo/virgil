use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use tokio::sync::broadcast::error::RecvError;
use virgil_core::ServerEvent;

use crate::app::App;

pub async fn upgrade(ws: WebSocketUpgrade, State(app): State<App>) -> Response {
    ws.on_upgrade(move |socket| session(socket, app))
}

/// Sends the full state, then every change. A client that falls behind gets
/// the full state again instead of a gap.
async fn session(mut socket: WebSocket, app: App) {
    let mut events = app.events.subscribe();
    if send(&mut socket, &app.hello_live().await).await.is_err() {
        return;
    }
    loop {
        tokio::select! {
            event = events.recv() => {
                let event = match event {
                    Ok(event) => event,
                    Err(RecvError::Lagged(_)) => app.hello_live().await,
                    Err(RecvError::Closed) => return,
                };
                if send(&mut socket, &event).await.is_err() {
                    return;
                }
            }
            message = socket.recv() => match message {
                Some(Ok(Message::Close(_)) | Err(_)) | None => return,
                // Commands go through the REST API; pings are answered by axum.
                Some(Ok(_)) => {}
            }
        }
    }
}

async fn send(socket: &mut WebSocket, event: &ServerEvent) -> Result<(), axum::Error> {
    let json = serde_json::to_string(event).expect("events serialize");
    socket.send(Message::Text(json.into())).await
}
