use std::{fmt, ops::RangeInclusive};

use itertools::Itertools;
use owo_colors::OwoColorize;
use serde::Serialize;

use crate::{
    cli::PortSelection,
    consts::{ORIGIN_BASE_URL, PAGE_URL},
    parse::RichTextSpan,
    store::{PortCategory, PortRangeInfo, PortType},
};

/// Short-hand macro to conditionally colorize output.
macro_rules! color {
    ($item: expr, $fg: ident) => {{
        let style = owo_colors::Style::new().fg::<owo_colors::colors::$fg>();
        $item.if_supports_color(owo_colors::Stream::Stdout, move |t| t.style(style))
    }};
    ($item: expr, xterm::$fg: ident) => {{
        let style = owo_colors::Style::new().fg::<owo_colors::colors::xterm::$fg>();
        $item.if_supports_color(owo_colors::Stream::Stdout, move |t| t.style(style))
    }};
}

/// Short-hand macro to conditionally stylise linked text.
macro_rules! style_linked_text {
    ($item: expr, $fg: ident) => {{
        let style = owo_colors::Style::new()
            .fg::<owo_colors::colors::$fg>()
            .italic();
        $item.if_supports_color(owo_colors::Stream::Stdout, move |t| t.style(style))
    }};
}

/// All possible kinds of output, serialisable into either human-readable or
/// machine-readable form.
#[derive(Clone, Debug, derive_more::Display, derive_more::From, Serialize)]
#[serde(tag = "type", content = "result")]
pub enum Output<'a> {
    Search(SearchOutput<'a>),
    PortLookup(PortLookupOutput<'a>),
}

/// Structured output data in response to a general search.
#[derive(Clone, Debug, Serialize)]
pub struct SearchOutput<'a> {
    pub search: String,
    pub matched: Vec<MatchedPort<'a>>,
}
impl<'a> fmt::Display for SearchOutput<'a> {
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
                        "Port {p} is a {c} port with {case_count} known use {case_form}",
                        p = color!(p.number.start(), Green),
                        c = color!(category, Blue),
                        case_form = if case_count == 1 { "case" } else { "cases" },
                    )
                } else {
                    format!(
                        "Port {p} are {c} ports with {case_count} known use {case_form}",
                        p = color!(format!("{}-{}", p.number.start(), p.number.end()), Green),
                        c = color!(category, Blue),
                        case_form = if case_count == 1 { "case" } else { "cases" },
                    )
                };
                let use_cases_str = p.as_use_cases_str(true, Some("    "), "\n");
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
        )
    }
}

/// Structured output data in response to a port lookup.
#[derive(Clone, Debug, Serialize)]
pub struct PortLookupOutput<'a> {
    pub lookup: PortSelection,
    pub matched: Option<MatchedPort<'a>>,
}
impl<'a> fmt::Display for PortLookupOutput<'a> {
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

        let use_cases_str = matched.as_use_cases_str(true, Some("    "), "\n");
        let count = matched.use_cases.len();
        write!(
            f,
            "Port {p} is a {c} port with {count} known use {case_form}\n{use_cases_str}",
            p = color!(self.lookup, Green),
            c = color!(category, Blue),
            case_form = if count == 1 { "case" } else { "cases" },
        )
    }
}

/// Information on a matched port.
///
/// The parent struct implementation decides how to display this info.
#[derive(Clone, Debug, Serialize)]
pub struct MatchedPort<'a> {
    pub number: RangeInclusive<u16>,
    pub use_cases: Vec<PortUseCase<'a>>,
}
impl<'a> MatchedPort<'a> {
    fn as_use_cases_str(
        &self,
        numbered: bool,
        indentation: Option<&str>,
        case_separator: &str,
    ) -> String {
        self.use_cases
            .iter()
            .enumerate()
            .map(|(i, use_case)| {
                let s = if numbered {
                    format!("{}: {use_case}", i + 1)
                } else {
                    use_case.to_string()
                };
                match indentation {
                    Some(indent) => s.lines().map(|line| format!("{indent}{line}")).join("\n"),
                    None => s,
                }
            })
            .join(case_separator)
    }
}

