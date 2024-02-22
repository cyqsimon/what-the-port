use clap::Parser;
use color_eyre::eyre::OptionExt;
use simplelog::{ColorChoice, TermLogger, TerminalMode};
use tokio::fs;

use crate::{cli::CliArgs, parse::parse_page, update::get_wikipedia_page_cached};

mod cli;
mod display;
mod parse;
mod store;
mod update;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    let CliArgs {
        port,
        revision,
        show_links,
        show_references,
        json_output,
        verbosity,
    } = CliArgs::parse();

    // init logging
    let logger_config = simplelog::ConfigBuilder::new()
        .add_filter_ignore_str("html5ever")
        .add_filter_ignore_str("selectors")
        .build();
    TermLogger::init(
        verbosity.log_level_filter(),
        logger_config,
        TerminalMode::Stderr,
        ColorChoice::Auto,
    )?;

    // get paths
    let cache_dir = directories::ProjectDirs::from("org", "wtp", "what-the-port")
        .ok_or_eyre("Cannot determine your home directory")?
        .cache_dir()
        .to_owned();

    // get page
    let cache_page_path = get_wikipedia_page_cached(&cache_dir, revision).await?;

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
