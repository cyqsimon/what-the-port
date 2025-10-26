use std::{fmt, ops::RangeInclusive};

use itertools::Itertools;
use serde::Serialize;

use crate::{
    cli::PortSelection,
    consts::{ORIGIN_BASE_URL, PAGE_URL},
    parse::RichTextSpan,
    store::{PortCategory, PortRangeInfo, PortType},
};

/// Short-hand macro to colorize output.
macro_rules! color {
    ($item: expr, $fg: ident) => {{
        let style = yansi::Style::new().fg(yansi::Color::$fg);
        yansi::Paint::paint(&$item, style)
    }};
}

/// Short-hand macro to stylise linked text.
macro_rules! hyperlink {
    ($item: expr, $fg: ident, $url: expr) => {{
        let style = yansi::Style::new().fg(yansi::Color::$fg).italic();
        yansi::hyperlink::HyperlinkExt::link(&yansi::Paint::paint(&$item, style), $url)
    }};
}

/// All possible kinds of output, serialisable into either human-readable or
/// machine-readable form.
#[derive(Clone, Debug, derive_more::Display, derive_more::From, Serialize)]
#[serde(tag = "type", content = "result", rename_all = "kebab-case")]
pub enum Output<'a> {
    PortLookup(PortLookupOutput<'a>),
    Search(SearchOutput<'a>),
}

/// Structured output data in response to a port lookup.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct PortLookupOutput<'a> {
    pub lookup: PortSelection,
    pub matched: Option<MatchedPort<'a>>,
}
impl fmt::Display for PortLookupOutput<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let category = PortCategory::from(self.lookup.number);

        let Some(matched) = &self.matched else {
            return write!(
                f,
                "Port {p} is a {c} port with no known use cases",
                p = color!(self.lookup, Red),
                c = color!(category, Blue),
            );
        };

        let count = matched.use_cases.len();
        let use_cases_str = matched.format_use_cases(true, Some("    "), "\n");
        write!(
            f,
            "Port {p} is a {c} port with {count} known use {case_form}\n{use_cases_str}",
            p = color!(self.lookup, Green),
            c = color!(category, Blue),
            case_form = if count == 1 { "case" } else { "cases" },
        )?;

        let links = matched.format_links();
        if !links.is_empty() {
            let lines = links.iter().map(|line| format!("    {line}")).join("\n");
            write!(f, "\n\nLinks:\n{lines}")?;
        }

        let notes_and_refs = matched.format_notes_and_refs();
        if !notes_and_refs.is_empty() {
            let lines = notes_and_refs
                .iter()
                .map(|line| format!("    {line}"))
                .join("\n");
            write!(f, "\n\nNotes and References:\n{lines}")?;
        }

        Ok(())
    }
}

/// Structured output data in response to a general search.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct SearchOutput<'a> {
    pub search: String,
    pub matched: Vec<MatchedPort<'a>>,
}
impl fmt::Display for SearchOutput<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { search, matched } = self;

        if matched.is_empty() {
            return write!(f, "Found no matches for \"{search}\" among known ports");
        }

        let matched_str = matched
            .iter()
            .map(|p| {
                let category = PortCategory::from(*p.number.start());
                let case_count = p.use_cases.len();

                let subtitle = if p.number.clone().count() == 1 {
                    format!(
                        "Port {p} is a {c} port with {case_count} matched use {case_form}",
                        p = color!(p.number.start(), Green),
                        c = color!(category, Blue),
                        case_form = if case_count == 1 { "case" } else { "cases" },
                    )
                } else {
                    format!(
                        "Port {p} are {c} ports with {case_count} matched use {case_form}",
                        p = color!(format!("{}-{}", p.number.start(), p.number.end()), Green),
                        c = color!(category, Blue),
                        case_form = if case_count == 1 { "case" } else { "cases" },
                    )
                };
                let use_cases_str = p.format_use_cases(true, Some("    "), "\n");
                format!("{subtitle}\n{use_cases_str}")
            })
            .join("\n\n");
        let port_count = matched.len();
        let case_count = matched.iter().map(|p| p.use_cases.len()).sum::<usize>();

        write!(
            f,
            "Found {port_count} {port_form} with {case_count} use {case_form} matching \"{search}\"\n\n{matched_str}",
            port_form = if port_count == 1 {
                "port or port range"
            } else {
                "ports or port ranges"
            },
            case_form = if case_count == 1 { "case" } else { "cases" },
        )?;

        let links = matched
            .iter()
            .flat_map(MatchedPort::format_links)
            .collect_vec();
        if !links.is_empty() {
            let lines = links.iter().map(|line| format!("    {line}")).join("\n");
            write!(f, "\n\nLinks:\n{lines}")?;
        }

        let notes_and_refs = matched
            .iter()
            .flat_map(MatchedPort::format_notes_and_refs)
            .collect_vec();
        if !notes_and_refs.is_empty() {
            let lines = notes_and_refs
                .iter()
                .map(|line| format!("    {line}"))
                .join("\n");
            write!(f, "\n\nNotes and References:\n{lines}")?;
        }

        Ok(())
    }
}

