//! Page links on the clipboard (E10-S03, ADR 0071). Leaf's own fragment keeps a link as itself; the text
//! other apps get names it, `[[Title]]` in Markdown and the title in plain text. Text pasted from elsewhere
//! turns `[[Title]]` back into a link when exactly one page has that title.

use leaf_core::PageId;
use leaf_model::{Marks, PAGE_LINK, TextSpan};

use super::{Fragment, FragmentBlock};
use crate::{EditorError, PageEditor};

/// Finds the page a pasted `[[Title]]` means, if exactly one page has that title.
pub(crate) type FindPage<'a> = &'a dyn Fn(&str) -> Option<PageId>;

impl Fragment {
    /// This fragment with each page link's placeholder replaced by `name(page)`, for writing out as
    /// Markdown or plain text. The named words keep the link's marks, so Markdown writes them `[[…]]`.
    #[must_use]
    pub fn with_link_names(&self, name: impl Fn(PageId) -> String) -> Self {
        self.map_spans(&|spans| {
            spans
                .iter()
                .flat_map(|span| match span.marks.page {
                    Some(page) => span
                        .text
                        .chars()
                        .map(|c| TextSpan {
                            text: if c == PAGE_LINK { name(page) } else { c.to_string() },
                            marks: span.marks.clone(),
                        })
                        .collect(),
                    None => vec![span.clone()],
                })
                .collect()
        })
    }

    /// This fragment with each `[[Title]]` that `find` knows turned into a link to that page; the rest
    /// stays text. Inline code is left alone.
    #[must_use]
    pub(crate) fn with_titles_linked(&self, find: FindPage<'_>) -> Self {
        self.map_spans(&|spans| {
            spans
                .iter()
                .flat_map(|span| {
                    if span.marks.code || span.marks.page.is_some() {
                        vec![span.clone()]
                    } else {
                        link_titles(span, find)
                    }
                })
                .collect()
        })
    }

    /// This fragment with every block's text (or the text) passed through `change`.
    fn map_spans(&self, change: &dyn Fn(&[TextSpan]) -> Vec<TextSpan>) -> Self {
        match self {
            Self::Text(spans) => Self::Text(change(spans)),
            Self::Blocks(list) => Self::Blocks(map_blocks(list, change)),
        }
    }
}

fn map_blocks(list: &[FragmentBlock], change: &dyn Fn(&[TextSpan]) -> Vec<TextSpan>) -> Vec<FragmentBlock> {
    list.iter()
        .map(|block| FragmentBlock {
            kind: block.kind.clone(),
            text: change(&block.text),
            children: map_blocks(&block.children, change),
        })
        .collect()
}

/// `span` split around each `[[Title]]` that `find` knows, the title replaced by a link.
fn link_titles(span: &TextSpan, find: FindPage<'_>) -> Vec<TextSpan> {
    let mut out = Vec::new();
    let mut rest = span.text.as_str();
    let mut words = String::new();
    while let Some(open) = rest.find("[[") {
        let after = &rest[open + 2..];
        let Some(close) = after.find("]]") else { break };
        let title = &after[..close];
        if let Some(page) = find(title.trim()) {
            words.push_str(&rest[..open]);
            if !words.is_empty() {
                out.push(TextSpan {
                    text: std::mem::take(&mut words),
                    marks: span.marks.clone(),
                });
            }
            out.push(TextSpan {
                text: PAGE_LINK.to_string(),
                marks: Marks {
                    page: Some(page),
                    link: None,
                    ..span.marks.clone()
                },
            });
            rest = &after[close + 2..];
        } else {
            words.push_str(&rest[..open + 2]);
            rest = after;
        }
    }
    words.push_str(rest);
    if !words.is_empty() {
        out.push(TextSpan {
            text: words,
            marks: span.marks.clone(),
        });
    }
    out
}

impl PageEditor {
    /// [`PageEditor::paste_text`], with each `[[Title]]` that `find` knows pasted as a link to that page
    /// (`find` gives the page for a title, when exactly one page has it). Into code, text stays text.
    pub fn paste_text_linking(
        &mut self,
        text: &str,
        find: FindPage<'_>,
    ) -> Result<Option<leaf_model::Caret>, EditorError> {
        self.paste_text_with(text, Some(find))
    }
}

#[cfg(test)]
mod tests;
