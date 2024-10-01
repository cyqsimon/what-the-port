use std::ops::RangeInclusive;

use color_eyre::eyre::bail;
use itertools::Itertools;
use scraper::ElementRef;
use serde::Serialize;

use crate::{
    cli::{PortSelection, SupportedProtocol},
    display::{MatchedPort, PortLookupOutput, PortUseCase, SearchOutput},
    parse::RichTextSpan,
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
#[derive(Clone, Debug)]
pub struct PortRangeInfo {
    pub number: RangeInclusive<u16>,
    pub tcp_type: PortType,
    pub udp_type: PortType,
    pub sctp_type: PortType,
    pub dccp_type: PortType,
    pub rich_description: Vec<RichTextSpan>,
}
impl PortRangeInfo {
    /// Whether this port matches the user's requested port and should be shown.
    pub fn matches_port(&self, lookup: PortSelection) -> bool {
        use SupportedProtocol as P;

        if !self.number.contains(&lookup.number) {
            return false;
        }
        match lookup.protocol {
            P::Any => true,
            P::Tcp => !self.tcp_type.is_unused(),
            P::Udp => !self.udp_type.is_unused(),
            P::Sctp => !self.sctp_type.is_unused(),
            P::Dccp => !self.dccp_type.is_unused(),
        }
    }

    /// Check if the description contains the search term.
    ///
    /// This match is case-insensitive.
    pub fn matches_search(
        &self,
        search: impl AsRef<str>,
        include_links: bool,
        include_notes_and_references: bool,
    ) -> bool {
        let search = search.as_ref().to_lowercase();

        // matched if any individual span contains the search term
        if self
            .rich_description
            .iter()
            .any(|span| span.matches_search(&search, include_links, include_notes_and_references))
        {
            return true;
        }

        // matched if the concatenated normal text contains the search term
        // this is necessary because a search term could ride on span boundaries
        // e.g. `foo bar` should match `foo [bar](example.org) baz`
        let text = self
            .rich_description
            .iter()
            .filter_map(RichTextSpan::normal_text)
            .map(str::to_lowercase)
            .join("");
        if text.contains(&search) {
            return true;
        }

        false
    }
}

/// Records all known use cases for all known ports.
#[derive(Clone, Debug)]
pub struct PortDatabase(pub Vec<PortRangeInfo>);
impl PortDatabase {
    pub fn lookup(
        &self,
        lookup: PortSelection,
        show_links: bool,
        show_notes_and_references: bool,
    ) -> PortLookupOutput {
        let use_cases = self
            .0
            .iter()
            .filter(|p| p.matches_port(lookup))
            .map(|p| PortUseCase::from_with_options(p, show_links, show_notes_and_references))
            .collect_vec();

        // note that these use cases may come from different port ranges
        // because ranges may overlap
        // e.g. revision 1248795838, port 3479

        let matched = if use_cases.is_empty() {
            None
        } else {
            Some(MatchedPort {
                number: lookup.number..=lookup.number,
                use_cases,
            })
        };
        PortLookupOutput { lookup, matched }
    }

    pub fn search(
        &self,
        search: impl AsRef<str>,
        show_links: bool,
        show_notes_and_references: bool,
    ) -> SearchOutput {
        let search = search.as_ref().to_owned();

        let matched = self
            .0
            .iter()
            .filter(|p| p.matches_search(&search, show_links, show_notes_and_references))
            .into_group_map_by(|p| &p.number)
            .into_iter()
            .map(|(n, info)| {
                let use_cases = info
                    .into_iter()
                    .map(|p| {
                        PortUseCase::from_with_options(p, show_links, show_notes_and_references)
                    })
                    .collect();
                MatchedPort { number: n.clone(), use_cases }
            })
            .sorted_by_key(|p| *p.number.start())
            .collect();

        SearchOutput { search, matched }
    }
}
