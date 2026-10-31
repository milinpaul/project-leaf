//! Markdown shortcuts: typing `# `, `- `, `1. `, `[] `, `[!] `, `> `, ```` ``` ```` or `---` at the start
//! of a block turns it into a heading, list item, to-do, action item, quote, code block or divider.
//!
//! The shortcut fires when the typing completes one of these exactly, with nothing else before the
//! caret; text after the caret stays and becomes the converted block's text. The conversion is an undo
//! step of its own, separate from the typing: undo right after it gives back the typed characters as
//! plain text, in case the user meant them literally (ADR 0025).

use leaf_model::{BlockKind, Caret, PageCommand};

use crate::page_editor::insert_after;
use crate::{EditorError, PageEditor};

/// The block kind a shortcut typed at the start of a block asks for, if `typed` is one. `rest_empty`
/// says whether the block has no text after the caret (a divider can't keep any).
fn shortcut(typed: &str, rest_empty: bool) -> Option<BlockKind> {
    Some(match typed {
        "# " => BlockKind::Heading { level: 1 },
        "## " => BlockKind::Heading { level: 2 },
        "### " => BlockKind::Heading { level: 3 },
        "- " | "* " => BlockKind::BulletedList,
        "[] " => BlockKind::Todo { checked: false },
        "[!] " => BlockKind::Task {
            done: false,
            due: None,
            time: None,
            priority: None,
        },
        "> " => BlockKind::Quote,
        "```" => BlockKind::Code {
            language: String::new(),
        },
        "---" if rest_empty => BlockKind::Divider,
        // `1. `, or any number: lists are numbered when drawn, so the number itself isn't kept.
        _ => {
            let number = typed.strip_suffix(". ")?;
            if number.is_empty() || number.len() > 9 || !number.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            BlockKind::NumberedList
        }
    })
}

/// GitHub's alert syntax, `> [!WARNING] `, typed at the start of a block makes a callout (E21-S03).
/// Typing `> ` already made the block a quote, so in a quote `[!WARNING] ` alone does it too.
fn alert(typed: &str, kind: &BlockKind) -> Option<BlockKind> {
    let rest = typed
        .strip_prefix("> ")
        .or_else(|| (*kind == BlockKind::Quote).then_some(typed))?;
    let name = rest.strip_prefix("[!")?.strip_suffix("] ")?;
    let tone = crate::clipboard::markdown::alert::tone(name)?;
    Some(BlockKind::Callout { tone })
}

impl PageEditor {
    /// After typing that left the caret at `caret`: if everything before it is a Markdown shortcut,
    /// converts the block and returns where the caret goes. Code blocks and image captions are left
    /// alone (their text is literal), and so is a block that already has the asked-for kind.
    pub(crate) fn markdown_shortcut(&mut self, caret: Caret) -> Result<Option<Caret>, EditorError> {
        let block = caret.block;
        let kind = self.doc.kind(block)?;
        // Code text, image captions and table cells are literal (a cell stays a cell); dividers and
        // unknown kinds have no text to type in.
        if !kind.allows_marks() || matches!(kind, BlockKind::Image { .. } | BlockKind::TableCell) {
            return Ok(None);
        }
        let Some(text) = self.doc.plain_text(block)? else {
            return Ok(None);
        };
        let typed: String = text.chars().take(caret.at).collect();
        let rest_empty = text.chars().count() == caret.at;
        let Some(target) = alert(&typed, &kind).or_else(|| shortcut(&typed, rest_empty)) else {
            return Ok(None);
        };
        let same = match (&kind, &target) {
            (BlockKind::Todo { .. }, BlockKind::Todo { .. })
            | (BlockKind::Task { .. }, BlockKind::Task { .. }) => true,
            _ => kind == target,
        };
        if same {
            return Ok(None);
        }
        self.begin()?;
        let divider = target == BlockKind::Divider;
        let caret = self.doc.as_separate_step(|doc| -> Result<Caret, EditorError> {
            doc.apply(PageCommand::DeleteText {
                id: block,
                range: 0..caret.at,
            })?;
            doc.apply(PageCommand::SetBlockKind {
                id: block,
                kind: target,
            })?;
            // A divider holds no text: the caret goes to a new paragraph below it.
            Ok(if divider {
                Caret {
                    block: insert_after(doc, block, BlockKind::Paragraph)?,
                    at: 0,
                }
            } else {
                Caret { block, at: 0 }
            })
        })?;
        let caret = self.set_caret(caret)?;
        self.finish()?;
        Ok(Some(caret))
    }
}

#[cfg(test)]
mod tests;
