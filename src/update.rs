use std::{borrow::Cow, path::Path};

use color_eyre::eyre::OptionExt;
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

/// Fetch and cache the Wikipedia page, optionally specifying a revision.
///
/// If the revision is omitted, the latest revision is used.
pub async fn cache_wikipedia_page(
    cache_dir: impl AsRef<Path>,
    revision: Option<u64>,
) -> color_eyre::Result<()> {
    let cache_dir = cache_dir.as_ref();

    // fetch
    let url = match revision {
        Some(rev) => Cow::Owned(format!("{PAGE_URL}&oldid={rev}")),
        None => Cow::Borrowed(PAGE_URL),
    };
    let page_bytes = reqwest::get(url.as_ref())
        .await?
        .error_for_status()?
        .bytes()
        .await?;

    // cache
    fs::create_dir_all(&cache_dir).await?;

    let rev_id = match revision {
        Some(rev) => rev.to_string(),
        None => "latest".to_string(),
    };
    fs::write(cache_dir.join(format!("{rev_id}.html")), page_bytes).await?;

    Ok(())
}
