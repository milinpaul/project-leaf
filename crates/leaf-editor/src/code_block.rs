//! Code blocks (E07-S09, ADR-054): the language picker's list, and Tab and Shift+Tab inside code,
//! which indent lines with spaces instead of nesting the block.
//!
//! A language is stored as the short name a Markdown code fence uses (`rust`, `python`), so pasting
//! fenced code and copying it back out keep it. Names Leaf doesn't list are kept and shown as they are.

use std::ops::Range;

use leaf_core::BlockId;
use leaf_model::{BlockKind, BlockProp, Caret, PageCommand};

use crate::keys::{Key, KeyOutcome};
use crate::{EditorError, PageEditor, fuzzy_match};

/// How many spaces Tab puts in front of a line of code.
pub const CODE_INDENT: usize = 4;

/// A language the picker offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeLanguage {
    /// What is stored: a Markdown fence name, or empty for plain text.
    pub id: &'static str,
    /// What is shown.
    pub label: &'static str,
}

const fn lang(id: &'static str, label: &'static str) -> CodeLanguage {
    CodeLanguage { id, label }
}

/// Every language the picker offers, plain text first, then by name.
pub const CODE_LANGUAGES: &[CodeLanguage] = &[
    lang("", "Plain text"),
    lang("bash", "Bash"),
    lang("c", "C"),
    lang("cpp", "C++"),
    lang("csharp", "C#"),
    lang("css", "CSS"),
    lang("diff", "Diff"),
    lang("go", "Go"),
    lang("html", "HTML"),
    lang("java", "Java"),
    lang("javascript", "JavaScript"),
    lang("json", "JSON"),
    lang("kotlin", "Kotlin"),
    lang("lua", "Lua"),
    lang("markdown", "Markdown"),
    lang("php", "PHP"),
    lang("python", "Python"),
    lang("ruby", "Ruby"),
    lang("rust", "Rust"),
    lang("scala", "Scala"),
    lang("shell", "Shell"),
    lang("sql", "SQL"),
    lang("swift", "Swift"),
    lang("toml", "TOML"),
    lang("typescript", "TypeScript"),
    lang("xml", "XML"),
    lang("yaml", "YAML"),
];

/// The name shown for a stored language: its label when Leaf lists it, "Plain text" for none, and
/// otherwise the stored name itself (from a pasted fence or a newer Leaf).
#[must_use]
pub fn language_label(id: &str) -> String {
    CODE_LANGUAGES
        .iter()
        .find(|l| l.id.eq_ignore_ascii_case(id.trim()))
        .map_or_else(|| id.trim().to_owned(), |l| l.label.to_owned())
}

/// The languages matching what was typed in the picker, best first; all of them for an empty query.
#[must_use]
pub fn find_languages(query: &str) -> Vec<CodeLanguage> {
    if query.trim().is_empty() {
        return CODE_LANGUAGES.to_vec();
    }
    let mut found: Vec<(u32, usize, CodeLanguage)> = CODE_LANGUAGES
        .iter()
        .enumerate()
        .filter_map(|(i, l)| {
            let by_label = fuzzy_match(l.label, query).map(|m| m.score);
            let by_id = fuzzy_match(l.id, query).map(|m| m.score);
            by_label.max(by_id).map(|score| (score, i, *l))
        })
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    found.into_iter().map(|(_, _, l)| l).collect()
}

impl PageEditor {
    /// Sets code block `block`'s language (an id from [`CODE_LANGUAGES`], or any name), as one undo
    /// step. Does nothing for other kinds of block, or when it is already that language.
    pub fn set_code_language(&mut self, block: BlockId, language: &str) -> Result<(), EditorError> {
        let BlockKind::Code { language: now } = self.doc.kind(block)? else {
            return Ok(());
        };
        if now == language {
            return Ok(());
        }
        self.begin()?;
        self.doc.apply(PageCommand::SetProp {
            id: block,
            prop: BlockProp::Language(language.to_owned()),
        })?;
        self.finish()
    }

