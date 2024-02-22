use std::fmt;

use itertools::Itertools;
use serde::Serialize;

use crate::{
    cli::PortSelection,
    store::{PortCategory, PortRangeInfo, PortType},
};

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
            return write!(f, "{port} is a {category} port with no known use cases");
        }

        let use_cases_str: String = use_cases
            .iter()
            .enumerate()
            .map(|(i, use_case)| format!("{}: {use_case}", i + 1))
            .join("\n");
        let count = use_cases.len();
        write!(
            f,
            "{port} is a {category} port with {count} known use {case_form}\n{use_cases_str}",
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
        let Self { tcp, udp, sctp, dccp, description } = self;

        let mut buf = vec![];
        if let Some(txt) = tcp.opt_show("TCP") {
            buf.push(txt);
        }
        if let Some(txt) = udp.opt_show("UDP") {
            buf.push(txt);
        }
        if let Some(txt) = sctp.opt_show("SCTP") {
            buf.push(txt);
        }
        if let Some(txt) = dccp.opt_show("DCCP") {
            buf.push(txt);
        }
        let protocol_types = buf.join(", ");

        write!(f, "{description}\n\t{protocol_types}")
    }
}
