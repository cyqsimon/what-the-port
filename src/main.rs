use std::time::Duration;

use clap::Parser;
use color_eyre::eyre::OptionExt;
use simplelog::{ColorChoice, TermLogger, TerminalMode};

use crate::{
    cli::{CliArgs, UserQuery},
    display::Output,
    parse::parse_page,
    source::{get_wikipedia_page_offline, get_wikipedia_page_online},
};

mod cli;
mod consts;
mod display;
mod parse;
mod source;
mod store;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    let CliArgs {
        query,
        revision,
        pull,
        show_links,
        show_notes_and_references,
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
    let (_page_path, page) = if pull {
        let client = reqwest::ClientBuilder::new()
            .connection_verbose(true)
            .timeout(Duration::from_secs(10))
            .build()?;
        get_wikipedia_page_online(&cache_dir, &client, revision).await?
    } else {
        get_wikipedia_page_offline(&cache_dir, revision).await?
    };

    // parse
    let db = parse_page(&page)?;

    // set conditional colourisation
    yansi::whenever(yansi::Condition::TTY_AND_COLOR);

    // query and print
    let output: Output = match query {
        UserQuery::Search(search) => db
            .search(search, show_links, show_notes_and_references)
            .into(),
        UserQuery::PortLookup(port) => db
            .lookup(port, show_links, show_notes_and_references)
            .into(),
    };
    let output_str = if json_output {
        serde_json::to_string(&output)?
    } else {
        output.to_string()
    };
    println!("{output_str}");

    Ok(())
}
