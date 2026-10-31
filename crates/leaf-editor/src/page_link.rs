//! Page links as they are drawn (E10-S03, ADR-071). In the page a link is one placeholder character; on
//! screen it is the target's icon and current title, or a note that the page is in the trash or gone.
//!
//! [`LinkedText`] turns a row's runs into the runs to draw and keeps the two sets of positions apart: the
//! caret, the selection and clicks speak in the page's positions, the text layout in drawn ones. A drawn
//! position inside a link's label snaps to the nearer end of the link, so the caret can never land inside
//! it.

use std::ops::Range;

use leaf_core::PageId;
use leaf_model::{PAGE_LINK, TextSpan};

use crate::UNTITLED;

/// Whether a link's target can be opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    /// The page exists and isn't in the trash.
    Live,
    /// The page is in the trash (or inside a page that is): restoring it brings the link back to life.
    Trashed,
    /// No such page: it was deleted for good, or never existed in this workspace.
    Missing,
}

/// What a link to one page shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkTarget {
    /// The page's title as it is now (empty for an untitled page).
    pub title: String,
    /// Its emoji icon, if it has one.
    pub icon: Option<String>,
    /// Whether it can be opened.
    pub state: LinkState,
}

impl LinkTarget {
    /// A link to a page that isn't there.
    #[must_use]
    pub fn missing() -> Self {
        Self {
            title: String::new(),
            icon: None,
            state: LinkState::Missing,
        }
    }

    /// The name a copied link is written as: the title (or "Untitled"), or "Deleted page".
    #[must_use]
    pub fn name(&self) -> String {
        match self.state {
            LinkState::Missing => "Deleted page".to_owned(),
            LinkState::Live | LinkState::Trashed if self.title.trim().is_empty() => UNTITLED.to_owned(),
            LinkState::Live | LinkState::Trashed => self.title.clone(),
        }
    }

    /// The words drawn in the link's place: the icon and title; "(in trash)" after a page in the trash;
    /// "Deleted page" for a page that is gone.
    #[must_use]
    pub fn label(&self) -> String {
        let title = if self.title.trim().is_empty() {
            UNTITLED
        } else {
            self.title.as_str()
        };
        match self.state {
            LinkState::Missing => "Deleted page".to_owned(),
            LinkState::Trashed => format!("{title} (in trash)"),
            LinkState::Live => match &self.icon {
                Some(icon) => format!("{icon} {title}"),
                None => title.to_owned(),
            },
        }
    }
}

/// One link as drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawnLink {
    /// Where its label is drawn, in characters of the drawn text.
    pub range: Range<usize>,
    /// The page it points at.
    pub page: PageId,
    /// Whether that page can be opened.
    pub state: LinkState,
}

/// A row's text with each page link drawn as its label, and the way between page and drawn positions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedText {
    /// The runs to draw. A link's label is a run of its own, with the link's marks.
    pub spans: Vec<TextSpan>,
    /// The drawn position of each page position, `0..=len`.
    starts: Vec<usize>,
    /// Every link, in order.
    links: Vec<DrawnLink>,
    /// Where web links are drawn, in characters of the drawn text.
    web: Vec<Range<usize>>,
}

impl LinkedText {
    /// Draws `spans`, asking `target` what each linked page shows. Text without links comes back as it is.
    pub fn new(spans: &[TextSpan], target: impl Fn(PageId) -> LinkTarget) -> Self {
        let mut out = Vec::new();
        let mut starts = vec![0];
        let mut links = Vec::new();
        let mut web = Vec::new();
        let mut drawn = 0usize;
        for span in spans {
            let Some(page) = span.marks.page else {
                let start = drawn;
                for _ in span.text.chars() {
                    drawn = drawn.saturating_add(1);
                    starts.push(drawn);
                }
                if span.marks.link.is_some() && drawn > start {
                    web.push(start..drawn);
                }
                out.push(span.clone());
                continue;
            };
            for c in span.text.chars() {
                // Only placeholders carry the page mark; anything else is drawn as it is.
                let (text, state) = if c == PAGE_LINK {
                    let target = target(page);
                    (target.label(), Some(target.state))
                } else {
                    (c.to_string(), None)
                };
                let len = text.chars().count();
                if let Some(state) = state {
                    links.push(DrawnLink {
                        range: drawn..drawn.saturating_add(len),
                        page,
                        state,
                    });
                }
                drawn = drawn.saturating_add(len);
                starts.push(drawn);
                out.push(TextSpan {
                    text,
                    marks: span.marks.clone(),
                });
            }
        }
        Self {
            spans: out,
            starts,
            links,
            web,
        }
    }

    /// The drawn text, all runs together.
    #[must_use]
    pub fn text(&self) -> String {
        self.spans.iter().map(|span| span.text.as_str()).collect()
    }

    /// Whether any link is drawn (so positions differ from the page's).
    #[must_use]
    pub fn has_links(&self) -> bool {
        !self.links.is_empty()
    }

    /// Where page position `at` is drawn (the end, past the text).
    #[must_use]
    pub fn to_drawn(&self, at: usize) -> usize {
        self.starts.get(at).or(self.starts.last()).copied().unwrap_or(0)
    }

    /// The page position drawn at `drawn`: inside a link's label, its nearer end.
    #[must_use]
    pub fn to_page(&self, drawn: usize) -> usize {
        let after = self.starts.partition_point(|&start| start <= drawn);
        let at = after.saturating_sub(1);
        let next = at.saturating_add(1);
        match (self.starts.get(at), self.starts.get(next)) {
            (Some(&start), Some(&end)) if drawn.saturating_sub(start) > end.saturating_sub(drawn) => next,
            _ => at,
        }
    }

    /// A page range in drawn positions.
    #[must_use]
    pub fn range_to_drawn(&self, range: &Range<usize>) -> Range<usize> {
        self.to_drawn(range.start)..self.to_drawn(range.end)
    }

    /// The link whose label covers drawn position `drawn` (its start included, its end not).
    #[must_use]
    pub fn link_at(&self, drawn: usize) -> Option<&DrawnLink> {
        self.links.iter().find(|link| link.range.contains(&drawn))
    }

    /// Every link, in order.
    #[must_use]
    pub fn links(&self) -> &[DrawnLink] {
        &self.links
    }

    /// Whether any web link is drawn.
    #[must_use]
    pub fn has_web_links(&self) -> bool {
        !self.web.is_empty()
    }

    /// Whether the character drawn at `drawn` can be clicked through (E10-S11): a page link to a page that
    /// is still there (live or in the trash), or a web link.
    #[must_use]
    pub fn clickable_at(&self, drawn: usize) -> bool {
        self.link_at(drawn)
            .is_some_and(|link| link.state != LinkState::Missing)
            || self.web.iter().any(|range| range.contains(&drawn))
    }
}

#[cfg(test)]
mod tests;
