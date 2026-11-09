//! Downloads against stand-ins for Lidarr and slskdpp that answer like the real ones.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};

use super::*;
use crate::config::{LidarrConfig, SlskdppConfig};

const ALBUM: &str = "0cabe2ec-43d7-49dc-bb91-5a3bd1eb2a7a";
const DOWNLOADED: &str = "/data/Downloads/slskdpp/Ash (2023)/01 - Flint.flac";
const IN_LIBRARY: &str = "/data/Music/Emptyset/Emptyset (2009)/01 Flint.flac";

/// What the stand-ins have, and what they were asked.
#[derive(Default)]
struct Fake {
    /// Lidarr has the album (id 158).
    has_album: bool,
    /// Lidarr has every track's file.
    complete: bool,
    /// Lidarr has the first song's file.
    song_in: bool,
    queue: Vec<Value>,
    added: Vec<Value>,
    monitored: Vec<Value>,
    commands: Vec<Value>,
    searching: bool,
    hits: Value,
    transfers: Vec<Value>,
    enqueued: Vec<Value>,
}

type Shared = Arc<Mutex<Fake>>;

fn album_json(fake: &Fake) -> Value {
    json!({
        "id": if fake.has_album { 158 } else { 0 },
        "title": "Emptyset",
        "foreignAlbumId": ALBUM,
        "albumType": "Album",
        "releaseDate": "2009-10-26T00:00:00Z",
        "monitored": false,
        "artistId": 1,
        "artist": { "id": 1, "artistName": "Emptyset", "foreignArtistId": "050092ae", "path": "/data/Music/Emptyset" },
        "remoteCover": "https://images.lidarr.audio/cover.jpg",
        "releases": [{ "id": 9, "monitored": true, "trackCount": 2 }],
        "statistics": { "trackFileCount": if fake.complete { 2 } else { u32::from(fake.song_in) }, "trackCount": 2 },
    })
}

fn lidarr(fake: Shared) -> Router {
    Router::new()
        .route(
            "/api/v1/rootfolder",
            get(|| async {
                Json(json!([{ "id": 1, "path": "/data/Music", "defaultQualityProfileId": 2, "defaultMetadataProfileId": 1 }]))
            }),
        )
        .route("/api/v1/album/lookup", get(|State(f): State<Shared>| async move { Json(json!([album_json(&f.lock().unwrap())])) }))
        .route(
            "/api/v1/album",
            get(|State(f): State<Shared>| async move { Json(json!([album_json(&f.lock().unwrap())])) }).post(
                |State(f): State<Shared>, Json(body): Json<Value>| async move {
                    let mut f = f.lock().unwrap();
                    f.added.push(body);
                    f.has_album = true;
                    (StatusCode::CREATED, Json(album_json(&f)))
                },
            ),
        )
        .route("/api/v1/album/{id}", get(|State(f): State<Shared>| async move { Json(album_json(&f.lock().unwrap())) }))
        .route(
            "/api/v1/album/monitor",
            axum::routing::put(|State(f): State<Shared>, Json(body): Json<Value>| async move {
                f.lock().unwrap().monitored.push(body);
                StatusCode::ACCEPTED
            }),
        )
        .route(
            "/api/v1/track",
            get(|State(f): State<Shared>| async move {
                let f = f.lock().unwrap();
                let first = f.complete || f.song_in;
                Json(json!([
                    { "id": 1001, "albumId": 158, "title": "Flint", "trackNumber": "1", "absoluteTrackNumber": 1,
                      "mediumNumber": 1, "duration": 169000, "hasFile": first, "trackFileId": if first { 77 } else { 0 } },
                    { "id": 1002, "albumId": 158, "title": "Flame", "trackNumber": "2", "absoluteTrackNumber": 2,
                      "mediumNumber": 1, "duration": 134000, "hasFile": f.complete, "trackFileId": if f.complete { 78 } else { 0 } },
                ]))
            }),
        )
        .route(
            "/api/v1/trackfile",
            get(|State(f): State<Shared>| async move {
                let f = f.lock().unwrap();
                let files = if f.complete || f.song_in { vec![json!({ "id": 77, "albumId": 158, "path": IN_LIBRARY })] } else { vec![] };
                Json(json!(files))
            }),
        )
        .route(
            "/api/v1/command",
            post(|State(f): State<Shared>, Json(body): Json<Value>| async move {
                let mut f = f.lock().unwrap();
                f.commands.push(body.clone());
                (StatusCode::CREATED, Json(json!({ "id": f.commands.len(), "name": body["name"], "status": "queued" })))
            }),
        )
        .route(
            "/api/v1/command/{id}",
            get(|Path(id): Path<u32>| async move {
                Json(json!({ "id": id, "name": "x", "status": "completed", "result": "successful" }))
            }),
        )
        .route("/api/v1/queue/details", get(|State(f): State<Shared>| async move { Json(json!(f.lock().unwrap().queue)) }))
        .route(
            "/api/v1/manualimport",
            get(|Query(q): Query<HashMap<String, String>>| async move {
                assert_eq!(q["folder"], "/data/Downloads/slskdpp/Ash (2023)");
                Json(json!([{ "path": DOWNLOADED, "albumReleaseId": 0, "quality": { "quality": { "id": 6, "name": "FLAC" } },
                              "rejections": [{ "reason": "Album match is not close enough" }] }]))
            }),
        )
        .with_state(fake)
}

