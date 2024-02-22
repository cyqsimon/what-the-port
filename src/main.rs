use clap::Parser;
use color_eyre::eyre::OptionExt;
use tokio::fs;

use crate::{cli::CliArgs, parse::parse_page, update::cache_wikipedia_page};

mod cli;
mod display;
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

    // get paths
    let cache_dir = directories::ProjectDirs::from("org", "wtp", "what-the-port")
        .ok_or_eyre("Cannot determine your home directory")?
        .cache_dir()
        .to_owned();
    let cache_page_path = cache_dir.join("latest.html"); // IMPRV: don't hardcode this

    // cache
    if !cache_page_path.exists() {
        cache_wikipedia_page(&cache_dir, None).await?;
    }

    // parse
    let page = fs::read_to_string(cache_page_path).await?;
    let db = parse_page(&page)?;

    // query and print
    let output = db.query(port);
    let output_str = if json_output {
        serde_json::to_string(&output)?
    } else {
        output.to_string()
    };
    println!("{output_str}");

    Ok(())
}
