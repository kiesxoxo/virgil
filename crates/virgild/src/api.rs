use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use virgil_core::{
    AddRequest, AlbumDetail, ArtOutcome, DesktopSettings, LibraryStatus, ModesRequest, MoveRequest, PlayState, Queue,
    SearchResults, SeekRequest, ServerEvent,
};

use crate::app::App;
use crate::{tags, web, ws};

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub fn not_found(message: impl Into<String>) -> Self {
        Self { status: StatusCode::NOT_FOUND, message: message.into() }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self { status: StatusCode::BAD_REQUEST, message: message.into() }
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self { status: StatusCode::CONFLICT, message: message.into() }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(serde_json::json!({ "error": self.message }))).into_response()
    }
}

impl From<virgil_engine_mpd::Error> for ApiError {
    fn from(e: virgil_engine_mpd::Error) -> Self {
        // An ACK is MPD refusing the request; anything else means MPD is unreachable.
        let status = if e.is_connection() { StatusCode::BAD_GATEWAY } else { StatusCode::CONFLICT };
        Self { status, message: e.to_string() }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        Self { status: StatusCode::INTERNAL_SERVER_ERROR, message: format!("{e:#}") }
    }
}

pub type ApiResult<T = StatusCode> = Result<T, ApiError>;

const DONE: StatusCode = StatusCode::NO_CONTENT;

pub fn router(app: App) -> Router {
    let art = ServiceBuilder::new()
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        ))
        .service(ServeDir::new(app.library.art_dir()));

    Router::new()
        .route("/api/state", get(state))
        .route("/api/ws", get(ws::upgrade))
        .route("/api/player/play", post(play))
        .route("/api/player/pause", post(pause))
        .route("/api/player/toggle", post(toggle))
        .route("/api/player/stop", post(stop))
        .route("/api/player/next", post(next))
        .route("/api/player/previous", post(previous))
        .route("/api/player/seek", post(seek))
        .route("/api/player/modes", post(modes))
        .route("/api/queue", get(queue).post(add).delete(clear))
        .route("/api/queue/{id}", delete(remove))
        .route("/api/queue/{id}/play", post(play_item))
        .route("/api/queue/{id}/move", post(move_item))
        .route("/api/library", get(library))
        .route("/api/library/albums", get(albums))
        .route("/api/library/albums/{id}", get(album))
        .route("/api/library/search", get(search))
        .route("/api/library/rescan", post(rescan))
        .route("/api/library/albums/{id}/art", post(download_art))
        .route("/api/library/albums/{id}/art/upload", post(upload_art).layer(DefaultBodyLimit::max(MAX_UPLOAD)))
        .route("/api/library/art/download-missing", post(download_missing_art))
        .route("/api/outputs/{id}/select", post(select_output))
        .route("/api/restart", post(restart))
        .route("/api/desktop", get(desktop_settings).post(save_desktop_settings))
        .route("/api/desktop/show", post(show_window))
        .route("/api/quit", post(quit))
        .route("/api/tags/read", post(tags::read))
        .route("/api/tags/write", post(tags::write))
        .route("/api/sources/{name}/acquire", post(acquire))
        .route("/api/sources/{name}/release", post(release))
        .nest_service("/art", art)
        .fallback(web::serve)
        .layer(CompressionLayer::new())
        .with_state(app)
}

async fn state(State(app): State<App>) -> Json<ServerEvent> {
    Json(app.hello())
}

async fn play(State(app): State<App>) -> ApiResult {
    app.play().await?;
    Ok(DONE)
}

async fn pause(State(app): State<App>) -> ApiResult {
    app.mpd.pause(true).await?;
    Ok(DONE)
}

async fn toggle(State(app): State<App>) -> ApiResult {
    if app.player().state == PlayState::Play {
        app.mpd.pause(true).await?;
    } else {
        app.play().await?;
    }
    Ok(DONE)
}

async fn stop(State(app): State<App>) -> ApiResult {
    app.mpd.stop().await?;
    Ok(DONE)
}

async fn next(State(app): State<App>) -> ApiResult {
    app.mpd.next().await?;
    Ok(DONE)
}

async fn previous(State(app): State<App>) -> ApiResult {
    app.mpd.previous().await?;
    Ok(DONE)
}

async fn seek(State(app): State<App>, Json(req): Json<SeekRequest>) -> ApiResult {
    app.mpd.seek(req.seconds).await?;
    Ok(DONE)
}

async fn modes(State(app): State<App>, Json(req): Json<ModesRequest>) -> ApiResult {
    app.mpd.set_modes(&req).await?;
    Ok(DONE)
}

async fn queue(State(app): State<App>) -> Json<Queue> {
    Json(app.queue())
}

async fn add(State(app): State<App>, Json(req): Json<AddRequest>) -> ApiResult {
    let mut uris = req.uris;
    if let Some(album) = req.album {
        let tracks = app.library.album_uris(album).await?;
        if tracks.is_empty() {
            return Err(ApiError::not_found("no such album"));
        }
        uris.extend(tracks);
    }
    if uris.is_empty() {
        return Err(ApiError::bad_request("nothing to add"));
    }
    // To MPD an empty URI is the root directory: the whole library.
    if uris.iter().any(|uri| uri.trim().is_empty()) {
        return Err(ApiError::bad_request("empty URI"));
    }
    let offset = req.start.and_then(|start| uris.iter().position(|uri| *uri == start)).unwrap_or(0);
    let first = app.mpd.add(&uris, req.mode).await?;
    if req.play {
        app.claim_local().await;
        app.mpd.play_pos(first + offset as u32).await?;
    }
    Ok(DONE)
}

