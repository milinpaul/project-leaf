//! What the card shows when the pointer rests on a link to a page (E10-S12): where the page sits, its
//! icon and title, then its first few blocks as small lines of plain text, so you can see it is the
//! right page before opening it. A link to a block shows that block, highlighted, with its neighbours.
//!
//! The lines come from the page's document, so the page must be loaded; the UI loads it from disk when
//! the pause before the card ends. Nothing here changes a document.

use leaf_core::{BlockId, PageId};
use leaf_model::{BlockKind, CalloutTone, PAGE_LINK, PageDoc, TextSpan};

use super::{Crumb, UNTITLED, WorkspaceEditor};
use crate::page_link::LinkState;

/// How many of the page's lines the card shows at most.
pub const PREVIEW_LINES: usize = 6;

/// How many characters of one block's words a line keeps before "…"; the card cuts long lines on
/// screen anyway, this only stops a huge block being copied for nothing.
const LINE_CHARS: usize = 120;

/// Nesting deeper than this is drawn at this depth, so a deep list still fits the card.
const MAX_DEPTH: usize = 3;

/// What a line of the preview is, so the card can draw it like the page does, smaller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewKind {
    /// A paragraph, a table cell or a callout's words.
    Text,
    /// A heading, `level` 1 to 3.
    Heading(u8),
    /// A bullet point.
    Bullet,
    /// A numbered item with its number, worked out as the page does.
    Number(usize),
    /// A to-do or a task, ticked or not.
    Todo(bool),
    /// A quotation.
    Quote,
    /// Code.
    Code,
    /// A callout, in its tone (the UI draws its icon).
    Callout(CalloutTone),
    /// A horizontal line.
    Divider,
    /// A picture; the words are its caption.
    Image,
    /// A table; the words are its first row.
    Table,
    /// A web bookmark; the words are its address.
    Bookmark,
    /// A database shown in the page; the words are its title.
    Database,
    /// A block this version doesn't know.
    Other,
}

/// One line of the preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewLine {
    /// What kind of block it is.
    pub kind: PreviewKind,
    /// Its words, plain (links as their pages' titles), cut at [`LINE_CHARS`].
    pub text: String,
    /// How deeply it is nested, 0 at the top level, at most [`MAX_DEPTH`].
    pub depth: usize,
    /// Whether it is the block a link to a block points at, drawn highlighted.
    pub linked: bool,
}

/// What the card shows for one link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagePreview {
    /// The page.
    pub page: PageId,
    /// The block a link to a block points at, if it is still there: a click on the card goes there.
    pub block: Option<BlockId>,
    /// The pages above it, from the top level down ("…" where a long path is shortened), each as
    /// its icon and title.
    pub path: Vec<String>,
    /// Its emoji icon, if it has one.
    pub icon: Option<String>,
    /// Its title, or [`UNTITLED`].
    pub title: String,
    /// Whether it is in the trash: the card then shows the title and "In trash", no lines.
    pub in_trash: bool,
    /// Whether the page's blocks were read. `false` for a page not loaded yet; the UI loads it and
    /// asks again.
    pub loaded: bool,
    /// Its first lines (or, for a link to a block, the lines around that block).
    pub lines: Vec<PreviewLine>,
    /// Whether the page has lines after the last one shown.
    pub more: bool,
}

impl WorkspaceEditor {
    /// The card for a link to `page`, or to `block` inside it. `None` for a page that is gone: a link
    /// to a deleted page shows no card.
    #[must_use]
    pub fn page_preview(&self, page: PageId, block: Option<BlockId>) -> Option<PagePreview> {
        let target = self.link_target(page);
        if target.state == LinkState::Missing {
            return None;
        }
        let in_trash = target.state == LinkState::Trashed;
        let mut path: Vec<String> = self
            .breadcrumb(page)
            .unwrap_or_default()
            .into_iter()
            .map(|crumb| match crumb {
                Crumb::Gap => "…".to_owned(),
                Crumb::Page { label, icon, .. } => match icon {
                    Some(icon) => format!("{icon} {label}"),
                    None => label,
                },
            })
            .collect();
        path.pop(); // the page itself
        let mut preview = PagePreview {
            page,
            block: None,
            path,
            icon: target.icon,
            title: if target.title.trim().is_empty() {
                UNTITLED.to_owned()
            } else {
                target.title
            },
            in_trash,
            loaded: false,
            lines: Vec::new(),
            more: false,
        };
        if in_trash {
            preview.loaded = true;
            return Some(preview);
        }
        let Some(editor) = self.page_editor(page) else {
            return Some(preview);
        };
        preview.loaded = true;
        let doc = editor.doc();
        let block = block.filter(|block| doc.contains(*block));
        preview.block = block;
        let all = self.lines_of(doc, block);
        // A link to a block starts one line above it, so the block reads in its place.
        let start = all
            .iter()
            .position(|line| line.linked)
            .map_or(0, |at| at.saturating_sub(1));
        let end = start.saturating_add(PREVIEW_LINES).min(all.len());
        preview.more = end < all.len();
        preview.lines = all.get(start..end).map(<[_]>::to_vec).unwrap_or_default();
        Some(preview)
    }

