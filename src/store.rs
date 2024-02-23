use std::ops::RangeInclusive;

use color_eyre::eyre::bail;
use itertools::Itertools;
use scraper::ElementRef;
use serde::Serialize;

use crate::{
    cli::{PortSelection, SupportedProtocol},
    display::PortInfoOutput,
};

/// The type of port, as classified by Wikipedia.
#[derive(Copy, Clone, Debug, PartialEq, Eq, strum::Display, Serialize)]
#[strum(serialize_all = "kebab-case")]
#[serde(rename_all = "kebab-case")]
pub enum PortCategory {
    /// 0 to 1023.
    WellKnown,
    /// 1024 to 49151 (2^10 to 2^14 + 2^15 - 1).
    Registered,
    /// 49152 to 65535 (2^15 + 2^14 to 2^16 - 1).
    Dynamic,
}
impl From<u16> for PortCategory {
    fn from(port: u16) -> Self {
        use PortCategory as C;
        match port {
            0..=1023 => C::WellKnown,
            1024..=49151 => C::Registered,
            49152..=65535 => C::Dynamic,
        }
    }
}
impl TryFrom<&RangeInclusive<u16>> for PortCategory {
    type Error = color_eyre::Report;

    fn try_from(range: &RangeInclusive<u16>) -> color_eyre::Result<Self> {
        let start: Self = (*range.start()).into();
        let end: Self = (*range.end()).into();
        if start != end {
            bail!(
                r#"Port range "{}-{}" crossed a category border"#,
                range.start(),
                range.end(),
            )
        }
        Ok(start)
    }
}

/// The port type as listed by Wikipedia.
#[derive(Copy, Clone, Debug, PartialEq, Eq, strum::Display, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PortType {
    /// Described protocol is not used.
    Unused,
    /// Described protocol is assigned by IANA for this port, and is: standardized, specified, or widely used for such.
    Yes,
    /// Described protocol is not assigned by IANA for this port, but is: standardized, specified, or widely used for such.
    Unofficial,
    /// Described protocol is assigned by IANA for this port, but is not: standardized, specified, or widely used for such.
    Assigned,
    /// Described protocol is not: assigned by IANA for this port, standardized, specified, or widely used for such.
    No,
    /// Port is reserved by IANA, generally to prevent collision having its previous use removed. The port number may be available for assignment upon request to IANA.
    Reserved,
}
impl TryFrom<ElementRef<'_>> for PortType {
    type Error = color_eyre::Report;

    fn try_from(cell: ElementRef<'_>) -> color_eyre::Result<Self> {
        // sanity check
        if cell.value().name() != "td" {
            bail!("A port type cell should be a `td` element");
        }

        for txt in cell.text() {
            let matched = match txt.trim() {
                "Yes" => Some(Self::Yes),
                "Unofficial" => Some(Self::Unofficial),
                "Assigned" => Some(Self::Assigned),
                "No" => Some(Self::No),
                "Reserved" => Some(Self::Reserved),
                _ => None, // ignore
            };
            if let Some(t) = matched {
                return Ok(t);
            }
        }
        Ok(Self::Unused)
    }
}
impl PortType {
    pub fn is_unused(&self) -> bool {
        matches!(self, Self::Unused)
    }
}

/// Records a use case of a range of ports.
///
/// There may be multiple use cases for the same range of ports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortRangeInfo {
    pub number: RangeInclusive<u16>,
    pub category: PortCategory,
    pub tcp_type: PortType,
    pub udp_type: PortType,
    pub sctp_type: PortType,
    pub dccp_type: PortType,
    pub description: String,
}
impl PortRangeInfo {
    /// Whether this port matches the user's requested port and should be shown.
    pub fn matches_request(&self, req: PortSelection) -> bool {
        use SupportedProtocol as P;

        if !self.number.contains(&req.number) {
            return false;
        }
        match req.protocol {
            P::Any => true,
            P::Tcp => !self.tcp_type.is_unused(),
            P::Udp => !self.udp_type.is_unused(),
            P::Sctp => !self.sctp_type.is_unused(),
            P::Dccp => !self.dccp_type.is_unused(),
        }
    }
}

/// Records all known use cases for all known ports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortDatabase(pub Vec<PortRangeInfo>);
impl PortDatabase {
    pub fn query(&self, req: PortSelection) -> PortInfoOutput<'_> {
        let category = req.number.into();
        let use_cases = self
            .0
            .iter()
            .filter(|p| p.matches_request(req))
            .map_into()
            .collect();

        PortInfoOutput { port: req, category, use_cases }
    }
}
