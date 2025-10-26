use std::{
    ops::{Deref, RangeInclusive},
    sync::Arc,
};

use color_eyre::eyre::{bail, OptionExt};
use ego_tree::NodeRef;
use itertools::Itertools;
use log::{trace, warn};
use once_cell::sync::Lazy;
use regex::Regex;
use scraper::{node::Element, CaseSensitivity, ElementRef, Html, Node, Selector};
use serde::Serialize;
use serde_with::{serde_as, DisplayFromStr};

use crate::store::{PortDatabase, PortRangeInfo, PortType};

/// Parse the Wikipedia port list page from its HTML source.
pub fn parse_page(html_str: &str) -> color_eyre::Result<PortDatabase> {
    let document = Html::parse_document(html_str);

    let table_selector = Selector::parse(".wikitable.sortable").unwrap();

    let list = document
        .select(&table_selector)
        .map(|table| parse_table(table))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect_vec();

    Ok(PortDatabase(list))
}

/// Parse a table that contains a list of ports with their descriptions.
fn parse_table(table: ElementRef<'_>) -> color_eyre::Result<Vec<PortRangeInfo>> {
    // sanity check
    if table.value().name() != "table" {
        bail!("A port table should be a `table` element")
    }

    let cell_selector = Selector::parse("td").unwrap();
    let row_selector = Selector::parse("tbody>tr").unwrap();

    let mut list = vec![];

    let mut rows = table.select(&row_selector).peekable();

    // the first row could be the header (contains only `th`s)
    if rows
        .peek()
        .ok_or_eyre("Table has 0 rows")?
        .select(&cell_selector)
        .next()
        .is_none()
    {
        // if so, ignore the first row
        let _ = rows.next();
    }

    // parse all rows
    while let Some(row) = rows.next() {
        let mut cells = row.select(&cell_selector).collect_vec().into_iter();

        // parse port range
        let range_cell = cells.next().ok_or_eyre("Encountered an empty row")?;
        let (range, span) = parse_port_range(range_cell)?;

        // parse this row
        let info = parse_row_info(range.clone(), cells)?;
        list.push(info);

        // parse subsequent rows in multi-row case
        for _ in 1..span {
            let row = rows
                .next()
                .ok_or_eyre("No more rows while parsing a multi-row port")?;
            let cells = row.select(&cell_selector).collect_vec().into_iter();
            let info = parse_row_info(range.clone(), cells)?;
            list.push(info);
        }
    }

    Ok(list)
}

/// Parse a cell that contains the port range, in the first column of the table.
///
/// Returns the port range and the row span in a tuple.
fn parse_port_range(cell: ElementRef<'_>) -> color_eyre::Result<(RangeInclusive<u16>, usize)> {
    // sanity check
    if cell.value().name() != "td" {
        bail!("A port range cell should be a `td` element");
    }

    let span = match cell.attr("rowspan") {
        Some(n) => n.parse()?,
        None => 1,
    };

    let sanitised_range_str = cell
        .children()
        // remove superscripts
        .filter(|n| {
            if let Node::Element(el) = n.value() {
                el.name() != "sup"
            } else {
                true
            }
        })
        .map(|n| get_text_from_node(&n, true))
        .collect::<String>();
    trace!("parsing {sanitised_range_str}");
    let port_range = match sanitised_range_str.split_once(['-', '–']) {
        Some((start, end)) => {
            let start = start.parse()?;
            let end = end.parse()?;
            start..=end
        }
        None => {
            let port = sanitised_range_str.parse()?;
            port..=port
        }
    };

    Ok((port_range, span))
}

/// Parse a row excluding the cell that contains the port range. The port range
/// parsing is handled by [`parse_port_range`] separately because there are cases
/// where a port has multiple uses and therefore has multiple rows.
fn parse_row_info<'a, I>(
    port_range: RangeInclusive<u16>,
    mut cells: I,
) -> color_eyre::Result<PortRangeInfo>
where
    I: DoubleEndedIterator<Item = ElementRef<'a>>,
{
    // old implementation was to read the last cell as description
    // and use the remaining cells as port type
    // but this approach does not handle extraneous cells well
    // see revision 1248795838, port 9876

    // TCP, UDP, SCTP, DCCP
    let mut port_types = [PortType::Unused; 4];
    let mut types_it = port_types.iter_mut();
    let mut span_count_sum = 0usize;
    while span_count_sum < 4 {
        let cell = cells
            .next()
            .ok_or_eyre("Ran out of port type cells before they span 4")?;
        let span = match cell.attr("colspan") {
            Some(n) => n.parse()?,
            None => 1,
        };
        span_count_sum += span;
        let type_ = cell.try_into()?;
        for _ in 0..span {
            *types_it.next().ok_or_eyre("Port type cells span > 4")? = type_;
        }
    }

    // description
    let description_cell = cells.next().ok_or_eyre("Row has no description cell")?;
    let rich_description = parse_rich_text_cell(description_cell)?;

    Ok(PortRangeInfo {
        number: port_range,
        tcp_type: port_types[0],
        udp_type: port_types[1],
        sctp_type: port_types[2],
        dccp_type: port_types[3],
        rich_description,
    })
}