    /// Every line of `doc` worth showing, in reading order: empty paragraphs are left out, a table is
    /// one line (its first row). `linked` marks the line of that block.
    fn lines_of(&self, doc: &PageDoc, linked: Option<BlockId>) -> Vec<PreviewLine> {
        let mut lines = Vec::new();
        // (block, depth), popped from the end, so children are pushed in reverse.
        let mut stack: Vec<(BlockId, usize)> = doc.root_blocks().into_iter().rev().map(|b| (b, 0)).collect();
        while let Some((block, depth)) = stack.pop() {
            let Ok(kind) = doc.kind(block) else { continue };
            let is_table = matches!(kind, BlockKind::Table { .. });
            let words = || self.words(doc.text(block).ok().flatten().unwrap_or_default());
            let (kind, text) = match &kind {
                BlockKind::Paragraph | BlockKind::TableCell => (PreviewKind::Text, words()),
                BlockKind::Heading { level } => (PreviewKind::Heading(*level), words()),
                BlockKind::BulletedList => (PreviewKind::Bullet, words()),
                BlockKind::NumberedList => (PreviewKind::Number(number_of(doc, block)), words()),
                BlockKind::Todo { checked } => (PreviewKind::Todo(*checked), words()),
                BlockKind::Task { done, .. } => (PreviewKind::Todo(*done), words()),
                BlockKind::Quote => (PreviewKind::Quote, words()),
                BlockKind::Code { .. } => (PreviewKind::Code, words()),
                BlockKind::Callout { tone } => (PreviewKind::Callout(tone.clone()), words()),
                BlockKind::Divider => (PreviewKind::Divider, String::new()),
                BlockKind::Image { .. } => (PreviewKind::Image, words()),
                BlockKind::Table { .. } => (PreviewKind::Table, self.first_row(doc, block)),
                BlockKind::TableRow => continue,
                BlockKind::WebBookmark { url, .. } => (PreviewKind::Bookmark, url.clone()),
                BlockKind::Database { page } => (PreviewKind::Database, self.link_target(*page).name()),
                BlockKind::Unknown { .. } => (PreviewKind::Other, String::new()),
            };
            let is_linked = linked == Some(block);
            if !(kind == PreviewKind::Text && text.is_empty()) || is_linked {
                lines.push(PreviewLine {
                    kind,
                    text,
                    depth: depth.min(MAX_DEPTH),
                    linked: is_linked,
                });
            }
            // A table's rows are read as its one line, not as blocks of their own.
            if !is_table {
                let children = doc.children(block).unwrap_or_default();
                stack.extend(
                    children
                        .into_iter()
                        .rev()
                        .map(|child| (child, depth.saturating_add(1))),
                );
            }
        }
        lines
    }

    /// A table's first row, its cells' words joined with " · ".
    fn first_row(&self, doc: &PageDoc, table: BlockId) -> String {
        let cells = doc.table_cells(table).unwrap_or_default();
        let row = cells.into_iter().next().unwrap_or_default();
        let words: Vec<String> = row
            .into_iter()
            .map(|cell| self.words(doc.text(cell).ok().flatten().unwrap_or_default()))
            .filter(|words| !words.is_empty())
            .collect();
        cut(&words.join(" · "))
    }

    /// A block's words as plain text, each link as its page's name, with runs of space made one.
    fn words(&self, spans: Vec<TextSpan>) -> String {
        let mut text = String::new();
        for span in spans {
            match span.marks.page {
                Some(page) => {
                    for _ in span.text.chars().filter(|&c| c == PAGE_LINK) {
                        text.push_str(&self.link_to(page, span.marks.block).name());
                    }
                }
                None => text.push_str(&span.text),
            }
        }
        cut(&text.split_whitespace().collect::<Vec<_>>().join(" "))
    }
}

/// A numbered item's number: one more than the numbered items right before it among its siblings.
fn number_of(doc: &PageDoc, block: BlockId) -> usize {
    let siblings = match doc.parent(block) {
        Ok(Some(parent)) => doc.children(parent).unwrap_or_default(),
        _ => doc.root_blocks(),
    };
    let Some(at) = siblings.iter().position(|&b| b == block) else {
        return 1;
    };
    let before = siblings
        .get(..at)
        .unwrap_or_default()
        .iter()
        .rev()
        .take_while(|&&b| matches!(doc.kind(b), Ok(BlockKind::NumberedList)))
        .count();
    before.saturating_add(1)
}

/// `text` cut at [`LINE_CHARS`] characters with "…".
fn cut(text: &str) -> String {
    if text.chars().count() <= LINE_CHARS {
        return text.to_owned();
    }
    let kept: String = text.chars().take(LINE_CHARS).collect();
    format!("{}…", kept.trim_end())
}

#[cfg(test)]
mod tests;
