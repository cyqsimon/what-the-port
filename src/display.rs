use std::fmt;

use itertools::Itertools;
use owo_colors::OwoColorize;
use serde::Serialize;

use crate::{
    cli::PortSelection,
    store::{PortCategory, PortRangeInfo, PortType},
};

/// Short-hand macro to conditionally colorize output.
macro_rules! color {
    ($item: expr, $fg: ident) => {
        $item
            .to_string()
            .if_supports_color(owo_colors::Stream::Stdout, |t| {
                t.fg::<owo_colors::colors::$fg>()
            })
    };
    ($item: expr, xterm::$fg: ident) => {
        $item
            .to_string()
            .if_supports_color(owo_colors::Stream::Stdout, |t| {
                t.fg::<owo_colors::colors::xterm::$fg>()
            })
    };
}

/// Structured output data, serialised into either human-readable or machine-readable form.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PortInfoOutput<'a> {
    pub port: PortSelection,
    pub category: PortCategory,
    pub use_cases: Vec<PortUseCase<'a>>,
}
impl<'a> fmt::Display for PortInfoOutput<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { port, category, use_cases } = self;

        if use_cases.is_empty() {
            return write!(
                f,
                "Port {p} is a {c} port with no known use cases",
                p = color!(port, Red),
                c = color!(category, Blue),
            );
        }

        let use_cases_str: String = use_cases
            .iter()
            .enumerate()
            .map(|(i, use_case)| format!("{}: {use_case}", i + 1))
            .join("\n");
        let count = use_cases.len();
        write!(
            f,
            "Port {p} is a {c} port with {count} known use {case_form}\n{use_cases_str}",
            p = color!(port, Green),
            c = color!(category, Blue),
            case_form = if count == 1 { "case" } else { "cases" },
        )
    }
}

/// A single use case for a user-specified port.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PortUseCase<'a> {
    #[serde(skip_serializing_if = "PortType::is_unused")]
    tcp: PortType,
    #[serde(skip_serializing_if = "PortType::is_unused")]
    udp: PortType,
    #[serde(skip_serializing_if = "PortType::is_unused")]
    sctp: PortType,
    #[serde(skip_serializing_if = "PortType::is_unused")]
    dccp: PortType,
    description: &'a str,
}
impl<'a> From<&'a PortRangeInfo> for PortUseCase<'a> {
    fn from(info: &'a PortRangeInfo) -> Self {
        let PortRangeInfo {
            tcp_type,
            udp_type,
            sctp_type,
            dccp_type,
            description,
            ..
        } = info;
        Self {
            tcp: *tcp_type,
            udp: *udp_type,
            sctp: *sctp_type,
            dccp: *dccp_type,
            description: description.as_str(),
        }
    }
}
impl<'a> fmt::Display for PortUseCase<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use PortType as T;
        let Self { tcp, udp, sctp, dccp, description } = self;

        let mut buf = vec![];

        macro_rules! push_proto {
            ($proto: ident, $label: expr) => {
                let proto_str = match $proto {
                    T::Unused => None, // skip
                    T::Yes => Some(format!("{}: {}", $label, color!($proto, Green))),
                    T::Unofficial => Some(format!("{}: {}", $label, color!($proto, Cyan))),
                    T::Assigned => Some(format!("{}: {}", $label, color!($proto, Yellow))),
                    T::No => Some(format!("{}: {}", $label, color!($proto, Red))),
                    T::Reserved => Some(format!("{}: {}", $label, color!($proto, xterm::Gray))),
                };
                if let Some(s) = proto_str {
                    buf.push(s);
                }
            };
        }

        push_proto!(tcp, "TCP");
        push_proto!(udp, "UDP");
        push_proto!(sctp, "SCTP");
        push_proto!(dccp, "DCCP");
        let protocol_types = buf.join(", ");

        write!(f, "{description}\n\t{protocol_types}")
    }
}