/// A single use case for a user-specified port.
#[derive(Clone, Debug, Serialize)]
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
impl<'a> fmt::Display for PortUseCase<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use PortType as T;
        let Self {
            tcp,
            udp,
            sctp,
            dccp,
            description,
            links,
            notes_and_refs,
            rich_description: _,
        } = self;
        // description line
        write!(f, "{description}")?;

        // protocol line
        let mut protocol_buf = vec![];
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
                    protocol_buf.push(s);
                }
            };
        }
        push_proto!(tcp, "TCP");
        push_proto!(udp, "UDP");
        push_proto!(sctp, "SCTP");
        push_proto!(dccp, "DCCP");
        write!(f, "\n    {p}", p = protocol_buf.join(", "))?;

        // optional sections
        macro_rules! write_list {
            ($title: literal, $list: ident) => {
                if !$list.is_empty() {
                    let s = $list
                        .iter()
                        .map(|(id, url)| format!("    {id}: {url}"))
                        .join("\n");
                    write!(f, "\n{t}\n{s}", t = $title)?;
                }
            };
        }
        write_list!("Links:", links);
        write_list!("Notes and References:", notes_and_refs);

        Ok(())
    }
}
impl<'a> PortUseCase<'a> {
    pub fn from_with_options(
        from: &'a PortRangeInfo,
        show_links: bool,
        show_notes_and_references: bool,
    ) -> Self {
        use RichTextSpan as Span;

        let mut description = String::new();
        let mut links = vec![];
        let mut notes_and_refs = vec![];

        let mut link_idx = 1usize;

        for span in from.rich_description.iter() {
            match span {
                Span::Text { text } => {
                    description.push_str(text);
                }
                Span::SiteLink { text, link } => {
                    if show_links {
                        let tag = color!(format!("[{link_idx}]"), Cyan).to_string();
                        link_idx += 1;

                        description.push_str(&style_linked_text!(text, Cyan).to_string());
                        description.push_str(&tag);
                        links.push((tag, format!("{ORIGIN_BASE_URL}{link}")));
                    } else {
                        description.push_str(text);
                    }
                }
                Span::SiteLinkNonExistent { text, link } => {
                    if show_links {
                        let tag = color!(format!("[{link_idx}]"), Red).to_string();
                        link_idx += 1;

                        description.push_str(&style_linked_text!(text, Red).to_string());
                        description.push_str(&tag);
                        links.push((tag, format!("{ORIGIN_BASE_URL}{link}")));
                    } else {
                        description.push_str(text);
                    }
                }
                Span::ExternalLink { text, link } => {
                    if show_links {
                        let tag = color!(format!("[{link_idx}]"), Cyan).to_string();
                        link_idx += 1;

                        description.push_str(&style_linked_text!(text, Cyan).to_string());
                        description.push_str(&tag);
                        links.push((tag, link.to_owned()));
                    } else {
                        description.push_str(text);
                    }
                }
                Span::Note { number, note_id } => {
                    if show_notes_and_references {
                        let tag = color!(format!("[note {number}]"), Yellow).to_string();
                        description.push_str(&tag);
                        notes_and_refs.push((tag, format!("{PAGE_URL}#{note_id}")));
                    }
                }
                Span::Reference { number, ref_id } => {
                    if show_notes_and_references {
                        let tag = color!(format!("[ref {number}]"), Yellow).to_string();
                        description.push_str(&tag);
                        notes_and_refs.push((tag, format!("{PAGE_URL}#{ref_id}")));
                    }
                }
                Span::Annotation { text, link } => {
                    if show_notes_and_references {
                        let tag = color!(format!("{text}"), Yellow).to_string();
                        description.push_str(&tag);
                        notes_and_refs.push((tag, format!("{ORIGIN_BASE_URL}{link}")));
                    }
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
}
