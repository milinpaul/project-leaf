//! The clipboard fragment: copied content as versioned JSON, so it survives the trip through the
//! system clipboard and a paste into another Leaf window, or a newer Leaf (ADR-030).
//!
//! ```json
//! {"leaf_fragment": 1, "blocks": [{"type": "heading", "level": 2,
//!   "text": [{"insert": "Plan", "bold": true}], "children": []}]}
//! ```
//!
//! A fragment is either a run of formatted text from inside one block (`"text"`), which pastes into
//! the line where the caret is, or whole blocks with what is nested in them (`"blocks"`).

use leaf_core::{AssetId, PageId};
use leaf_model::{BlockKind, Marks, TextSpan};
use serde::{Deserialize, Serialize};

/// The format version written. A reader refuses versions it doesn't know rather than guess.
pub const FRAGMENT_VERSION: u32 = 1;

/// Copied content.
#[derive(Debug, Clone, PartialEq)]
pub enum Fragment {
    /// Formatted text from inside one block.
    Text(Vec<TextSpan>),
    /// Whole blocks, each with what is nested in it.
    Blocks(Vec<FragmentBlock>),
}

/// One copied block.
#[derive(Debug, Clone, PartialEq)]
pub struct FragmentBlock {
    /// Its kind and properties.
    pub kind: BlockKind,
    /// Its formatted text; empty for blocks without text.
    pub text: Vec<TextSpan>,
    /// Blocks nested in it.
    pub children: Vec<FragmentBlock>,
}

impl FragmentBlock {
    /// A paragraph holding one link to `page`: what an inline database is copied as (E09-S09). A page shows a
    /// database in one place, so a copy links to it rather than making a second table or copying its data.
    #[must_use]
    pub fn link_to(page: PageId) -> Self {
        Self {
            kind: BlockKind::Paragraph,
            text: vec![TextSpan {
                text: leaf_model::PAGE_LINK.to_string(),
                marks: Marks {
                    page: Some(page),
                    ..Marks::default()
                },
            }],
            children: Vec::new(),
        }
    }
}

impl Fragment {
    /// The fragment as JSON, for the system clipboard.
    #[must_use]
    pub fn to_json(&self) -> String {
        let wire = match self {
            Self::Text(spans) => Wire {
                leaf_fragment: FRAGMENT_VERSION,
                text: Some(spans.iter().map(WireSpan::from).collect()),
                blocks: None,
            },
            Self::Blocks(blocks) => Wire {
                leaf_fragment: FRAGMENT_VERSION,
                text: None,
                blocks: Some(blocks.iter().filter_map(WireBlock::from_block).collect()),
            },
        };
        serde_json::to_string(&wire).unwrap_or_default()
    }

