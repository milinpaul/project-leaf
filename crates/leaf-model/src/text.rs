//! Inline marks on block text: which exist, how they grow while typing, and how text reads back.
//!
//! Follows the "Inline marks" table in `docs/architecture/BLOCK_MODEL.md`. Loro needs the expand
//! behaviour configured on every document before any edit, on creation and again after loading,
//! because the setting is not stored in the document.

use leaf_core::PageId;
use loro::{ExpandType, LoroDoc, LoroText, LoroValue, StyleConfig, StyleConfigMap, TextDelta};

/// Mark names as stored in Loro.
pub(crate) mod marks {
    pub const BOLD: &str = "bold";
    pub const ITALIC: &str = "italic";
    pub const STRIKE: &str = "strike";
    pub const CODE: &str = "code";
    pub const LINK: &str = "link";
    pub const PAGE: &str = "page";
}

/// The character a link to a page occupies in block text (U+FFFC, the object replacement character): one
/// placeholder carrying a `page` mark whose value is the target's ID (ADR-071). The title isn't stored, so
/// renaming the page updates every link to it; and being one character, a link can't be half edited.
pub const PAGE_LINK: char = '\u{FFFC}';

/// Applies the mark table: formatting grows when typing right after it; inline code, links and marks this
/// version doesn't know don't.
pub(crate) fn configure_marks(doc: &LoroDoc) {
    let mut styles = StyleConfigMap::new();
    for (key, expand) in [
        (marks::BOLD, ExpandType::After),
        (marks::ITALIC, ExpandType::After),
        (marks::STRIKE, ExpandType::After),
        (marks::CODE, ExpandType::None),
        (marks::LINK, ExpandType::None),
        (marks::PAGE, ExpandType::None),
    ] {
        styles.insert(key.into(), StyleConfig { expand });
    }
    doc.config_text_style(styles);
    // Marks from a newer version: Loro refuses unconfigured keys, which would make such text impossible
    // to edit or strip. Give them a safe default; they don't grow while typing.
    doc.config_default_text_style(Some(StyleConfig {
        expand: ExpandType::None,
    }));
}

/// The inline formatting on a run of text.
// Each mark is independent and maps one-to-one to a stored key, so plain flags read best here.
#[allow(
    clippy::struct_excessive_bools,
    reason = "each mark is an independent flag stored under its own key"
)]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Marks {
    /// **Bold.**
    pub bold: bool,
    /// *Italic.*
    pub italic: bool,
    /// ~~Struck through.~~
    pub strike: bool,
    /// `Inline code.`
    pub code: bool,
    /// The link target, if the text is a link.
    pub link: Option<String>,
    /// The page a page link points at; only ever on a [`PAGE_LINK`] placeholder.
    pub page: Option<PageId>,
}

/// A run of text whose characters all share the same marks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextSpan {
    /// The characters.
    pub text: String,
    /// Their formatting. Marks this version does not know are kept in the document but not shown.
    pub marks: Marks,
}

/// Reads a text container as spans, in order. Adjacent spans always differ in their marks.
pub(crate) fn spans(text: &LoroText) -> Vec<TextSpan> {
    text.to_delta()
        .into_iter()
        .filter_map(|delta| match delta {
            TextDelta::Insert { insert, attributes } => {
                let mut marks = Marks::default();
                for (key, value) in attributes.unwrap_or_default() {
                    let on = matches!(value, LoroValue::Bool(true));
                    match key.as_str() {
                        marks::BOLD => marks.bold = on,
                        marks::ITALIC => marks.italic = on,
                        marks::STRIKE => marks.strike = on,
                        marks::CODE => marks.code = on,
                        marks::LINK => {
                            if let LoroValue::String(href) = value {
                                marks.link = Some(href.to_string());
                            }
                        }
                        // An unreadable ID is kept in the document but not shown as a link.
                        marks::PAGE => {
                            if let LoroValue::String(page) = value {
                                marks.page = page.parse().ok();
                            }
                        }
                        _ => {}
                    }
                }
                Some(TextSpan { text: insert, marks })
            }
            TextDelta::Retain { .. } | TextDelta::Delete { .. } => None,
        })
        .collect()
}

/// An inline mark to apply to a range of text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mark {
    /// Bold.
    Bold,
    /// Italic.
    Italic,
    /// Strikethrough.
    Strike,
    /// Inline code.
    Code,
    /// A link to this address; must not be empty.
    Link(String),
    /// Points page links at this page. Only placeholders take it: links are made with
    /// [`PageCommand::InsertPageLink`](crate::PageCommand::InsertPageLink), and this re-points them.
    PageLink(PageId),
}

/// Which mark to remove, regardless of its value (removing a link does not need its address).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkKind {
    /// Bold.
    Bold,
    /// Italic.
    Italic,
    /// Strikethrough.
    Strike,
    /// Inline code.
    Code,
    /// Link.
    Link,
    /// Page link. Never removed on its own: a link goes by deleting its placeholder.
    PageLink,
}

impl Mark {
    /// Which kind of mark this is.
    #[must_use]
    pub fn kind(&self) -> MarkKind {
        match self {
            Self::Bold => MarkKind::Bold,
            Self::Italic => MarkKind::Italic,
            Self::Strike => MarkKind::Strike,
            Self::Code => MarkKind::Code,
            Self::Link(_) => MarkKind::Link,
            Self::PageLink(_) => MarkKind::PageLink,
        }
    }

    /// The value stored with the mark: `true`, or the link's address.
    pub(crate) fn value(&self) -> LoroValue {
        match self {
            Self::Link(href) => LoroValue::String(href.as_str().into()),
            Self::PageLink(page) => LoroValue::String(page.to_string().into()),
            Self::Bold | Self::Italic | Self::Strike | Self::Code => LoroValue::Bool(true),
        }
    }
}

impl MarkKind {
    /// The key the mark is stored under.
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Bold => marks::BOLD,
            Self::Italic => marks::ITALIC,
            Self::Strike => marks::STRIKE,
            Self::Code => marks::CODE,
            Self::Link => marks::LINK,
            Self::PageLink => marks::PAGE,
        }
    }
}
