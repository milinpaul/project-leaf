//! The clipboard fragment: copied content as versioned JSON, so it survives the trip through the
//! system clipboard and a paste into another Leaf window, or a newer Leaf (ADR 0030).
//!
//! ```json
//! {"leaf_fragment": 1, "blocks": [{"type": "heading", "level": 2,
//!   "text": [{"insert": "Plan", "bold": true}], "children": []}]}
//! ```
//!
//! A fragment is either a run of formatted text from inside one block (`"text"`), which pastes into
//! the line where the caret is, or whole blocks with what is nested in them (`"blocks"`).

use leaf_core::AssetId;
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

// Each mark is a separate optional flag on the wire, written only when set.
#[allow(clippy::struct_excessive_bools)]
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
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's `skip_serializing_if` passes a reference
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