    /// Reads a fragment back. `None` for anything else on the clipboard, or a version this Leaf
    /// doesn't know.
    #[must_use]
    pub fn from_json(json: &str) -> Option<Self> {
        let wire: Wire = serde_json::from_str(json).ok()?;
        if wire.leaf_fragment != FRAGMENT_VERSION {
            return None;
        }
        match (wire.text, wire.blocks) {
            (Some(text), None) => Some(Self::Text(text.into_iter().map(TextSpan::from).collect())),
            (None, Some(blocks)) => Some(Self::Blocks(
                blocks
                    .into_iter()
                    .map(WireBlock::into_block)
                    .collect::<Option<_>>()?,
            )),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Wire {
    leaf_fragment: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text: Option<Vec<WireSpan>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    blocks: Option<Vec<WireBlock>>,
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "each mark is a separate optional flag on the wire, written only when set"
)]
#[derive(Serialize, Deserialize)]
struct WireSpan {
    insert: String,
    #[serde(default, skip_serializing_if = "is_false")]
    bold: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    italic: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    strike: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    code: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    link: Option<String>,
    /// The page a page link points at (ADR-071).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    page: Option<PageId>,
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's `skip_serializing_if` passes a reference"
)]
fn is_false(value: &bool) -> bool {
    !value
}

impl From<&TextSpan> for WireSpan {
    fn from(span: &TextSpan) -> Self {
        Self {
            insert: span.text.clone(),
            bold: span.marks.bold,
            italic: span.marks.italic,
            strike: span.marks.strike,
            code: span.marks.code,
            link: span.marks.link.clone(),
            page: span.marks.page,
        }
    }
}

impl From<WireSpan> for TextSpan {
    fn from(span: WireSpan) -> Self {
        Self {
            text: span.insert,
            marks: Marks {
                bold: span.bold,
                italic: span.italic,
                strike: span.strike,
                code: span.code,
                link: span.link.filter(|l| !l.is_empty()),
                page: span.page,
            },
        }
    }
}

/// A block's kind on the wire, tagged by the same type names the document uses.
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WireKind {
    Paragraph,
    Heading {
        level: u8,
    },
    BulletedList,
    NumberedList,
    Todo {
        checked: bool,
    },
    Quote,
    Code {
        language: String,
    },
    Divider,
    Image {
        asset: AssetId,
        width: Option<f64>,
        alt: String,
    },
    Table {
        header_row: bool,
    },
    TableRow,
    TableCell,
    /// An action item (E19): its fields as the document stores them.
    Task {
        #[serde(default, skip_serializing_if = "is_false")]
        done: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        due: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        due_time: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        priority: Option<String>,
    },
    /// A callout (E21): its tone by its stored name, so a tone from a newer version survives a copy.
    Callout {
        tone: String,
    },
    /// A web bookmark (E15): its address and the preview fetched for it, so a copy needs no new fetch.
    WebBookmark {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        image: Option<AssetId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        favicon: Option<AssetId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fetched_at: Option<i64>,
    },
}

#[derive(Serialize, Deserialize)]
struct WireBlock {
    #[serde(flatten)]
    kind: WireKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    text: Vec<WireSpan>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    children: Vec<WireBlock>,
}

impl WireBlock {
    /// Blocks from a newer version can't be pasted (this version can't create them), so they are left
    /// out of the copy, with what is nested in them.
    fn from_block(block: &FragmentBlock) -> Option<Self> {
        let kind = match &block.kind {
            BlockKind::Paragraph => WireKind::Paragraph,
            BlockKind::Heading { level } => WireKind::Heading { level: *level },
            BlockKind::BulletedList => WireKind::BulletedList,
            BlockKind::NumberedList => WireKind::NumberedList,
            BlockKind::Todo { checked } => WireKind::Todo { checked: *checked },
            BlockKind::Quote => WireKind::Quote,
            BlockKind::Code { language } => WireKind::Code {
                language: language.clone(),
            },
            BlockKind::Divider => WireKind::Divider,
            BlockKind::Image { asset, width, alt } => WireKind::Image {
                asset: *asset,
                width: *width,
                alt: alt.clone(),
            },
            BlockKind::Table { header_row } => WireKind::Table {
                header_row: *header_row,
            },
            BlockKind::TableRow => WireKind::TableRow,
            BlockKind::TableCell => WireKind::TableCell,
            BlockKind::Task {
                done,
                due,
                time,
                priority,
            } => WireKind::Task {
                done: *done,
                due: due.map(|d| d.to_string()),
                due_time: time.map(|t| t.to_string()),
                priority: priority.map(|p| p.as_str().to_owned()),
            },
            BlockKind::Callout { tone } => WireKind::Callout {
                tone: tone.as_str().to_owned(),
            },
            BlockKind::WebBookmark { url, preview } => WireKind::WebBookmark {
                url: url.clone(),
                title: preview.title.clone(),
                description: preview.description.clone(),
                image: preview.image,
                favicon: preview.favicon,
                fetched_at: preview.fetched_at,
            },
            BlockKind::Database { page } => return Self::from_block(&FragmentBlock::link_to(*page)),
            BlockKind::Unknown { .. } => return None,
        };
        Some(Self {
            kind,
            text: block.text.iter().map(WireSpan::from).collect(),
            children: block.children.iter().filter_map(Self::from_block).collect(),
        })
    }

    fn into_block(self) -> Option<FragmentBlock> {
        let kind = match self.kind {
            WireKind::Paragraph => BlockKind::Paragraph,
            WireKind::Heading { level } => BlockKind::Heading { level },
            WireKind::BulletedList => BlockKind::BulletedList,
            WireKind::NumberedList => BlockKind::NumberedList,
            WireKind::Todo { checked } => BlockKind::Todo { checked },
            WireKind::Quote => BlockKind::Quote,
            WireKind::Code { language } => BlockKind::Code { language },
            WireKind::Divider => BlockKind::Divider,
            WireKind::Image { asset, width, alt } => BlockKind::Image { asset, width, alt },
            WireKind::Table { header_row } => BlockKind::Table { header_row },
            WireKind::TableRow => BlockKind::TableRow,
            WireKind::TableCell => BlockKind::TableCell,
            // A field this version can't read makes the copy unreadable, like a kind it doesn't know.
            WireKind::Task {
                done,
                due,
                due_time,
                priority,
            } => BlockKind::Task {
                done,
                due: due.map(|d| d.parse()).transpose().ok()?,
                time: due_time.map(|t| t.parse()).transpose().ok()?,
                priority: priority.map(|p| p.parse()).transpose().ok()?,
            },
            WireKind::Callout { tone } => BlockKind::Callout {
                tone: leaf_model::CalloutTone::parse(&tone),
            },
            // An address no bookmark may hold makes the copy unreadable rather than half pasted.
            WireKind::WebBookmark {
                url,
                title,
                description,
                image,
                favicon,
                fetched_at,
            } => {
                leaf_model::check_web_address(&url).ok()?;
                BlockKind::WebBookmark {
                    url,
                    preview: leaf_model::WebPreview {
                        title,
                        description,
                        image,
                        favicon,
                        fetched_at,
                    },
                }
            }
        };
        Some(FragmentBlock {
            kind,
            text: self.text.into_iter().map(TextSpan::from).collect(),
            children: self
                .children
                .into_iter()
                .map(Self::into_block)
                .collect::<Option<_>>()?,
        })
    }
}
