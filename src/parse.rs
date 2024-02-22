use std::ops::{Deref, RangeInclusive};

use color_eyre::eyre::{bail, OptionExt};
use itertools::Itertools;
use scraper::{ElementRef, Html, Node, Selector};

use crate::store::{PortInfo, PortType};

/// Parse the Wikipedia port list page from its HTML source.
pub fn parse_page(html_str: &str) -> color_eyre::Result<Vec<PortInfo>> {
    let document = Html::parse_document(html_str);

    let table_selector = Selector::parse(".wikitable.sortable").unwrap();

    let list = document
        .select(&table_selector)
        .map(|table| parse_table(table))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect_vec();

    Ok(list)
}

/// Parse a table that contains a list of ports with their descriptions.
fn parse_table(table: ElementRef<'_>) -> color_eyre::Result<Vec<PortInfo>> {
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
        // remove comments
        .filter(|n| {
            if let Node::Element(el) = n.value() {
                el.name() != "sup"
            } else {
                true
            }
        })
        // find all text in remaining nodes
        .flat_map(|n| {
            n.descendants()
                .filter_map(|d| d.value().as_text())
                .map(|t| t.deref().trim())
        })
        .collect::<String>();
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
) -> color_eyre::Result<PortInfo>
where
    I: DoubleEndedIterator<Item = ElementRef<'a>>,
{
    // category
    let category = (&port_range).try_into()?;

    // description
    let description_cell = cells.next_back().ok_or_eyre("Row has no cells")?;
    // UNSURE: maybe this is incorrect
    let description = description_cell.text().collect::<String>().trim().into();

    // TCP, UDP, SCTP, DCCP
    let mut port_types = [PortType::Unused; 4];
    let mut types_it = port_types.iter_mut();
    for cell in cells {
        let span = match cell.attr("colspan") {
            Some(n) => n.parse::<usize>()?,
            None => 1,
        };
        let type_ = cell.try_into()?;
        for _ in 0..span {
            *types_it.next().ok_or_eyre("Port type cells span > 4")? = type_;
        }
    }

    Ok(PortInfo {
        number: port_range,
        category,
        tcp_type: port_types[0],
        udp_type: port_types[1],
        sctp_type: port_types[2],
        dccp_type: port_types[3],
        description,
    })
}
