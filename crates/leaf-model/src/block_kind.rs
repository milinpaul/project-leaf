//! The block schema: which kinds of block exist, what each stores, and what it may contain.
//!
//! Matches the "Block types" table in `docs/architecture/BLOCK_MODEL.md`. A block's kind is stored
//! as a `type` string plus a `props` map in its meta, so a newer app can add kinds without
//! breaking older ones: anything unrecognised reads as [`BlockKind::Unknown`] and is left untouched.

use leaf_core::AssetId;
use loro::{Container, LoroMap, LoroValue, ValueOrContainer};

use crate::page_meta::value;

/// Meta and props key names.
pub(crate) mod keys {
    pub const ID: &str = "id";
    pub const TYPE: &str = "type";
    pub const PROPS: &str = "props";
    pub const TEXT: &str = "text";
    pub const LEVEL: &str = "level";
    pub const CHECKED: &str = "checked";
    pub const LANGUAGE: &str = "language";
    pub const ASSET: &str = "asset";
    pub const WIDTH: &str = "width";
    pub const ALT: &str = "alt";
}

/// What kind of block this is, with its type-specific properties.
#[derive(Debug, Clone, PartialEq)]
pub enum BlockKind {
    /// Plain text.
    Paragraph,
    /// A section heading, `level` 1 to 3.
    Heading {
        /// 1 is the largest.
        level: u8,
    },
    /// A bullet point.
    BulletedList,
    /// A numbered item. Numbers are worked out when drawing, never stored.
    NumberedList,
    /// A checkbox item.
    Todo {
        /// Whether it is ticked.
        checked: bool,
    },
    /// A quotation.
    Quote,
    /// Code, shown in a monospace font. Its text never carries marks.
    Code {
        /// Language label, for example `"rust"`; may be empty.
        language: String,
    },
    /// A horizontal line. Has no text.
    Divider,
    /// A picture from the workspace's asset store. Its text is the caption.
    Image {
        /// Which stored file to show.
        asset: AssetId,
        /// Display width in pixels, or `None` for the natural size.
        width: Option<f64>,
        /// Description for screen readers.
        alt: String,
    },
    /// A kind this version of Leaf does not know, probably from a newer version. It is shown as a
    /// read-only placeholder and its data is never rewritten, so nothing is lost.
    Unknown {
        /// The stored `type` string.
        type_name: String,
    },
}

impl BlockKind {
    /// The `type` string stored in the block's meta.
    #[must_use]
    pub fn type_name(&self) -> &str {
        match self {
            Self::Paragraph => "paragraph",
            Self::Heading { .. } => "heading",
            Self::BulletedList => "bulleted_list",
            Self::NumberedList => "numbered_list",
            Self::Todo { .. } => "todo",
            Self::Quote => "quote",
            Self::Code { .. } => "code",
            Self::Divider => "divider",
            Self::Image { .. } => "image",
            Self::Unknown { type_name } => type_name,
        }
    }

    /// Whether other blocks may be nested inside this one. Unknown kinds are read-only, so no.
    #[must_use]
    pub fn allows_children(&self) -> bool {
        matches!(
            self,
            Self::Paragraph | Self::BulletedList | Self::NumberedList | Self::Todo { .. } | Self::Quote
        )
    }

    /// Whether the block has a text container (the caption, for images).
    #[must_use]
    pub fn has_text(&self) -> bool {
        !matches!(self, Self::Divider | Self::Unknown { .. })
    }

    /// Whether its text may carry inline marks.
    #[must_use]
    pub fn allows_marks(&self) -> bool {
        self.has_text() && !matches!(self, Self::Code { .. })
    }

    /// Reads the kind from a block's meta. Errors name the problem; the caller adds which block.
    pub(crate) fn read(meta: &LoroMap) -> Result<Self, (&'static str, String)> {
        let type_name = match value(meta, keys::TYPE) {
            Some(LoroValue::String(s)) => s.to_string(),
            Some(_) => return Err((keys::TYPE, "is not text".into())),
            None => return Err((keys::TYPE, "is missing".into())),
        };
        let props = match meta.get(keys::PROPS) {
            Some(ValueOrContainer::Container(Container::Map(props))) => Some(props),
            _ => None,
        };
        let prop = |key| props.as_ref().and_then(|p| value(p, key));
        Ok(match type_name.as_str() {
            "paragraph" => Self::Paragraph,
            "heading" => match prop(keys::LEVEL) {
                Some(LoroValue::I64(level @ 1..=3)) => Self::Heading {
                    level: u8::try_from(level).unwrap_or(1),
                },
                Some(_) => return Err((keys::LEVEL, "must be 1, 2 or 3".into())),
                None => return Err((keys::LEVEL, "is missing".into())),
            },
            "bulleted_list" => Self::BulletedList,
            "numbered_list" => Self::NumberedList,
            "todo" => match prop(keys::CHECKED) {
                Some(LoroValue::Bool(checked)) => Self::Todo { checked },
                None => Self::Todo { checked: false },
                Some(_) => return Err((keys::CHECKED, "is not true or false".into())),
            },
            "quote" => Self::Quote,
            "code" => match prop(keys::LANGUAGE) {
                Some(LoroValue::String(language)) => Self::Code {
                    language: language.to_string(),
                },
                None => Self::Code {
                    language: String::new(),
                },
                Some(_) => return Err((keys::LANGUAGE, "is not text".into())),
            },
            "divider" => Self::Divider,
            "image" => {
                let asset = match prop(keys::ASSET) {
                    Some(LoroValue::String(hex)) => hex
                        .parse()
                        .map_err(|e: leaf_core::CoreError| (keys::ASSET, e.to_string()))?,
                    Some(_) => return Err((keys::ASSET, "is not text".into())),
                    None => return Err((keys::ASSET, "is missing".into())),
                };
                let width = match prop(keys::WIDTH) {
                    Some(LoroValue::Double(width)) => Some(width),
                    None => None,
                    Some(_) => return Err((keys::WIDTH, "is not a number".into())),
                };
                let alt = match prop(keys::ALT) {
                    Some(LoroValue::String(alt)) => alt.to_string(),
                    None => String::new(),
                    Some(_) => return Err((keys::ALT, "is not text".into())),
                };
                Self::Image { asset, width, alt }
            }
            _ => Self::Unknown { type_name },
        })
    }

    /// Writes `type` and `props` into a new block's meta. Never called for unknown kinds: the model
    /// only creates kinds it knows.
    pub(crate) fn write(&self, meta: &LoroMap) -> loro::LoroResult<()> {
        meta.insert(keys::TYPE, self.type_name())?;
        let props = || meta.ensure_mergeable_map(keys::PROPS);
        match self {
            Self::Heading { level } => props()?.insert(keys::LEVEL, i64::from(*level))?,
            Self::Todo { checked } => props()?.insert(keys::CHECKED, *checked)?,
            Self::Code { language } => props()?.insert(keys::LANGUAGE, language.as_str())?,
            Self::Image { asset, width, alt } => {
                let props = props()?;
                props.insert(keys::ASSET, asset.to_string())?;
                if let Some(width) = width {
                    props.insert(keys::WIDTH, *width)?;
                }
                props.insert(keys::ALT, alt.as_str())?;
            }
            // No props to write. Unknown kinds are never created, so never written either.
            Self::Paragraph
            | Self::BulletedList
            | Self::NumberedList
            | Self::Quote
            | Self::Divider
            | Self::Unknown { .. } => {}
        }
        Ok(())
    }
}
