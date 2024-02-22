use std::path::{Path, PathBuf};

use color_eyre::eyre::OptionExt;
use log::warn;
use serde::Deserialize;
use tokio::fs;

const HISTORY_API_URL: &str =
    "https://api.wikimedia.org/core/v1/wikipedia/en/page/List_of_TCP_and_UDP_port_numbers/history";
const PAGE_URL: &str =
    "https://en.wikipedia.org/w/index.php?title=List_of_TCP_and_UDP_port_numbers";

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
pub async fn query_latest_revision() -> color_eyre::Result<u64> {
    let list: RevisionList = reqwest::get(HISTORY_API_URL)
        .await?
        .error_for_status()?
        .json()
        .await?;
    let latest = list.0.get(0).ok_or_eyre("Revision history is empty")?;
    Ok(*latest)
}

/// Get the latest cached revision.
pub async fn get_latest_cached_revision(cache_dir: impl AsRef<Path>) -> color_eyre::Result<u64> {
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

/// Get the Wikipedia page and cache it. Use cache if available.
///
/// If the revision is omitted, the latest revision is checked and used.
///
/// Returns the path to the cached page.
pub async fn get_wikipedia_page_cached(
    cache_dir: impl AsRef<Path>,
    revision: Option<u64>,
) -> color_eyre::Result<PathBuf> {
    let cache_dir = cache_dir.as_ref();

    // get revision
    let rev_id = match revision {
        Some(r) => r,
        None => match query_latest_revision().await {
            Ok(r) => r,
            Err(err) => {
                warn!("Failed to query the latest revision: {err}");
                warn!("Will attempt to use the newest cached page");
                get_latest_cached_revision(cache_dir).await?
            }
        },
    };

    // use cached if exists
    let page_path = cache_dir.join(format!("{rev_id}.html"));
    if page_path.exists() {
        return Ok(page_path);
    }

    // fetch
    let url = format!("{PAGE_URL}&oldid={rev_id}");
    let page_bytes = reqwest::get(url).await?.error_for_status()?.bytes().await?;

    // cache
    fs::create_dir_all(&cache_dir).await?;
    fs::write(&page_path, page_bytes).await?;

    Ok(page_path)
}
