use std::{convert::Infallible, fmt, str::FromStr};

use clap::Parser;
use clap_verbosity_flag::{InfoLevel, Verbosity};
use serde_with::SerializeDisplay;

#[derive(Clone, Debug, Parser)]
#[command(author, version)]
pub struct CliArgs {
    /// Plain text search term or a port specification.
    ///
    /// ## Port specification
    /// - either a port number: `80`
    /// - or a number-protocol pair: `443/udp`
    #[arg(index = 1, value_name = "QUERY")]
    pub query: UserQuery,

    /// Which Wikipedia page revision you would like to use.
    ///
    /// If unspecified, use the latest revision from either online or local cache,
    /// depending on whether `--pull` is used.
    #[arg(long = "revision", visible_alias = "rev")]
    pub revision: Option<u64>,

    /// Attempt to retrieve revisions from Wikipedia.
    ///
    /// If `--revision` is unspecified, this will pull the latest revision.
    #[arg(short = 'p', long = "pull", visible_alias = "online")]
    pub pull: bool,

    /// Show an additional link section.
    ///
    /// Note: when outputting to TTY, inline hyperlinks are always available
    /// regardless of this option. This option is most useful when redirecting
    /// output to a file, or when your terminal does not support OSC8.
    #[arg(short = 'l', long = "links", alias = "link")]
    pub show_links: bool,

    /// Show notes and references in the port description.
    ///
    /// Note: in contrast to links, notes and references will not be shown inline
    /// without this option.
    #[arg(short = 'r', long = "references", visible_aliases = ["refs", "notes"], aliases = ["reference", "ref", "note"])]
    pub show_notes_and_references: bool,

    /// Use machine-friendly JSON output.
    #[arg(short = 'j', long = "json")]
    pub json_output: bool,

    #[command(flatten)]
    pub verbosity: Verbosity<InfoLevel>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UserQuery {
    /// User specified a search term.
    Search(String),
    /// User specified a port lookup.
    PortLookup(PortSelection),
}
impl fmt::Display for UserQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Search(s) => write!(f, r#"Search: "{s}""#),
            Self::PortLookup(port) => write!(f, "{port}"),
        }
    }
}
impl FromStr for UserQuery {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let query = if let Ok(port) = s.parse() {
            Self::PortLookup(port)
        } else {
            Self::Search(s.into())
        };
        Ok(query)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, SerializeDisplay)]
pub struct PortSelection {
    pub number: u16,
    pub protocol: SupportedProtocol,
}
impl fmt::Display for PortSelection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let PortSelection { number, protocol } = self;
        match protocol {
            SupportedProtocol::Any => write!(f, "{number}"),
            proto => write!(f, "{number}/{proto}"),
        }
    }
}
impl FromStr for PortSelection {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (number_str, protocol) = match s.split_once('/') {
            Some((n, p)) => {
                let proto = p
                    .parse()
                    .map_err(|_| format!(r#"Unknown protocol: "{p}""#))?;
                (n, proto)
            }
            None => (s, SupportedProtocol::Any),
        };
        let number = number_str
            .parse()
            .map_err(|_| format!(r#""{number_str}" is not a valid port number"#))?;
        Ok(Self { number, protocol })
    }
}

/// Known port protocols.
#[derive(Copy, Clone, Debug, PartialEq, Eq, strum::Display, strum::EnumString)]
#[strum(serialize_all = "lowercase")]
pub enum SupportedProtocol {
    /// Protocol unspecified.
    Any,
    /// Transmission Control Protocol.
    Tcp,
    /// User Datagram Protocol.
    Udp,
    /// Stream Control Transmission Protocol.
    Sctp,
    /// Datagram Congestion Control Protocol.
    Dccp,
}