/// All known kinds of content in a rich text cell.
///
/// A cell may contain multiple different kinds concatenated together.
#[serde_as]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RichTextSpan {
    /// Plain text segment.
    Text { text: String },
    /// A link to somewhere within the same origin.
    SiteLink { text: String, link: String },
    /// A link to somewhere within the same origin that does not yet exist.
    SiteLinkNonExistent { text: String, link: String },
    /// A link to somewhere external.
    ExternalLink { text: String, link: String },
    /// A link to a note in superscript, e.g. `[note 1]`.
    ///
    /// Always an ID on the same page.
    Note { number: usize, note_id: String },
    /// A link to a reference in superscript, e.g. `[69]`.
    ///
    /// Always an ID on the same page.
    Reference { number: usize, ref_id: String },
    /// A link to an annotation in superscript, e.g. `[jargon]`.
    ///
    /// Always a site link.
    Annotation { text: String, link: String },
    /// A span of unknown type.
    Unknown {
        text: String,
        #[serde_as(as = "DisplayFromStr")]
        // we use `Arc` here so that we can `#[derive(Clone)]`
        err: Arc<color_eyre::Report>,
    },
}
impl RichTextSpan {
    fn parse(node: NodeRef<Node>) -> Vec<Self> {
        use CaseSensitivity::CaseSensitive as Cased;
        use RichTextSpan as Span;

        /// Helper function to simplify error handling.
        ///
        /// - Returns an empty list if the node is a known type but should be ignored.
        /// - Returns a one-item list in most scenarios.
        /// - May return a multi-item list when we need to recurse.
        /// - Returns `Err(...)` if the node is unknown or is known but has unexpected format.
        fn parse_impl(node: NodeRef<Node>) -> color_eyre::Result<Vec<Span>> {
            let span =
                match node.value() {
                    n @ Node::Document
                    | n @ Node::Fragment
                    | n @ Node::Doctype(_)
                    | n @ Node::ProcessingInstruction(_) => {
                        bail!("Encountered an unexpected node: {n:?}")
                    }
                    // ignore comment
                    Node::Comment(_) => vec![],
                    // plain text
                    Node::Text(txt) => {
                        // many (if not all) description cells have a trailing newline
                        let text = txt.replace('\n', "");
                        if text.is_empty() {
                            vec![]
                        } else {
                            vec![Span::Text { text }]
                        }
                    }
                    // ignore style tags and recurse
                    Node::Element(el) if matches!(el.name(), "b" | "i") => node
                        .children()
                        .map(parse_impl)
                        .collect::<Result<Vec<_>, _>>()?
                        .into_iter()
                        .flatten()
                        .collect(),
                    // links
                    Node::Element(el) if el.name() == "a" => {
                        let text = get_text_from_node(&node, false);
                        let link = get_link_from_element(el)?;

                        let span = if el.has_class("new", Cased) {
                            Span::SiteLinkNonExistent { text, link }
                        } else if el.has_class("external", Cased) {
                            Span::ExternalLink { text, link }
                        } else {
                            Span::SiteLink { text, link }
                        };
                        vec![span]
                    }
                    // superscripts
                    Node::Element(el) if el.name() == "sup" => 'el: {
                        if el.has_class("update", Cased) {
                            // ignore hidden `update` annotations
                            break 'el vec![];
                        }

                        let text = get_text_from_node(&node, false);

                        if el.has_class("Inline-Template", Cased) {
                            let Some(link_el) = node
                                .descendants()
                                .find_map(|n| n.value().as_element().filter(|el| el.name() == "a"))
                            else {
                                bail!("Encountered an annotation without a link");
                            };
                            let link = get_link_from_element(link_el)?;
                            break 'el vec![Span::Annotation { text, link }];
                        }

                        if el.has_class("reference", Cased) {
                            static REF_REGEX: Lazy<Regex> =
                                Lazy::new(|| Regex::new(r"\[(\d+)\]").unwrap());
                            if let Some(caps) = REF_REGEX.captures(&text) {
                                let number = caps.get(1).unwrap().as_str().parse()?;
                                let Some(link_el) = node.descendants().find_map(|n| {
                                    n.value().as_element().filter(|el| el.name() == "a")
                                }) else {
                                    bail!("Encountered a reference without a link");
                                };
                                let ref_id = get_link_from_element(link_el)?
                                    .trim_start_matches('#')
                                    .into();
                                break 'el vec![Span::Reference { number, ref_id }];
                            }

                            static NOTE_REGEX: Lazy<Regex> =
                                Lazy::new(|| Regex::new(r"\[note (\d+)\]").unwrap());
                            if let Some(caps) = NOTE_REGEX.captures(&text) {
                                let number = caps.get(1).unwrap().as_str().parse()?;
                                let Some(link_el) = node.descendants().find_map(|n| {
                                    n.value().as_element().filter(|el| el.name() == "a")
                                }) else {
                                    bail!("Encountered a note without a link");
                                };
                                let note_id = get_link_from_element(link_el)?
                                    .trim_start_matches('#')
                                    .into();
                                break 'el vec![Span::Note { number, note_id }];
                            }
                        }

                        bail!("Encountered an unknown superscript item")
                    }
                    Node::Element(el) => bail!("Encountered an unknown tag: {el:?}"),
                };

            Ok(span)
        }

