use std::path::{Path, PathBuf};

use color_eyre::eyre::OptionExt;
use log::warn;
use serde::Deserialize;
use tokio::fs;

use crate::consts::{HISTORY_API_URL, PAGE_URL};

/// Representation of the revision number in history API's response.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
struct RevisionNumberRepr {
    id: u64,
}

/// Representation of the history API's response.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct HistoryApiResponse {
    revisions: Vec<RevisionNumberRepr>,
}
impl From<HistoryApiResponse> for RevisionList {
    fn from(res: HistoryApiResponse) -> Self {
        let list = res
            .revisions
            .into_iter()
            .map(|RevisionNumberRepr { id }| id)
            .collect();
        Self(list)
    }
}

/// A list of revision IDs of a Wikipedia article.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(from = "HistoryApiResponse")]
struct RevisionList(Vec<u64>);

/// Query Wikipedia to find out the ID of the latest page revision.
async fn query_latest_revision(client: &reqwest::Client) -> color_eyre::Result<u64> {
    let list: RevisionList = client
        .get(HISTORY_API_URL)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let latest = list.0.first().ok_or_eyre("Revision history is empty")?;
    Ok(*latest)
}

/// Get the latest cached revision.
async fn get_latest_cached_revision(cache_dir: impl AsRef<Path>) -> color_eyre::Result<u64> {
    let cache_dir = cache_dir.as_ref();

    let mut max_rev = None;

    let mut read_dir = fs::read_dir(cache_dir).await?;
    while let Some(entry) = read_dir.next_entry().await? {
        let file_path = entry.path();
        let Some(rev) = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse().ok())
        else {
            continue; // ignore files with bad names
        };
        if max_rev.unwrap_or_default() < rev {
            max_rev = Some(rev);
        }
    }

    max_rev.ok_or_eyre("No cached pages found")
}

/// Get the local path for a revision.
///
/// This function does not perform any verification that this path exists.
fn get_revision_path(cache_dir: impl AsRef<Path>, revision: u64) -> PathBuf {
    cache_dir.as_ref().join(format!("{revision}.html"))
}

/// Cache the specified revision if it does not exist already.
async fn ensure_cached(
    cache_dir: impl AsRef<Path>,
    client: &reqwest::Client,
    revision: u64,
) -> color_eyre::Result<PathBuf> {
    let cache_dir = cache_dir.as_ref();

    // use cached if exists
    let page_path = get_revision_path(cache_dir, revision);
    if page_path.exists() {
        return Ok(page_path);
    }

    // fetch
    let url = format!("{PAGE_URL}?oldid={revision}");
    let page_bytes = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;

    // cache
    fs::create_dir_all(&cache_dir).await?;
    fs::write(&page_path, page_bytes).await?;

    Ok(page_path)
}

/// Get the Wikipedia page with network disabled. Will error if no cached page is available.
///
/// If a revision is absent, we return the newest available revision.
///
/// Returns the page as string.
pub async fn get_wikipedia_page_offline(
    cache_dir: impl AsRef<Path>,
    revision: Option<u64>,
) -> color_eyre::Result<String> {
    let cache_dir = cache_dir.as_ref();

    let revision = match revision {
        Some(r) => r,
        None => get_latest_cached_revision(cache_dir).await?,
    };

    let page_path = get_revision_path(cache_dir, revision);
    let page_str = fs::read_to_string(page_path).await?;

    Ok(page_str)
}

/// Get the Wikipedia page with network enabled. Cache is written or read when appropriate.
///
/// This function is intended to provide the best experience for the user. Therefore,
/// - if a revision is provided, we only return `Ok` when the exact revision is accessible.
/// - if a revision is absent, we return the newest accessible revision, and only error
///     when nothing is available.
///
/// Returns the page as string.
pub async fn get_wikipedia_page_online(
    cache_dir: impl AsRef<Path>,
    client: &reqwest::Client,
    revision: Option<u64>,
) -> color_eyre::Result<String> {
    let cache_dir = cache_dir.as_ref();

    let page_path = match revision {
        Some(rev_id) => ensure_cached(cache_dir, client, rev_id).await?,
        None => {
            let latest_page_path = match query_latest_revision(client).await {
                Ok(r) => match ensure_cached(cache_dir, client, r).await {
                    Ok(path) => Some(path),
                    Err(err) => {
                        warn!("Successfully queried the latest revision ID ({r}), but fetch failed: {err}");
                        None
                    }
                },
                Err(err) => {
                    warn!("Failed to query the latest revision: {err}");
                    None
                }
            };
            match latest_page_path {
                Some(path) => path,
                None => {
                    warn!("Will attempt to use the newest cached page");
                    let local_rev = get_latest_cached_revision(cache_dir).await?;
                    get_revision_path(cache_dir, local_rev)
                }
            }
        }
    };
    let page_str = fs::read_to_string(page_path).await?;

    Ok(page_str)
}