fn slskdpp(fake: Shared) -> Router {
    Router::new()
        .route(
            "/api/v1/searches",
            post(|Json(body): Json<Value>| async move {
                assert_eq!(body["query"], "emptyset flint");
                (StatusCode::CREATED, Json(json!({ "id": 5, "query": body["query"], "active": true })))
            }),
        )
        .route(
            "/api/v1/searches/{id}",
            get(|State(f): State<Shared>| async move { Json(json!({ "id": 5, "active": f.lock().unwrap().searching })) })
                .delete(|| async { StatusCode::NO_CONTENT }),
        )
        .route("/api/v1/searches/{id}/hits", get(|State(f): State<Shared>| async move { Json(f.lock().unwrap().hits.clone()) }))
        .route(
            "/api/v1/downloads",
            get(|State(f): State<Shared>| async move { Json(json!(f.lock().unwrap().transfers)) }).post(
                |State(f): State<Shared>, Json(body): Json<Value>| async move {
                    let mut f = f.lock().unwrap();
                    f.enqueued.push(body);
                    let view = json!({ "id": 42, "folder": "Ash (2023)", "name": "01 - Flint.flac", "size": 100, "bytes": 0,
                                       "state": "queued_remote", "queue_place": 2 });
                    f.transfers = vec![view.clone()];
                    (StatusCode::CREATED, Json(json!([view])))
                },
            ),
        )
        .route("/api/v1/downloads/{id}/cancel", post(|| async { StatusCode::NO_CONTENT }))
        .with_state(fake)
}

async fn serve(router: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    format!("http://{addr}")
}

/// Virgil's side: MPD rescans, and the library has an album once its folder was rescanned.
#[derive(Default)]
struct Library {
    rescanned: Mutex<Vec<String>>,
}

impl Host for Library {
    async fn rescan(&self, folder: &str) -> anyhow::Result<()> {
        self.rescanned.lock().unwrap().push(folder.to_string());
        Ok(())
    }

    fn album_of(&self, uri: &str) -> Option<String> {
        let rescanned = self.rescanned.lock().unwrap();
        (rescanned.iter().any(|f| uri.starts_with(&format!("{f}/")))).then(|| "a1b2".to_string())
    }
}

async fn fetcher(fake: &Shared, patience: Patience) -> Fetcher {
    let lidarr = LidarrConfig { url: serve(super::tests::lidarr(fake.clone())).await, api_key: "k".into(), root: None };
    let slskdpp = SlskdppConfig {
        url: serve(super::tests::slskdpp(fake.clone())).await,
        api_key: "s".into(),
        downloads: "/data/Downloads/slskdpp/".into(),
        formats: vec!["flac".into(), "mp3".into()],
    };
    Fetcher::new(&lidarr, Some(&slskdpp), None, patience).unwrap()
}

fn only(fetcher: &Fetcher) -> Download {
    let downloads = fetcher.downloads();
    assert_eq!(downloads.jobs.len(), 1);
    downloads.jobs[0].clone()
}

#[tokio::test]
async fn gets_an_album_through_lidarr() {
    let fake = Shared::default();
    let fetcher = fetcher(&fake, Patience::default()).await;
    let library = Library::default();

    let started =
        fetcher.start(&DownloadRequest { album: ALBUM.into(), track: None }, Some("ola".into())).await.unwrap();
    assert_eq!(
        (started.state, started.title.as_str(), started.by.as_deref()),
        (DownloadState::Adding, "Emptyset", Some("ola"))
    );
    // Asking again gets the same download.
    let again = fetcher.start(&DownloadRequest { album: ALBUM.into(), track: None }, None).await.unwrap();
    assert_eq!(again.id, started.id);

    // Lidarr grabs a release as soon as it searches.
    fake.lock().unwrap().queue = vec![json!({ "id": 3, "albumId": 158, "size": 400.0, "sizeleft": 100.0,
                                              "status": "downloading", "trackedDownloadState": "downloading" })];
    assert!(fetcher.round(&library).await);
    let download = only(&fetcher);
    assert_eq!((download.state, download.progress), (DownloadState::Downloading, Some(0.75)));
    {
        let fake = fake.lock().unwrap();
        assert_eq!(fake.added.len(), 1, "Lidarr adds the album");
        assert_eq!(fake.added[0]["monitored"], json!(true));
        assert_eq!(fake.added[0]["addOptions"]["searchForNewAlbum"], json!(false), "Virgil starts the search itself");
        assert_eq!(fake.commands, [json!({ "name": "AlbumSearch", "albumIds": [158] })]);
    }

    // Imported: Virgil rescans the artist's folder, and links the album once the library has it.
    {
        let mut fake = fake.lock().unwrap();
        fake.queue.clear();
        fake.complete = true;
    }
    fetcher.round(&library).await;
    assert_eq!(*library.rescanned.lock().unwrap(), ["Emptyset"]);
    assert!(!fetcher.round(&library).await, "nothing left under way");
    let download = only(&fetcher);
    assert_eq!((download.state, download.album_id.as_deref()), (DownloadState::Done, Some("a1b2")));
}

