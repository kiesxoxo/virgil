//! The SvelteKit build, embedded into the binary. Debug builds read it from
//! disk instead, so `npm run build` shows up without recompiling.

use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../web/build"]
struct Ui;

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path.starts_with("api/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    // Client-side routes (`/album/…`) all get the SPA shell.
    asset(path)
        .or_else(|| asset("index.html"))
        .unwrap_or_else(|| (StatusCode::NOT_FOUND, "web UI not built: run `npm run build` in web/").into_response())
}

fn asset(path: &str) -> Option<Response> {
    let path = if path.is_empty() { "index.html" } else { path };
    let file = Ui::get(path)?;
    let mime = mime_guess::from_path(path).first_or_octet_stream().to_string();
    // Vite fingerprints everything under _app/immutable.
    let cache = if path.starts_with("_app/immutable/") { "public, max-age=31536000, immutable" } else { "no-cache" };
    Some(([(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, cache.to_string())], file.data).into_response())
}
