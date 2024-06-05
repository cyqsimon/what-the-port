use std::fmt;

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

/// Structured output data, serialised into either human-readable or machine-readable form.
#[derive(Clone, Debug, Serialize)]
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
            .map(|(i, use_case)| {
                format!("{}: {use_case}", i + 1)
                    .lines()
                    .map(|line| format!("    {line}")) // indentation
                    .join("\n")
            })
            .join("\n\n");
        let count = use_cases.len();
        write!(
            f,
            "Port {p} is a {c} port with {count} known use {case_form}\n\n{use_cases_str}",
            p = color!(port, Green),
            c = color!(category, Blue),
            case_form = if count == 1 { "case" } else { "cases" },
        )
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
