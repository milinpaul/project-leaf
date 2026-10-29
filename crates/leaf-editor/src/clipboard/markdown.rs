//! Markdown and plain text on the clipboard: pasting text from other apps, and writing what Leaf
//! copies as Markdown and plain text for them (ADR 0032).
//!
//! Reading is line by line, following `BLOCK_MODEL.md` "Markdown mapping": `#`–`###` headings
//! (deeper ones become heading 3), `-`/`*`/`+` bullets, `1.` numbers, `- [ ]`/`- [x]` to-dos, `>`
//! quotes, fenced code with its language, `---` dividers, and nesting by indentation (two spaces a
//! level). Every other line is a paragraph of its own, as pasted plain text expects; a line indented
//! under a list item or quote without a marker of its own continues it. Inline formatting is read by
//! [`inline`].

pub(crate) mod inline;

use leaf_model::{BlockKind, TextSpan};

use super::{Fragment, FragmentBlock};

/// What the start of a line says the block is, and where its text starts.
fn block_start(line: &str) -> Option<(BlockKind, &str)> {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') {
        let text = line[hashes + 1..].trim_end().trim_end_matches('#').trim_end();
        let level = u8::try_from(hashes.min(3)).unwrap_or(3);
        return Some((BlockKind::Heading { level }, text));
    }
    let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.len() >= 3
        && ["-", "*", "_"]
            .iter()
            .any(|d| compact.chars().all(|c| c.to_string() == *d))
    {
        return Some((BlockKind::Divider, ""));
    }
    for marker in ["- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(marker) {
            for (todo, checked) in [("[ ] ", false), ("[x] ", true), ("[X] ", true)] {
                if let Some(text) = rest.strip_prefix(todo) {
                    return Some((BlockKind::Todo { checked }, text));
                }
            }
            return Some((BlockKind::BulletedList, rest));
        }
    }
    for todo in ["[] ", "[ ] "] {
        if let Some(text) = line.strip_prefix(todo) {
            return Some((BlockKind::Todo { checked: false }, text));
        }
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if (1..=9).contains(&digits) && (line[digits..].starts_with(". ") || line[digits..].starts_with(") ")) {
        return Some((BlockKind::NumberedList, &line[digits + 2..]));
    }
    if let Some(rest) = line.strip_prefix('>') {
        return Some((BlockKind::Quote, rest.strip_prefix(' ').unwrap_or(rest)));
    }
    None
}

