use clap::Parser;
use color_eyre::eyre::OptionExt;
use itertools::Itertools;
use tokio::fs;

use crate::{cli::CliArgs, parse::parse_page, update::cache_wikipedia_page};

mod cli;
mod parse;
mod store;
mod update;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    let CliArgs {
        show_links,
        show_references,
        json_output,
        port,
        verbosity,
    } = CliArgs::parse();

    let cache_dir = directories::ProjectDirs::from("org", "wtp", "what-the-port")
        .ok_or_eyre("Cannot determine your home directory")?
        .cache_dir()
        .to_owned();
    let cache_page_path = cache_dir.join("latest.html"); // IMPRV: don't hardcode this

    if !cache_page_path.exists() {
        cache_wikipedia_page(&cache_dir, None).await?;
    }

    let page = fs::read_to_string(cache_page_path).await?;
    let list = parse_page(&page)?;

    let filtered = list
        .iter()
        .filter(|p| p.matches_request(port))
        .collect_vec();
    dbg!(&filtered);

    Ok(())
}
