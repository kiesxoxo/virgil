use axum::body::{Body, Bytes};
use axum::extract::{FromRequestParts, Path, Query, State};
use axum::http::request::Parts;
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
    AddRequest, Added, Aircheck, AlbumDetail, HostRequest, LibraryStatus, LinkRequest, ModesRequest, MoveRequest,
    PartyRequest, Person, PlayState, ProfileRequest, Queue, Radio, SearchResults, SeekRequest, ServerEvent, SkipRule,
};

use crate::app::App;
use crate::party::{Caller, clean_note};
use crate::{web, ws};

/// The header each device sends its token in (the WebSocket, which can't, uses `?token=`).
const TOKEN_HEADER: &str = "x-virgil-token";

/// The device's secret token, if it sent one.
pub struct Token(pub Option<String>);

impl<S: Send + Sync> FromRequestParts<S> for Token {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let header = parts.headers.get(TOKEN_HEADER).and_then(|v| v.to_str().ok()).map(str::to_string);
        let query = || parts.uri.query()?.split('&').find_map(|pair| pair.strip_prefix("token=")).map(str::to_string);
        Ok(Token(header.or_else(query)))
    }
}

impl FromRequestParts<App> for Caller {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, app: &App) -> Result<Self, Self::Rejection> {
        let Ok(Token(token)) = Token::from_request_parts(parts, app).await;
        Ok(app.caller(token.as_deref()))
    }
}

/// Refuses guests while the line is open; `what` finishes "Ask the host to …".
fn host_only(caller: &Caller, what: &str) -> ApiResult<()> {
    if caller.host { Ok(()) } else { Err(ApiError::forbidden(format!("Ask the host to {what}"))) }
}

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

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self { status: StatusCode::FORBIDDEN, message: message.into() }
    }

    /// A service Virgil relies on (NTS, MusicBrainz) didn't answer.
    pub fn upstream(e: anyhow::Error) -> Self {
        Self { status: StatusCode::BAD_GATEWAY, message: format!("{e:#}") }
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
    // Covers never change under a URL (it names a hash of the picture), but one that isn't there
    // yet may be soon (a size being added): only what was found is cached for good.
    let art = ServiceBuilder::new()
        .layer(SetResponseHeaderLayer::overriding(header::CACHE_CONTROL, |res: &Response<_>| {
            res.status().is_success().then(|| HeaderValue::from_static("public, max-age=31536000, immutable"))
        }))
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
        .route("/api/queue/requests/{request}", delete(remove_request))
        .route("/api/people/me", get(me).put(set_me))
        .route("/api/people/me/link", post(link_me))
        .route("/api/people/{id}/host", post(set_host))
        .route("/api/people/{id}", delete(forget))
        .route("/api/party", post(set_party))
        .route("/api/party/skip", post(skip))
        .route("/api/party/cheer", post(cheer))
        .route("/api/party/aircheck", get(aircheck))
        .route("/api/library", get(library))
        .route("/api/library/albums", get(albums))
        .route("/api/library/albums/{id}", get(album))
        .route("/api/library/search", get(search))
        .route("/api/library/rescan", post(rescan))
        .route("/api/radio", get(radio))
        .route("/api/radio/{id}/play", post(play_station))
        .route("/api/radio/{id}/aac", get(aac_stream))
        .route("/api/outputs/{id}/select", post(select_output))
        .route("/api/restart", post(restart))
        .route("/api/sources/{name}/acquire", post(acquire))
        .route("/api/sources/{name}/release", post(release))
        .route("/api/homekit/reset", post(homekit_reset))
        .nest_service("/art", art)
        .fallback(web::serve)
        .layer(CompressionLayer::new())
        .with_state(app)
}

async fn state(State(app): State<App>) -> Json<ServerEvent> {
    Json(app.hello_live().await)
}

/// Guests may pause and resume when the rules say so.
fn may_pause(caller: &Caller) -> ApiResult<()> {
    if caller.rules.guests_pause { Ok(()) } else { host_only(caller, "pause") }
}

async fn play(State(app): State<App>, caller: Caller) -> ApiResult {
    may_pause(&caller)?;
    app.play().await?;
    Ok(DONE)
}

async fn pause(State(app): State<App>, caller: Caller) -> ApiResult {
    may_pause(&caller)?;
    app.mpd.pause(true).await?;
    Ok(DONE)
}

async fn toggle(State(app): State<App>, caller: Caller) -> ApiResult {
    may_pause(&caller)?;
    if app.player().state == PlayState::Play {
        app.pause_local().await?;
    } else {
        app.play().await?;
    }
    Ok(DONE)
}

async fn stop(State(app): State<App>, caller: Caller) -> ApiResult {
    may_pause(&caller)?;
    app.mpd.stop().await?;
    Ok(DONE)
}

async fn next(State(app): State<App>, caller: Caller) -> ApiResult {
    if caller.rules.skip != SkipRule::Free {
        host_only(&caller, "skip, or vote to skip")?;
    }
    app.party.skipping();
    app.mpd.next().await?;
    Ok(DONE)
}