/// Information on a matched port.
///
/// The parent struct implementation decides how to display this info.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct MatchedPort<'a> {
    pub number: RangeInclusive<u16>,
    pub use_cases: Vec<PortUseCase<'a>>,
}
impl MatchedPort<'_> {
    /// Format the use cases lines.
    ///
    /// Note that this does not include the optional sections.
    fn format_use_cases(
        &self,
        numbered: bool,
        indentation: Option<&str>,
        case_separator: &str,
    ) -> String {
        let indent = indentation.unwrap_or("");
        self.use_cases
            .iter()
            .enumerate()
            .map(|(i, use_case)| {
                let description = if numbered {
                    format!("{}: {}", i + 1, use_case.format_description())
                } else {
                    use_case.format_description()
                };
                format!(
                    "{indent}{description}\n{indent}{indent}{proto}",
                    proto = use_case.format_protocols()
                )
            })
            .join(case_separator)
    }

    /// Format lines of the optional link section.
    ///
    /// Each element contains its assigned link ID and line content.
    fn format_links(&self) -> Vec<String> {
        let links = self
            .use_cases
            .iter()
            .flat_map(PortUseCase::format_links)
            .collect_vec();
        links
    }

    /// Format lines of the optional notes and references section.
    fn format_notes_and_refs(&self) -> Vec<String> {
        self.use_cases
            .iter()
            .flat_map(PortUseCase::format_notes_and_refs)
            .collect()
    }
}

