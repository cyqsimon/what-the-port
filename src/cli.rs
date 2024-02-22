use std::{fmt, str::FromStr};

use clap::Parser;
use clap_verbosity_flag::{InfoLevel, Verbosity};

#[derive(Clone, Debug, Parser)]
#[command(author, version)]
pub struct CliArgs {
    #[arg(long = "link")]
    pub show_links: bool,

    #[arg(long = "reference", visible_alias = "ref")]
    pub show_references: bool,

    #[arg(short = 'j', long = "json")]
    pub json_output: bool,

    #[arg(index = 1, value_name = "PORT")]
    pub port: Port,

    #[command(flatten)]
    pub verbosity: Verbosity<InfoLevel>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Port {
    pub number: u16,
    pub protocol: SupportedProtocol,
}
impl fmt::Display for Port {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Port { number, protocol } = self;
        match protocol {
            SupportedProtocol::Any => write!(f, "{number}"),
            proto => write!(f, "{number}/{proto}"),
        }
    }
}
impl FromStr for Port {
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