async fn previous(State(app): State<App>, caller: Caller) -> ApiResult {
    host_only(&caller, "go back")?;
    app.mpd.previous().await?;
    Ok(DONE)
}

async fn seek(State(app): State<App>, caller: Caller, Json(req): Json<SeekRequest>) -> ApiResult {
    host_only(&caller, "seek")?;
    app.mpd.seek(req.seconds).await?;
    Ok(DONE)
}

async fn modes(State(app): State<App>, caller: Caller, Json(req): Json<ModesRequest>) -> ApiResult {
    host_only(&caller, "change how the queue plays")?;
    app.mpd.set_modes(&req).await?;
    Ok(DONE)
}

async fn queue(State(app): State<App>) -> Json<Queue> {
    Json(app.queue())
}

async fn add(State(app): State<App>, caller: Caller, Json(req): Json<AddRequest>) -> ApiResult<Json<Added>> {
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
    let start = req.start.and_then(|start| uris.iter().position(|uri| *uri == start)).unwrap_or(0);
    let added = app.request(&caller, &uris, req.mode, req.play, start, clean_note(req.note)).await?;
    Ok(Json(added))
}

/// Clears the queue but lets the current song finish.
async fn clear(State(app): State<App>, caller: Caller) -> ApiResult {
    host_only(&caller, "clear the queue")?;
    app.clear().await?;
    Ok(DONE)
}

async fn remove(State(app): State<App>, caller: Caller, Path(id): Path<u32>) -> ApiResult {
    app.remove(&caller, id).await?;
    Ok(DONE)
}

async fn remove_request(State(app): State<App>, caller: Caller, Path(request): Path<u32>) -> ApiResult {
    app.remove_request(&caller, request).await?;
    Ok(DONE)
}

async fn play_item(State(app): State<App>, caller: Caller, Path(id): Path<u32>) -> ApiResult {
    host_only(&caller, "play that now")?;
    app.claim_local().await;
    app.mpd.play_id(id).await?;
    Ok(DONE)
}

async fn move_item(
    State(app): State<App>,
    caller: Caller,
    Path(id): Path<u32>,
    Json(req): Json<MoveRequest>,
) -> ApiResult {
    host_only(&caller, "move things around")?;
    app.move_item(id, req.to).await?;
    Ok(DONE)
}

async fn me(caller: Caller) -> ApiResult<Json<Person>> {
    caller.person.map(Json).ok_or_else(|| ApiError::not_found("this device hasn't said who it is"))
}

async fn set_me(
    State(app): State<App>,
    Token(token): Token,
    Json(req): Json<ProfileRequest>,
) -> ApiResult<Json<Person>> {
    let token = token.ok_or_else(|| ApiError::bad_request("no device token"))?;
    let person = app.party.register(&token, &req).ok_or_else(|| ApiError::bad_request("bad device token"))?;
    if req.join == Some(true) && app.party.is_open() {
        app.shout(format!("{} called in", person.name));
    }
    app.refresh_party();
    app.reenrich_queue();
    Ok(Json(person))
}

async fn link_me(
    State(app): State<App>,
    Token(token): Token,
    caller: Caller,
    Json(req): Json<LinkRequest>,
) -> ApiResult<Json<Person>> {
    let token = token.ok_or_else(|| ApiError::bad_request("no device token"))?;
    let person = app.party.link(&token, &req.person, &caller).map_err(ApiError::forbidden)?;
    app.refresh_party();
    Ok(Json(person))
}

/// Refuses to leave the line without a host: someone has to be able to close it.
fn keeps_a_host(app: &App, id: &str) -> ApiResult<()> {
    let people = app.party.party().people;
    let hosts = people.iter().filter(|p| p.host).count();
    if hosts == 1 && people.iter().any(|p| p.host && p.id == id) {
        return Err(ApiError::bad_request("Someone has to stay host"));
    }
    Ok(())
}

async fn set_host(
    State(app): State<App>,
    caller: Caller,
    Path(id): Path<String>,
    Json(req): Json<HostRequest>,
) -> ApiResult {
    host_only(&caller, "choose the hosts")?;
    if !req.host {
        keeps_a_host(&app, &id)?;
    }
    if !app.party.set_host(&id, req.host) {
        return Err(ApiError::not_found("no such person"));
    }
    app.refresh_party();
    Ok(DONE)
}

async fn forget(State(app): State<App>, caller: Caller, Path(id): Path<String>) -> ApiResult {
    host_only(&caller, "forget people")?;
    keeps_a_host(&app, &id)?;
    if !app.party.forget(&id) {
        return Err(ApiError::not_found("no such person"));
    }
    app.refresh_party();
    app.reenrich_queue();
    Ok(DONE)
}

async fn set_party(State(app): State<App>, caller: Caller, Json(req): Json<PartyRequest>) -> ApiResult {
    host_only(&caller, "run the request line")?;
    let was_open = app.party.is_open();
    app.party.set(req.open, req.rules);
    match (was_open, app.party.is_open()) {
        (false, true) => app.shout("The request line is open".into()),
        (true, false) => app.shout("The request line is closed".into()),
        _ => {}
    }
    app.refresh_party();
    Ok(DONE)
}