/// Columns of indentation, a tab counting as four.
fn indent(line: &str) -> usize {
    line.chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

/// Reads pasted text into blocks.
pub(crate) fn parse(text: &str) -> Vec<FragmentBlock> {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut rows: Vec<(usize, FragmentBlock)> = Vec::new();
    // An open code fence: its marker, indentation, language and lines so far.
    let mut fence: Option<(String, usize, String, Vec<String>)> = None;
    // Whether the last block is a list item or quote, which indented lines without a marker continue.
    let mut open_item: Option<usize> = None;
    for line in text.split('\n') {
        if let Some((marker, fence_indent, language, lines)) = &mut fence {
            if line.trim_start().starts_with(marker.as_str())
                && line.trim().chars().all(|c| c == '`' || c == '~')
            {
                let block = FragmentBlock {
                    kind: BlockKind::Code {
                        language: std::mem::take(language),
                    },
                    text: plain(&lines.join("\n")),
                    children: Vec::new(),
                };
                rows.push((*fence_indent / 2, block));
                fence = None;
            } else {
                let cut = indent(line).min(*fence_indent);
                lines.push(line.chars().skip(cut).collect());
            }
            continue;
        }
        if line.trim().is_empty() {
            open_item = None;
            continue;
        }
        let columns = indent(line);
        let content = line.trim_start();
        let ticks = content.chars().take_while(|&c| c == '`').count();
        if ticks >= 3 && !content[ticks..].contains('`') {
            fence = Some((
                "`".repeat(ticks),
                columns,
                content[ticks..].trim().to_owned(),
                Vec::new(),
            ));
            open_item = None;
            continue;
        }
        let start = block_start(content);
        if start.is_none()
            && open_item.is_some()
            && columns > 0
            && let Some((_, last)) = rows.last_mut()
        {
            // A wrapped line of the list item or quote above.
            last.text.push(TextSpan {
                text: " ".into(),
                marks: leaf_model::Marks::default(),
            });
            last.text.extend(inline::parse(content.trim_end()));
            last.text = inline::merge(std::mem::take(&mut last.text));
            continue;
        }
        let (kind, text) = start.unwrap_or((BlockKind::Paragraph, content));
        open_item = matches!(
            kind,
            BlockKind::BulletedList | BlockKind::NumberedList | BlockKind::Todo { .. } | BlockKind::Quote
        )
        .then_some(columns);
        let text = if kind == BlockKind::Divider {
            Vec::new()
        } else {
            inline::parse(text.trim_end())
        };
        rows.push((
            columns / 2,
            FragmentBlock {
                kind,
                text,
                children: Vec::new(),
            },
        ));
    }
    // A fence left open runs to the end of the text.
    if let Some((_, fence_indent, language, lines)) = fence {
        rows.push((
            fence_indent / 2,
            FragmentBlock {
                kind: BlockKind::Code { language },
                text: plain(&lines.join("\n")),
                children: Vec::new(),
            },
        ));
    }
    super::nest(rows)
}

/// What pasting `text` from another app gives: one line without block syntax pastes as text into the
/// line (with its inline formatting); anything else as blocks.
pub(crate) fn fragment_from_text(text: &str) -> Fragment {
    let trimmed = text.trim_end_matches(['\n', '\r']);
    if !trimmed.contains(['\n', '\r'])
        && block_start(trimmed.trim_start()).is_none()
        && !trimmed.trim_start().starts_with("```")
    {
        return Fragment::Text(inline::parse(trimmed));
    }
    Fragment::Blocks(parse(trimmed))
}

fn plain(text: &str) -> Vec<TextSpan> {
    if text.is_empty() {
        return Vec::new();
    }
    vec![TextSpan {
        text: text.to_owned(),
        marks: leaf_model::Marks::default(),
    }]
}

impl Fragment {
    /// The fragment as Markdown, for apps that read it (`BLOCK_MODEL.md` "Markdown mapping").
    #[must_use]
    pub fn to_markdown(&self) -> String {
        match self {
            Self::Text(spans) => inline::write(spans),
            Self::Blocks(blocks) => {
                let mut out = Vec::new();
                write_blocks(blocks, 0, &mut out);
                out.join("\n")
            }
        }
    }

    /// The fragment as plain text, one line per block: what apps without Markdown get.
    #[must_use]
    pub fn to_plain_text(&self) -> String {
        fn lines(blocks: &[FragmentBlock], out: &mut Vec<String>) {
            for block in blocks {
                if block.kind != BlockKind::Divider {
                    out.push(block.text.iter().map(|s| s.text.as_str()).collect());
                }
                lines(&block.children, out);
            }
        }
        match self {
            Self::Text(spans) => spans.iter().map(|s| s.text.as_str()).collect(),
            Self::Blocks(blocks) => {
                let mut out = Vec::new();
                lines(blocks, &mut out);
                out.join("\n")
            }
        }
    }
}

fn is_item(kind: &BlockKind) -> bool {
    matches!(
        kind,
        BlockKind::BulletedList | BlockKind::NumberedList | BlockKind::Todo { .. }
    )
}

fn write_blocks(blocks: &[FragmentBlock], depth: usize, out: &mut Vec<String>) {
    let pad = "  ".repeat(depth);
    let mut number = 0;
    for (i, block) in blocks.iter().enumerate() {
        // Top-level blocks are separated by a blank line, except within a run of list items.
        if depth == 0 && i > 0 && !(is_item(&block.kind) && is_item(&blocks[i - 1].kind)) {
            out.push(String::new());
        }
        number = if block.kind == BlockKind::NumberedList {
            number + 1
        } else {
            0
        };
        let text = inline::write(&block.text);
        match &block.kind {
            BlockKind::Heading { level } => {
                out.push(format!("{pad}{} {text}", "#".repeat(usize::from(*level))));
            }
            BlockKind::BulletedList => out.push(format!("{pad}- {text}")),
            BlockKind::NumberedList => out.push(format!("{pad}{number}. {text}")),
            BlockKind::Todo { checked } => {
                out.push(format!("{pad}- [{}] {text}", if *checked { "x" } else { " " }));
            }
            BlockKind::Quote => {
                for line in text.split('\n') {
                    out.push(format!("{pad}> {line}"));
                }
            }
            BlockKind::Code { language } => {
                let body: String = block.text.iter().map(|s| s.text.as_str()).collect();
                let longest = body
                    .lines()
                    .map(|l| l.chars().take_while(|&c| c == '`').count())
                    .max()
                    .unwrap_or(0);
                let fence = "`".repeat(longest.max(2) + 1);
                out.push(format!("{pad}{fence}{language}"));
                out.extend(body.split('\n').map(|l| format!("{pad}{l}")));
                out.push(format!("{pad}{fence}"));
            }
            BlockKind::Divider => out.push(format!("{pad}---")),
            BlockKind::Image { asset, alt, .. } => {
                out.push(format!("{pad}![{alt}](assets/{asset})"));
                if !text.is_empty() {
                    out.push(format!("{pad}*{text}*"));
                }
            }
            BlockKind::Paragraph | BlockKind::Unknown { .. } => {
                out.push(format!("{pad}{}", escape_start(&text)));
            }
        }
        write_blocks(&block.children, depth + 1, out);
    }
}

/// A paragraph that starts like a heading, list or quote gets that start escaped, so it reads back as
/// a paragraph.
fn escape_start(text: &str) -> String {
    let Some((kind, _)) = block_start(text) else {
        return text.to_owned();
    };
    if kind == BlockKind::NumberedList {
        let digits = text.chars().take_while(char::is_ascii_digit).count();
        return format!("{}\\{}", &text[..digits], &text[digits..]);
    }
    format!("\\{text}")
}

#[cfg(test)]
mod tests;