#[tokio::test]
async fn an_album_that_comes_in_short_is_still_in() {
    let fake = Shared::default();
    fake.lock().unwrap().has_album = true;
    let fetcher = fetcher(&fake, Patience::default()).await;
    let library = Library::default();

    fetcher.start(&DownloadRequest { album: ALBUM.into(), track: None }, None).await.unwrap();
    fake.lock().unwrap().queue = vec![json!({ "id": 3, "albumId": 158, "size": 400.0, "sizeleft": 0.0,
                                              "status": "completed", "trackedDownloadState": "importing" })];
    fetcher.round(&library).await;
    assert_eq!(only(&fetcher).state, DownloadState::Importing);

    // The release Lidarr found lacked a track: one of two came in.
    {
        let mut fake = fake.lock().unwrap();
        fake.queue.clear();
        fake.song_in = true;
    }
    fetcher.round(&library).await;
    assert_eq!(*library.rescanned.lock().unwrap(), ["Emptyset"], "rescanned all the same");
    fetcher.round(&library).await;
    assert_eq!(only(&fetcher).state, DownloadState::Done);
}

#[tokio::test]
async fn an_album_nobody_has_stays_wanted() {
    let fake = Shared::default();
    fake.lock().unwrap().has_album = true;
    let fetcher = fetcher(&fake, Patience { grab: 0, ..Patience::default() }).await;
    let library = Library::default();

    fetcher.start(&DownloadRequest { album: ALBUM.into(), track: None }, None).await.unwrap();
    fetcher.round(&library).await;
    let download = only(&fetcher);
    assert_eq!(download.state, DownloadState::NotFound);
    let fake = fake.lock().unwrap();
    assert!(fake.added.is_empty(), "Lidarr had it already");
    assert_eq!(fake.monitored, [json!({ "albumIds": [158], "monitored": true })], "and keeps looking out for it");

    // Tried again, it starts over.
    drop(fake);
    let retried = fetcher.retry(download.id).unwrap();
    assert_eq!(retried.state, DownloadState::Adding);
}

#[tokio::test]
async fn gets_a_song_from_soulseek_into_its_album() {
    let fake = Shared::default();
    {
        let mut fake = fake.lock().unwrap();
        fake.has_album = true;
        fake.searching = true;
        fake.hits = serde_json::from_str(include_str!("../../../fetch/tests/fixtures/hits.json")).unwrap();
    }
    let fetcher = fetcher(&fake, Patience { answers: 0, ..Patience::default() }).await;
    let library = Library::default();

    let started = fetcher.start(&DownloadRequest { album: ALBUM.into(), track: Some(1001) }, None).await.unwrap();
    assert_eq!(
        (started.kind, started.title.as_str(), started.album_title.as_str()),
        (DownloadKind::Song, "Flint", "Emptyset")
    );

    fetcher.round(&library).await; // searches
    fetcher.round(&library).await; // takes the best answer, waits in their queue
    let download = only(&fetcher);
    assert_eq!((download.state, download.note.as_deref()), (DownloadState::Waiting, Some("2nd in their queue")));
    assert_eq!(fake.lock().unwrap().enqueued[0]["username"], json!("ola"));

    fake.lock().unwrap().transfers[0]["state"] = json!("completed");
    fetcher.round(&library).await;
    assert_eq!(only(&fetcher).state, DownloadState::Importing);
    let import = fake.lock().unwrap().commands.last().cloned().unwrap();
    assert_eq!(import["name"], json!("ManualImport"));
    let file = &import["files"][0];
    assert_eq!(file["path"], json!(DOWNLOADED));
    assert_eq!((file["trackIds"].clone(), file["albumReleaseId"].clone()), (json!([1001]), json!(9)));
    assert_eq!(file["quality"]["quality"]["name"], json!("FLAC"));

    fake.lock().unwrap().song_in = true;
    fetcher.round(&library).await; // imported: rescans
    fetcher.round(&library).await; // the library has it
    let download = only(&fetcher);
    assert_eq!((download.state, download.album_id.as_deref()), (DownloadState::Done, Some("a1b2")));
}

#[tokio::test]
async fn songs_need_the_album_in_lidarr() {
    let fake = Shared::default();
    let fetcher = fetcher(&fake, Patience::default()).await;
    let error = fetcher.start(&DownloadRequest { album: ALBUM.into(), track: Some(1001) }, None).await.unwrap_err();
    assert!(error.to_string().contains("Open the album first"));
}

#[test]
fn ordinals() {
    assert_eq!(
        [1, 2, 3, 4, 11, 12, 13, 21, 22, 103].map(ordinal),
        ["1st", "2nd", "3rd", "4th", "11th", "12th", "13th", "21st", "22nd", "103rd"]
    );
}