/// Skips the song playing for hosts, whoever picked it, and everyone when the rules say so;
/// otherwise it toggles a vote, and skips once enough people want it gone.
async fn skip(State(app): State<App>, caller: Caller) -> ApiResult {
    let current = app.current().ok_or_else(|| ApiError::not_found("nothing is playing"))?;
    if app.player().state == PlayState::Stop {
        return Err(ApiError::conflict("Nothing is playing"));
    }
    let picked = current.pick.as_ref().is_some_and(|p| Some(p.by.as_str()) == caller.id());
    if caller.host || picked || caller.rules.skip == SkipRule::Free {
        app.party.skipping();
        app.mpd.next().await?;
        return Ok(DONE);
    }
    if caller.rules.skip == SkipRule::Off {
        return Err(ApiError::forbidden("Skipping is up to the host tonight"));
    }
    let who = caller.id().ok_or_else(|| ApiError::bad_request("say who you are to vote"))?;
    let (votes, need) = app.party.vote_skip(who, current.id);
    if votes >= need {
        app.party.skipping();
        app.mpd.next().await?;
        let here = app.party.party().here.len().max(votes as usize);
        app.shout(format!("Skipped by vote ({votes} of {here})"));
    }
    app.refresh_party();
    Ok(DONE)
}

async fn cheer(State(app): State<App>, caller: Caller) -> ApiResult {
    let current = app.current().ok_or_else(|| ApiError::not_found("nothing is playing"))?;
    let who = caller.id().ok_or_else(|| ApiError::bad_request("say who you are to cheer"))?;
    app.party.cheer(who, current.id);
    app.refresh_party();
    Ok(DONE)
}

/// The night in progress or, with the line closed, the last one.
async fn aircheck(State(app): State<App>) -> ApiResult<Json<Aircheck>> {
    app.party.aircheck().map(Json).ok_or_else(|| ApiError::not_found("no nights on the request line yet"))
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

async fn rescan(State(app): State<App>, caller: Caller) -> ApiResult {
    host_only(&caller, "rescan the library")?;
    app.mpd.update().await?;
    Ok(StatusCode::ACCEPTED)
}

/// NTS's live channels, with the shows on air, and its Infinite Mixtapes.
async fn radio(State(app): State<App>) -> ApiResult<Json<Radio>> {
    app.radio.radio().await.map(Json).map_err(ApiError::upstream)
}

async fn play_station(State(app): State<App>, caller: Caller, Path(id): Path<String>) -> ApiResult {
    host_only(&caller, "tune the radio")?;
    let station = app.radio.station(&id).await.map_err(ApiError::upstream)?;
    let station = station.ok_or_else(|| ApiError::not_found("no such station"))?;
    // AAC when there is any (the mixtapes), through this server's relay; else MP3.
    let uri = match station.stream_aac {
        Some(_) => format!("http://127.0.0.1:{}/api/radio/{}/aac", app.config.listen.port(), station.id),
        None => station.stream,
    };
    app.play_stream(&uri).await?;
    Ok(DONE)
}

/// A station's AAC stream for MPD: NTS serves it as HLS, which MPD can't
/// play, so its chunks are joined into one ADTS stream here.
async fn aac_stream(State(app): State<App>, Path(id): Path<String>) -> ApiResult<Response> {
    let station = app.radio.station(&id).await.map_err(ApiError::upstream)?;
    let Some(playlist) = station.and_then(|s| s.stream_aac) else {
        return Err(ApiError::not_found("no AAC stream for this station"));
    };
    let chunks = app.radio.relay_aac(&playlist);
    let body = Body::from_stream(futures_util::stream::unfold(chunks, |mut chunks| async move {
        let chunk = chunks.recv().await?;
        Some((Ok::<_, std::convert::Infallible>(Bytes::from(chunk)), chunks))
    }));
    Ok(([(header::CONTENT_TYPE, "audio/aac"), (header::CACHE_CONTROL, "no-store")], body).into_response())
}

async fn select_output(State(app): State<App>, caller: Caller, Path(id): Path<u32>) -> ApiResult {
    host_only(&caller, "change the output")?;
    app.select_output(id).await?;
    Ok(DONE)
}

/// Restarts Virgil so MPD gets outputs for sound devices connected since it
/// started: virgild exits and its supervisor (Docker, systemd) brings it back.
async fn restart(caller: Caller) -> ApiResult {
    host_only(&caller, "restart Virgil")?;
    tracing::info!("restart requested from the UI");
    tokio::spawn(async {
        // Let the response go out first.
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        std::process::exit(RESTART_EXIT_CODE);
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

async fn homekit_reset(State(app): State<App>, caller: Caller) -> ApiResult {
    host_only(&caller, "reset Apple Home")?;
    let homekit = app.homekit.as_ref().ok_or_else(|| ApiError::not_found("HomeKit is off"))?;
    homekit.reset()?;
    Ok(DONE)
}
