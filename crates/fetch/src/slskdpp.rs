//! slskdpp's own API (v1): Soulseek searches and downloads. Authenticated with `X-API-Key`.
//! slskdpp downloads files and nothing more; Lidarr files them into the library.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{answer, base, client};

const SERVICE: &str = "slskdpp";

pub struct Slskdpp {
    http: reqwest::Client,
    base: String,
    key: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Search {
    pub id: u32,
    #[serde(default)]
    pub query: String,
    /// Still taking responses.
    #[serde(default)]
    pub active: bool,
    /// Users who answered.
    #[serde(default)]
    pub responses: u32,
    #[serde(default)]
    pub files: u32,
}

/// Every file one user returned from one folder.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FolderHit {
    pub user: String,
    /// The folder's full remote path.
    pub folder: String,
    #[serde(default)]
    pub files: Vec<File>,
    #[serde(default)]
    pub slot_free: bool,
    #[serde(default)]
    pub queue_length: u32,
    /// Bytes per second.
    #[serde(default)]
    pub upload_speed: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct File {
    /// The full remote path, as needed to download it.
    pub path: String,
    pub name: String,
    pub size: u64,
    /// Lowercase, without the dot.
    #[serde(default)]
    pub ext: String,
    pub bitrate: Option<u32>,
    /// Seconds.
    pub duration: Option<u32>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Download {
    pub id: i64,
    /// The local folder, relative to the downloads directory.
    #[serde(default)]
    pub folder: String,
    /// The local file name.
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub bytes: u64,
    /// queued, requested, queued_remote, initializing, transferring, completed, failed, cancelled
    pub state: String,
    pub error: Option<String>,
    /// Place in the peer's upload queue, while in it.
    pub queue_place: Option<u32>,
}

impl Download {
    pub fn progress(&self) -> Option<f32> {
        (self.size > 0).then(|| (self.bytes as f64 / self.size as f64).clamp(0.0, 1.0) as f32)
    }
}

impl Slskdpp {
    pub fn new(url: &str, key: &str) -> anyhow::Result<Self> {
        Ok(Self { http: client()?, base: base(url), key: key.to_string() })
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.http.request(method, format!("{}/api/v1/{path}", self.base)).header("X-API-Key", &self.key)
    }

    /// Asks the Soulseek network; answers come in over the next minutes.
    pub async fn search(&self, query: &str) -> anyhow::Result<Search> {
        let response = self.request(reqwest::Method::POST, "searches").json(&json!({ "query": query })).send().await;
        if response.as_ref().is_ok_and(|r| r.status() == reqwest::StatusCode::SERVICE_UNAVAILABLE) {
            anyhow::bail!("Soulseek isn't connected");
        }
        answer(SERVICE, response).await
    }

    pub async fn search_status(&self, id: u32) -> anyhow::Result<Search> {
        answer(SERVICE, self.request(reqwest::Method::GET, &format!("searches/{id}")).send().await).await
    }

    /// What came back, in folders with at least one file of `formats` (any, when empty).
    pub async fn hits(&self, id: u32, formats: &[String]) -> anyhow::Result<Vec<FolderHit>> {
        let mut request = self.request(reqwest::Method::GET, &format!("searches/{id}/hits"));
        if !formats.is_empty() {
            request = request.query(&[("formats", formats.join(","))]);
        }
        answer(SERVICE, request.send().await).await
    }

    pub async fn forget_search(&self, id: u32) -> anyhow::Result<()> {
        let _: Value =
            answer(SERVICE, self.request(reqwest::Method::DELETE, &format!("searches/{id}")).send().await).await?;
        Ok(())
    }

    /// Queues one file from one user.
    pub async fn download(&self, user: &str, path: &str, size: u64) -> anyhow::Result<Download> {
        let body = json!({ "username": user, "files": [{ "path": path, "size": size }] });
        let response = self.request(reqwest::Method::POST, "downloads").json(&body).send().await;
        let views: Vec<Download> = answer(SERVICE, response).await?;
        views.into_iter().next().ok_or_else(|| anyhow::anyhow!("{SERVICE} queued nothing"))
    }

    pub async fn downloads(&self) -> anyhow::Result<Vec<Download>> {
        answer(SERVICE, self.request(reqwest::Method::GET, "downloads").send().await).await
    }

    pub async fn cancel(&self, id: i64) -> anyhow::Result<()> {
        let path = format!("downloads/{id}/cancel");
        let _: Value = answer(SERVICE, self.request(reqwest::Method::POST, &path).send().await).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_hits_and_downloads() {
        let hits: Vec<FolderHit> = serde_json::from_str(include_str!("../tests/fixtures/hits.json")).unwrap();
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].files[0].duration, Some(169));
        assert!(hits[0].slot_free);

        let downloads: Vec<Download> = serde_json::from_str(include_str!("../tests/fixtures/downloads.json")).unwrap();
        assert_eq!(downloads[0].progress(), Some(0.5));
        assert_eq!(downloads[0].state, "transferring");
        assert_eq!(downloads[1].queue_place, Some(4));
    }
}
