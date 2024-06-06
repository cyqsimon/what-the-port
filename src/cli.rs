use std::{fmt, str::FromStr};

use clap::Parser;
use clap_verbosity_flag::{InfoLevel, Verbosity};
use serde_with::SerializeDisplay;

#[derive(Clone, Debug, Parser)]
#[command(author, version)]
pub struct CliArgs {
    /// Which port would you like to learn about?
    ///
    /// You can either specify a port number, or a number-protocol pair.
    ///
    /// For example: `80`, `443/udp`.
    #[arg(index = 1, value_name = "PORT")]
    pub port: PortSelection,

    /// Which Wikipedia page revision you would like to use.
    ///
    /// If unspecified, we will try to fetch the latest.
    /// If that fails, we will use the latest revision from local cache.
    #[arg(long = "revision", visible_alias = "rev")]
    pub revision: Option<u64>,

    /// Disable all network access and only use local cache.
    #[arg(short = 'o', long = "offline")]
    pub offline: bool,

    /// Show links in the port description.
    #[arg(short = 'l', long = "links", alias = "link")]
    pub show_links: bool,

    /// Show notes and references in the port description.
    #[arg(short = 'r', long = "references", visible_aliases = ["refs", "notes"], aliases = ["reference", "ref", "note"])]
    pub show_notes_and_references: bool,

    /// Use machine-friendly JSON output.
    #[arg(short = 'j', long = "json")]
    pub json_output: bool,

    #[command(flatten)]
    pub verbosity: Verbosity<InfoLevel>,
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