/// Clears the queue but lets the current song finish.
async fn clear(State(app): State<App>) -> ApiResult {
    app.mpd.clear_except_current().await?;
    Ok(DONE)
}

async fn remove(State(app): State<App>, Path(id): Path<u32>) -> ApiResult {
    app.mpd.delete_id(id).await?;
    Ok(DONE)
}

async fn play_item(State(app): State<App>, Path(id): Path<u32>) -> ApiResult {
    app.claim_local().await;
    app.mpd.play_id(id).await?;
    Ok(DONE)
}

async fn move_item(State(app): State<App>, Path(id): Path<u32>, Json(req): Json<MoveRequest>) -> ApiResult {
    app.mpd.move_id(id, req.to).await?;
    Ok(DONE)
}

async fn library(State(app): State<App>) -> Json<LibraryStatus> {
    Json(app.library.status())
}

/// The whole album list in one response; the client filters and sorts locally.
/// The library revision is the ETag, so unchanged libraries cost a 304.
async fn albums(State(app): State<App>, headers: HeaderMap) -> ApiResult<Response> {
    let etag = format!("\"{}\"", app.library.status().revision);
    if headers.get(header::IF_NONE_MATCH).is_some_and(|v| v.as_bytes() == etag.as_bytes()) {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }
    let albums = app.library.albums().await?;
    Ok(([(header::ETAG, etag), (header::CACHE_CONTROL, "no-cache".into())], Json(albums)).into_response())
}

async fn album(State(app): State<App>, Path(id): Path<String>) -> ApiResult<Json<AlbumDetail>> {
    app.library.album(id).await?.map(Json).ok_or_else(|| ApiError::not_found("no such album"))
}

#[derive(Deserialize)]
struct SearchParams {
    q: String,
}

async fn search(State(app): State<App>, Query(params): Query<SearchParams>) -> ApiResult<Json<SearchResults>> {
    Ok(Json(app.library.search(params.q).await?))
}

async fn rescan(State(app): State<App>) -> ApiResult {
    app.mpd.update().await?;
    Ok(StatusCode::ACCEPTED)
}

/// Downloads a cover for one album into its folder, unless it already has art.
async fn download_art(State(app): State<App>, Path(id): Path<String>) -> ApiResult<Json<ArtOutcome>> {
    Ok(Json(app.library.download_art(id).await?))
}

/// Covers bigger than this are refused.
const MAX_UPLOAD: usize = 20 << 20;

/// Saves the uploaded image (the request body) as the album's cover, replacing any cover.* file.
async fn upload_art(State(app): State<App>, Path(id): Path<String>, body: Bytes) -> ApiResult {
    if virgil_library::cover_extension(&body).is_none() {
        return Err(ApiError::bad_request("choose a JPEG, PNG or WebP image"));
    }
    app.library.upload_art(id, body.to_vec()).await?;
    Ok(DONE)
}

/// Starts downloading covers for every album without art; progress arrives
/// in the library status.
async fn download_missing_art(State(app): State<App>) -> ApiResult {
    if app.library.download_missing_art() {
        Ok(StatusCode::ACCEPTED)
    } else {
        Err(ApiError::conflict("artwork is already being downloaded"))
    }
}

async fn select_output(State(app): State<App>, Path(id): Path<u32>) -> ApiResult {
    app.select_output(id).await?;
    Ok(DONE)
}

/// Restarts Virgil so MPD gets outputs for sound devices connected since it
/// started. Desktop mode starts a fresh Virgil.exe itself; otherwise virgild
/// exits and its supervisor (Docker, systemd) brings it back.
async fn restart(State(app): State<App>) -> ApiResult {
    tracing::info!("restart requested from the UI");
    tokio::spawn(async move {
        // Let the response go out first.
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        match &app.desktop {
            Some(desktop) => desktop.restart().await,
            None => std::process::exit(RESTART_EXIT_CODE),
        }
    });
    Ok(StatusCode::ACCEPTED)
}

fn desktop(app: &App) -> ApiResult<&std::sync::Arc<crate::desktop::Desktop>> {
    app.desktop.as_ref().ok_or_else(|| ApiError::not_found("not running in desktop mode"))
}

/// A second launch of Virgil.exe asks for the running one's window.
async fn show_window(State(app): State<App>) -> ApiResult {
    desktop(&app)?.show_window();
    Ok(DONE)
}

async fn desktop_settings(State(app): State<App>) -> ApiResult<Json<DesktopSettings>> {
    Ok(Json(desktop(&app)?.settings()))
}

/// Saves the settings and restarts Virgil to apply them.
async fn save_desktop_settings(State(app): State<App>, Json(settings): Json<DesktopSettings>) -> ApiResult {
    desktop(&app)?.save_settings(settings).map_err(|e| ApiError::bad_request(format!("{e:#}")))?;
    restart(State(app)).await
}

async fn quit(State(app): State<App>) -> ApiResult {
    let desktop = desktop(&app)?.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        desktop.quit().await
    });
    Ok(StatusCode::ACCEPTED)
}

/// Non-zero, so `Restart=on-failure` (systemd) and Docker's restart policy both bring Virgil back.
const RESTART_EXIT_CODE: i32 = 75;

async fn acquire(State(app): State<App>, Path(name): Path<String>) -> ApiResult {
    app.acquire(&name).await?;
    Ok(DONE)
}

async fn release(State(app): State<App>, Path(name): Path<String>) -> ApiResult {
    app.release(&name).await;
    Ok(DONE)
}
