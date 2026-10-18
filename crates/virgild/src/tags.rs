//! Tag editing: reading and writing the files behind library tracks.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use axum::Json;
use axum::extract::State;
use tokio::sync::Mutex;
use tracing::{info, warn};
use virgil_core::{TagWriteError, TagWriteRequest, TagWriteResult, TaggedFile, TagsRequest};

use crate::api::{ApiError, ApiResult};
use crate::app::App;

/// More files than this in one request is a mistake, not a selection.
const MAX_FILES: usize = 5000;

/// One write at a time: two edits racing on the same file would lose one.
static WRITING: Mutex<()> = Mutex::const_new(());

/// The file for a library URI, which must stay inside the music directory.
fn resolve(music_dir: &Path, uri: &str) -> Result<PathBuf, String> {
    let relative = Path::new(uri);
    if uri.is_empty() || uri.contains("://") || !relative.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err(format!("{uri} is not a file in the library"));
    }
    Ok(music_dir.join(relative))
}

fn music_dir(app: &App, uris: usize) -> ApiResult<PathBuf> {
    if uris > MAX_FILES {
        return Err(ApiError::bad_request(format!("at most {MAX_FILES} files at once")));
    }
    app.config
        .music_dir
        .clone()
        .ok_or_else(|| ApiError::conflict("music_dir is not set in virgild's config, so tags can't be edited"))
}

pub async fn read(State(app): State<App>, Json(req): Json<TagsRequest>) -> ApiResult<Json<Vec<TaggedFile>>> {
    let dir = music_dir(&app, req.uris.len())?;
    let files = tokio::task::spawn_blocking(move || {
        req.uris
            .into_iter()
            .map(|uri| {
                match resolve(&dir, &uri).and_then(|path| virgil_tags::read(&path).map_err(|e| format!("{e:#}"))) {
                    Ok((fields, info)) => TaggedFile { uri, fields, info: Some(info), error: None },
                    Err(error) => TaggedFile { uri, fields: Vec::new(), info: None, error: Some(error) },
                }
            })
            .collect()
    })
    .await
    .map_err(anyhow::Error::from)?;
    Ok(Json(files))
}

pub async fn write(State(app): State<App>, Json(req): Json<TagWriteRequest>) -> ApiResult<Json<TagWriteResult>> {
    let dir = music_dir(&app, req.edits.len())?;
    for field in req.edits.iter().flat_map(|e| &e.set) {
        virgil_tags::check_name(&field.name).map_err(|e| ApiError::bad_request(e.to_string()))?;
    }
    let _writing = WRITING.lock().await;
    let (result, written) = tokio::task::spawn_blocking(move || {
        let mut result = TagWriteResult::default();
        let mut written = Vec::new();
        for edit in req.edits {
            match resolve(&dir, &edit.uri)
                .and_then(|path| virgil_tags::write(&path, &edit).map_err(|e| format!("{e:#}")))
            {
                Ok(true) => {
                    result.written += 1;
                    written.push(edit.uri);
                }
                Ok(false) => {}
                Err(error) => {
                    warn!("tags: {error}");
                    result.failed.push(TagWriteError { uri: edit.uri, error });
                }
            }
        }
        (result, written)
    })
    .await
    .map_err(anyhow::Error::from)?;
    if !written.is_empty() {
        info!("tags: wrote {} file(s)", written.len());
        // Rescan just the folders that changed; the library follows MPD's database.
        let folders: BTreeSet<&str> =
            written.iter().map(|uri| uri.rsplit_once('/').map_or("", |(dir, _)| dir)).collect();
        for folder in folders {
            if let Err(e) = app.mpd.update_path(folder).await {
                warn!("tags: rescanning {folder}: {e}");
            }
        }
    }
    Ok(Json(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_uris_inside_the_music_dir() {
        let music = Path::new("/music");
        assert_eq!(resolve(music, "A/01 x.flac").unwrap(), Path::new("/music/A/01 x.flac"));
        for bad in ["", "../etc/passwd", "/etc/passwd", "A/../../x", "http://radio/stream", "./x"] {
            assert!(resolve(music, bad).is_err(), "{bad}");
        }
    }
}