/// A single use case for a user-specified port.
///
/// This struct is intended for direct output, therefore the information about
/// stored in this struct should already be filtered on creation based on
/// user options.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct PortUseCase<'a> {
    #[serde(skip_serializing_if = "PortType::is_unused")]
    tcp: PortType,
    #[serde(skip_serializing_if = "PortType::is_unused")]
    udp: PortType,
    #[serde(skip_serializing_if = "PortType::is_unused")]
    sctp: PortType,
    #[serde(skip_serializing_if = "PortType::is_unused")]
    dccp: PortType,

    /// Description string formatted from rich description, depending on user options.
    description: String,
    /// Links extracted from rich description, depending on user options.
    ///
    /// Format: `(id, url)`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    links: Vec<(String, String)>,
    /// Notes and references extracted from rich description, depending on user options.
    ///
    /// Format: `(id, url)`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    notes_and_refs: Vec<(String, String)>,

    /// The full description parsed, as provided by Wikipedia.
    ///
    /// This is useful for JSON output.
    rich_description: &'a [RichTextSpan],
}
impl<'a> PortUseCase<'a> {
    /// Create an instance of [`PortUseCase`] by applying user options.
    ///
    /// `show_links` expects a starting index if links are to be shown.
    pub fn from_with_options(
        from: &'a PortRangeInfo,
        mut show_links: Option<usize>,
        show_notes_and_references: bool,
    ) -> Self {
        use RichTextSpan as Span;

        let mut description = String::new();
        let mut links = vec![];
        let mut notes_and_refs = vec![];

        for span in from.rich_description.iter() {
            match span {
                Span::Text { text } => {
                    description.push_str(text);
                }
                Span::SiteLink { text, link } => {
                    let url = format!("{ORIGIN_BASE_URL}{link}");
                    if let Some(idx) = show_links.as_mut() {
                        let tag = format!("[{idx}]");
                        *idx += 1;
                        description
                            .push_str(&hyperlink!(format!("{text}{tag}"), Cyan, &url).to_string());
                        links.push((color!(tag, Cyan).to_string(), url));
                    } else {
                        description.push_str(&hyperlink!(text, Cyan, &url).to_string());
                    }
                }
                Span::SiteLinkNonExistent { text, link } => {
                    let url = format!("{ORIGIN_BASE_URL}{link}");
                    if let Some(idx) = show_links.as_mut() {
                        let tag = format!("[{idx}]");
                        *idx += 1;
                        description
                            .push_str(&hyperlink!(format!("{text}{tag}"), Red, &url).to_string());
                        links.push((color!(tag, Red).to_string(), url));
                    } else {
                        description.push_str(&hyperlink!(text, Red, &url).to_string());
                    }
                }
                Span::ExternalLink { text, link } => {
                    let url = link.to_owned();
                    if let Some(idx) = show_links.as_mut() {
                        let tag = format!("[{idx}]");
                        *idx += 1;
                        description
                            .push_str(&hyperlink!(format!("{text}{tag}"), Cyan, &url).to_string());
                        links.push((color!(tag, Cyan).to_string(), url));
                    } else {
                        description.push_str(&hyperlink!(text, Cyan, &url).to_string());
                    }
                }
                Span::Note { number, note_id } => {
                    if show_notes_and_references {
                        let url = format!("{PAGE_URL}#{note_id}");
                        let tag = format!("[note {number}]");
                        description.push_str(&hyperlink!(tag, Yellow, &url).to_string());
                        notes_and_refs.push((color!(tag, Yellow).to_string(), url));
                    }
                }
                Span::Reference { number, ref_id } => {
                    if show_notes_and_references {
                        let url = format!("{PAGE_URL}#{ref_id}");
                        let tag = format!("[ref {number}]");
                        description.push_str(&hyperlink!(tag, Yellow, &url).to_string());
                        notes_and_refs.push((color!(tag, Yellow).to_string(), url));
                    }
                }
                Span::Annotation { text, link } => {
                    if show_notes_and_references {
                        let url = format!("{ORIGIN_BASE_URL}{link}");
                        // currently annotation text already contains delimiting brackets
                        let tag = text.clone();
                        description.push_str(&hyperlink!(tag, Yellow, &url).to_string());
                        notes_and_refs.push((color!(tag, Yellow).to_string(), url));
                    }
                }
                Span::Subscript { text } => {
                    description.push_str(&format!("_{{{text}}}")); // LaTeX syntax
                }
                Span::Unknown { text, err: _ } => {
                    description.push_str(text);
                }
            }
        }

        Self {
            tcp: from.tcp_type,
            udp: from.udp_type,
            sctp: from.sctp_type,
            dccp: from.dccp_type,
            description,
            links,
            notes_and_refs,
            rich_description: &from.rich_description,
        }
    }

    /// Return the number of stored links.
    ///
    /// Useful for accumulating the global link index.
    pub fn link_count(&self) -> usize {
        self.links.len()
    }

    /// Format the description line.
    fn format_description(&self) -> String {
        self.description.clone()
    }

    /// Format the protocol line.
    fn format_protocols(&self) -> String {
        use PortType as T;
        let Self { tcp, udp, sctp, dccp, .. } = self;

        let mut buf = vec![];
        macro_rules! push_proto {
            ($proto: ident, $label: expr) => {
                let proto_str = match $proto {
                    T::Unused => None, // skip
                    T::Yes => Some(format!("{}: {}", $label, color!($proto, Green))),
                    T::Unofficial => Some(format!("{}: {}", $label, color!($proto, Cyan))),
                    T::Assigned => Some(format!("{}: {}", $label, color!($proto, Yellow))),
                    T::No => Some(format!("{}: {}", $label, color!($proto, Red))),
                    T::Reserved => Some(format!("{}: {}", $label, color!($proto, BrightBlack))),
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

        buf.join(", ")
    }

    /// Format lines of the optional link section.
    fn format_links(&self) -> Vec<String> {
        let links = self
            .links
            .iter()
            .map(|(id, url)| format!("{id}: {url}"))
            .collect_vec();
        links
    }

    /// Format lines of the optional notes and references section.
    fn format_notes_and_refs(&self) -> Vec<String> {
        self.notes_and_refs
            .iter()
            .map(|(id, url)| format!("{id}: {url}"))
            .collect()
    }
}