    /// Tab or Shift+Tab with the caret, or a selection, inside one code block: indents or outdents
    /// the lines it touches by [`CODE_INDENT`] spaces, as one undo step. With just a caret, Tab
    /// inserts the spaces at the caret. `None` when this isn't the code block's case (the key nests
    /// blocks as usual).
    pub(crate) fn code_tab(
        &mut self,
        key: Key,
        caret: Caret,
        span: Option<(Caret, Caret)>,
    ) -> Result<Option<KeyOutcome>, EditorError> {
        if !matches!(key, Key::Tab | Key::ShiftTab)
            || !matches!(self.doc.kind(caret.block)?, BlockKind::Code { .. })
            || span.is_some_and(|(start, end)| start.block != end.block)
        {
            return Ok(None);
        }
        let block = caret.block;
        let text: Vec<char> = self.doc.plain_text(block)?.unwrap_or_default().chars().collect();
        let (start, end) = span.map_or((caret.at, caret.at), |(s, e)| (s.at, e.at));
        let spaces = " ".repeat(CODE_INDENT);
        // Each edit is (position, characters added or removed), applied from the end backwards so
        // earlier positions stay valid.
        let mut edits: Vec<(usize, isize)> = Vec::new();
        if key == Key::Tab && span.is_none() {
            edits.push((start, signed(CODE_INDENT)));
        } else {
            for line in line_starts(&text, start..end) {
                let change = if key == Key::Tab {
                    signed(CODE_INDENT)
                } else {
                    signed(
                        text.get(line..)
                            .unwrap_or_default()
                            .iter()
                            .take(CODE_INDENT)
                            .take_while(|c| **c == ' ')
                            .count(),
                    )
                    .saturating_neg()
                };
                if change != 0 {
                    edits.push((line, change));
                }
            }
        }
        self.doc.as_one_step(|doc| -> Result<(), EditorError> {
            for &(at, change) in edits.iter().rev() {
                let command = if change > 0 {
                    PageCommand::InsertText {
                        id: block,
                        pos: at,
                        text: spaces.clone(),
                    }
                } else {
                    PageCommand::DeleteText {
                        id: block,
                        range: at..at.saturating_add(change.unsigned_abs()),
                    }
                };
                doc.apply(command)?;
            }
            Ok(())
        })?;
        let moved = |pos: usize| -> usize {
            let mut new = signed(pos);
            for &(at, change) in &edits {
                if change > 0 && at <= pos {
                    new = new.saturating_add(change);
                } else if change < 0 && at < pos {
                    // Removed spaces before `pos` pull it back, but never before the line start.
                    new = new.saturating_sub(signed(pos.saturating_sub(at)).min(change.saturating_neg()));
                }
            }
            usize::try_from(new).unwrap_or(0)
        };
        let at = Caret {
            block,
            at: moved(caret.at),
        };
        match span {
            Some((s, e)) => {
                let tail = if caret == s { e } else { s };
                self.select_between(
                    Caret {
                        block,
                        at: moved(tail.at),
                    },
                    at,
                )?;
            }
            None => {
                self.set_caret(at)?;
            }
        }
        Ok(Some(KeyOutcome::Caret(at)))
    }
}

/// Where each line touched by `range` starts. A selection ending right at a line's start doesn't
/// touch that line, as in code editors.
fn line_starts(text: &[char], range: Range<usize>) -> Vec<usize> {
    let start_of = |pos: usize| {
        text.get(..pos)
            .unwrap_or(text)
            .iter()
            .rposition(|c| *c == '\n')
            .map_or(0, |i| i.saturating_add(1))
    };
    let last = if range.end > range.start && start_of(range.end) == range.end {
        range.end.saturating_sub(1)
    } else {
        range.end
    };
    let mut starts = vec![start_of(range.start)];
    for (i, c) in text
        .iter()
        .enumerate()
        .take(last.min(text.len()))
        .skip(range.start)
    {
        if *c == '\n' {
            starts.push(i.saturating_add(1));
        }
    }
    starts
}

/// A count as a signed change. Text lengths are far below `isize::MAX`.
fn signed(n: usize) -> isize {
    isize::try_from(n).unwrap_or(isize::MAX)
}

#[cfg(test)]
mod tests;
