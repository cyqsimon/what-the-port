use std::path::{Path, PathBuf};

use color_eyre::eyre::OptionExt;
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

/// Get and cache a Wikipedia page from the network.
///
/// If a revision is absent, we query and fetch the newest revision.
///
/// Returns the path to and content of the cached page.
/// Errors if we encounter network problems, or if the revision is invalid.
pub async fn get_wikipedia_page_online(
    cache_dir: impl AsRef<Path>,
    client: &reqwest::Client,
    revision: Option<u64>,
) -> color_eyre::Result<(PathBuf, String)> {
    let cache_dir = cache_dir.as_ref();

    // get revision
    let revision = match revision {
        Some(rev) => rev,
        None => query_latest_revision(&client).await?,
    };

    // use cached if exists
    let page_path = get_revision_path(cache_dir, revision);
    if page_path.exists() {
        let content = fs::read_to_string(&page_path).await?;
        return Ok((page_path, content));
    }

    // fetch
    let url = format!("{PAGE_URL}?oldid={revision}");
    let content = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    // cache
    fs::create_dir_all(&cache_dir).await?;
    fs::write(&page_path, &content).await?;

    Ok((page_path, content))
}

/// Get the Wikipedia page with network disabled.
///
/// If a revision is absent, we return the newest available revision.
///
/// Returns the path to and content of the page.
/// Errors if the requested page is unavailable.
pub async fn get_wikipedia_page_offline(
    cache_dir: impl AsRef<Path>,
    revision: Option<u64>,
) -> color_eyre::Result<(PathBuf, String)> {
    let cache_dir = cache_dir.as_ref();

    let revision = match revision {
        Some(r) => r,
        None => get_latest_cached_revision(cache_dir).await?,
    };

    let page_path = get_revision_path(cache_dir, revision);
    let content = fs::read_to_string(&page_path).await?;

    Ok((page_path, content))
}