        parse_impl(node).unwrap_or_else(|err| {
            warn!("{err}");
            vec![Self::Unknown {
                text: get_text_from_node(&node, false),
                err: Arc::new(err),
            }]
        })
    }

    /// Get the displayed text, excluding all superscripts and subscripts.
    pub fn normal_text(&self) -> Option<&str> {
        match self {
            Self::Text { text } => Some(text),
            Self::SiteLink { text, .. }
            | Self::SiteLinkNonExistent { text, .. }
            | Self::ExternalLink { text, .. } => Some(text),
            Self::Note { .. } | Self::Reference { .. } | Self::Annotation { .. } => None,
            Self::Unknown { text, .. } => Some(text),
        }
    }

    /// Check if this span contains the search term.
    ///
    /// This match is case-insensitive.
    pub fn matches_search(
        &self,
        search: impl AsRef<str>,
        include_links: bool,
        include_notes_and_references: bool,
    ) -> bool {
        let search = search.as_ref().to_lowercase();

        // eligible search scope
        let (text, link) = match self {
            Self::Text { text } => (Some(text), None),
            Self::SiteLink { text, link }
            | Self::SiteLinkNonExistent { text, link }
            | Self::ExternalLink { text, link } => {
                // link text is always shown
                (Some(text), if include_links { Some(link) } else { None })
            }
            Self::Note { note_id: id, .. } | Self::Reference { ref_id: id, .. } => {
                if include_notes_and_references {
                    (None, Some(id))
                } else {
                    (None, None)
                }
            }
            Self::Annotation { .. } => (None, None), // annotations are not helpful
            Self::Unknown { text, .. } => (Some(text), None),
        };

        // matches if found anywhere in search scope
        text.iter()
            .chain(link.iter())
            .any(|t| t.to_lowercase().contains(&search))
    }
}

pub fn parse_rich_text_cell(cell: ElementRef<'_>) -> color_eyre::Result<Vec<RichTextSpan>> {
    // sanity check
    if cell.value().name() != "td" {
        bail!("A rich text cell should be a `td` element");
    }

    let spans = cell.children().flat_map(RichTextSpan::parse).collect();
    Ok(spans)
}

/// Utility function to recursively get all text from a node.
fn get_text_from_node<'a, N>(node: N, trim: bool) -> String
where
    N: Deref<Target = NodeRef<'a, Node>>,
{
    node.descendants()
        .filter_map(|d| d.value().as_text())
        .map(|t| if trim { t.trim() } else { t.deref() })
        .collect()
}

/// Utility function to get a link from an `a` element.
fn get_link_from_element(el: &Element) -> color_eyre::Result<String> {
    // sanity check
    if el.name() != "a" {
        bail!("A link should be an `a` element");
    }

    let link = el
        .attr("href")
        .ok_or_eyre("Element has no `href` attribute")?;
    Ok(link.into())
}
