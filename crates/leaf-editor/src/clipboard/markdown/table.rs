//! Markdown tables on the clipboard (E16-S04): reading a GitHub-style table into a table block, and
//! writing one out.
//!
//! A table is a header line, a delimiter line (`|---|:--:|`) with as many columns, then rows. Outer
//! pipes are optional, `\|` is a pipe inside a cell, and `<br>` is a line break in a cell. Text copied
//! from rendered pages often has blank lines between the rows, so a row after blank lines still
//! belongs to the table when it starts with `|`. The table ends at the first line that isn't a row
//! (has no `|`, or comes after a blank line without a leading `|`), or where a new table's header and
//! delimiter start. Rows with too many cells lose the extras; short rows get empty cells.
//!
//! Markdown tables always have a header, so a table written without a header row comes out with its
//! first row as the header.

use leaf_model::{BlockKind, Marks, TextSpan};

use super::inline;
use crate::FragmentBlock;

/// The table starting at `lines[at]`, if there is one, and the index of the first line after it.
pub(super) fn read(lines: &[&str], at: usize) -> Option<(FragmentBlock, usize)> {
    let header = lines.get(at)?.trim();
    if !header.contains('|') {
        return None;
    }
    let delimiter = next_filled(lines, at + 1)?;
    let header = cells(header);
    if !is_delimiter(lines[delimiter]) || cells(lines[delimiter].trim()).len() != header.len() {
        return None;
    }
    let columns = header.len();
    let mut rows = vec![row(&header, columns)];
    let mut end = delimiter + 1;
    while let Some(next) = next_filled(lines, end) {
        let line = lines[next].trim();
        let starts_table = next_filled(lines, next + 1).is_some_and(|d| is_delimiter(lines[d]));
        let after_blank = next > end;
        if !line.contains('|')
            || (after_blank && !line.starts_with('|'))
            || is_delimiter(line)
            || starts_table
        {
            break;
        }
        rows.push(row(&cells(line), columns));
        end = next + 1;
    }
    let table = FragmentBlock {
        kind: BlockKind::Table { header_row: true },
        text: Vec::new(),
        children: rows,
    };
    Some((table, end))
}

/// The index of the first non-blank line at or after `from`.
fn next_filled(lines: &[&str], from: usize) -> Option<usize> {
    (from..lines.len()).find(|&i| !lines[i].trim().is_empty())
}

/// Whether `line` is a delimiter line: every cell is dashes, with an optional colon at either end.
fn is_delimiter(line: &str) -> bool {
    let line = line.trim();
    line.contains('-')
        && (line.contains('|') || line.starts_with(':') || line.starts_with('-'))
        && cells(line).iter().all(|cell| {
            let dashes = cell.trim_start_matches(':').trim_end_matches(':');
            !dashes.is_empty() && dashes.chars().all(|c| c == '-')
        })
}

/// A row line's cells, trimmed: split at pipes that aren't escaped or inside a code span, without the
/// outer pipes.
fn cells(line: &str) -> Vec<String> {
    let line = line.strip_prefix('|').unwrap_or(line);
    let mut out = Vec::new();
    let mut cell = String::new();
    let mut in_code = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cell.push('|');
                chars.next();
            }
            '`' => {
                in_code = !in_code;
                cell.push(c);
            }
            '|' if !in_code => out.push(std::mem::take(&mut cell)),
            _ => cell.push(c),
        }
    }
    // Text after the last pipe is a cell unless the line ended with the outer pipe.
    if !cell.trim().is_empty() || out.is_empty() {
        out.push(cell);
    }
    out.into_iter().map(|c| c.trim().to_owned()).collect()
}

/// A table row of exactly `columns` cells from the cells' Markdown.
fn row(cells: &[String], columns: usize) -> FragmentBlock {
    let cell = |markdown: Option<&String>| FragmentBlock {
        kind: BlockKind::TableCell,
        text: markdown.map(|m| cell_text(m)).unwrap_or_default(),
        children: Vec::new(),
    };
    FragmentBlock {
        kind: BlockKind::TableRow,
        text: Vec::new(),
        children: (0..columns).map(|i| cell(cells.get(i))).collect(),
    }
}

/// A cell's formatted text: its inline Markdown, with `<br>` as a line break.
fn cell_text(markdown: &str) -> Vec<TextSpan> {
    let mut spans = Vec::new();
    for (i, line) in split_breaks(markdown).iter().enumerate() {
        if i > 0 {
            spans.push(TextSpan {
                text: "\n".into(),
                marks: Marks::default(),
            });
        }
        spans.extend(inline::parse(line.trim()));
    }
    inline::merge(spans)
}

/// `text` split at `<br>`, `<br/>` and `<br />`, any case.
fn split_breaks(text: &str) -> Vec<String> {
    let lower = text.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut from = 0;
    let mut search = 0;
    while let Some(found) = lower[search..].find("<br") {
        let start = search + found;
        let rest = &lower[start + 3..];
        let close = rest.trim_start().trim_start_matches('/').trim_start();
        if close.starts_with('>') {
            let end = start + 3 + (rest.len() - close.len()) + 1;
            out.push(text[from..start].to_owned());
            from = end;
            search = end;
        } else {
            search = start + 3;
        }
    }
    out.push(text[from..].to_owned());
    out
}

/// Writes `table` as a Markdown table, each line starting with `pad`.
pub(super) fn write(table: &FragmentBlock, pad: &str, out: &mut Vec<String>) {
    let rows: Vec<Vec<String>> = table
        .children
        .iter()
        .map(|row| {
            row.children
                .iter()
                .map(|cell| cell_markdown(&cell.text))
                .collect()
        })
        .collect();
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0).max(1);
    let line = |cells: &[String]| {
        let cells: Vec<&str> = (0..columns)
            .map(|i| cells.get(i).map_or("", String::as_str))
            .collect();
        format!("{pad}| {} |", cells.join(" | "))
    };
    let mut rows = rows.iter();
    out.push(line(rows.next().map_or(&[][..], Vec::as_slice)));
    out.push(format!("{pad}|{}", " --- |".repeat(columns)));
    out.extend(rows.map(|row| line(row)));
}

/// A cell's text as Markdown that stays in its cell: pipes escaped, line breaks as `<br>`.
fn cell_markdown(text: &[TextSpan]) -> String {
    inline::write(text).replace('|', "\\|").replace('\n', "<br>")
}

/// `table` as plain text: a line per row, cells separated by tabs, as spreadsheets paste it.
pub(super) fn plain(table: &FragmentBlock, out: &mut Vec<String>) {
    for row in &table.children {
        let cells: Vec<String> = row
            .children
            .iter()
            .map(|cell| {
                cell.text
                    .iter()
                    .map(|s| s.text.as_str())
                    .collect::<String>()
                    .replace(['\n', '\t'], " ")
            })
            .collect();
        out.push(cells.join("\t"));
    }
}
