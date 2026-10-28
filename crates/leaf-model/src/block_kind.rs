//! The block schema: which kinds of block exist, what each stores, and what it may contain.
//!
//! Matches the "Block types" table in `docs/architecture/BLOCK_MODEL.md`. A block's kind is stored
//! as a `type` string plus a `props` map in its meta, so a newer app can add kinds without
//! breaking older ones: anything unrecognised reads as [`BlockKind::Unknown`] and is left untouched.

use leaf_core::AssetId;
use loro::{LoroMap, LoroValue};

use crate::ModelError;
use crate::page_meta::value;

/// One property of a block, for [`PageCommand::SetProp`](crate::PageCommand::SetProp). Each belongs to
/// one kind: setting it on another kind is an error rather than a silent no-op.
#[derive(Debug, Clone, PartialEq)]
pub enum BlockProp {
    /// Ticks or unticks a to-do.
    Checked(bool),
    /// A heading's level, 1 to 3.
    HeadingLevel(u8),
    /// A code block's language label.
    Language(String),
    /// An image's display width in pixels, or `None` for its natural size.
    ImageWidth(Option<f64>),
    /// An image's description for screen readers.
    ImageAlt(String),
}

impl BlockProp {
    /// A short name for error messages.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Checked(_) => "checked",
            Self::HeadingLevel(_) => "heading level",
            Self::Language(_) => "language",
            Self::ImageWidth(_) => "image width",
            Self::ImageAlt(_) => "image alt text",
        }
    }
}

/// Every known prop key, for removing the ones a new kind no longer uses.
pub(crate) const KNOWN_PROP_KEYS: [&str; 6] = [
    keys::LEVEL,
    keys::CHECKED,
    keys::LANGUAGE,
    keys::ASSET,
    keys::WIDTH,
    keys::ALT,
];

/// Meta and props key names.
pub(crate) mod keys {
    pub const ID: &str = "id";
    pub const TYPE: &str = "type";
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

    /// Checks this kind can be written by a command: no unknown kinds (the model only creates what it
    /// knows) and heading levels 1 to 3.
    pub(crate) fn check_creatable(&self) -> Result<(), ModelError> {
        match self {
            Self::Unknown { type_name } => Err(ModelError::InvalidKind(format!(
                "cannot create a block of unknown type {type_name:?}"
            ))),
            Self::Heading { level } if !(1..=3).contains(level) => Err(ModelError::InvalidKind(format!(
                "heading level must be 1, 2 or 3, not {level}"
            ))),
            _ => Ok(()),
        }
    }

    /// The prop keys this kind stores; any other known prop key is stale once a block becomes it.
    pub(crate) fn prop_keys(&self) -> &'static [&'static str] {
        match self {
            Self::Heading { .. } => &[keys::LEVEL],
            Self::Todo { .. } => &[keys::CHECKED],
            Self::Code { .. } => &[keys::LANGUAGE],
            Self::Image { .. } => &[keys::ASSET, keys::WIDTH, keys::ALT],
            _ => &[],
        }
    }

    /// The same kind with one property changed, or an error if the property doesn't belong to it.
    pub(crate) fn with_prop(&self, prop: BlockProp) -> Result<Self, ModelError> {
        let mismatch = |prop: &BlockProp| ModelError::PropNotForKind {
            prop: prop.name(),
            kind: self.type_name().to_owned(),
        };
        Ok(match (self.clone(), prop) {
            (Self::Todo { .. }, BlockProp::Checked(checked)) => Self::Todo { checked },
            (Self::Heading { .. }, BlockProp::HeadingLevel(level)) => Self::Heading { level },
            (Self::Code { .. }, BlockProp::Language(language)) => Self::Code { language },
            (Self::Image { asset, alt, .. }, BlockProp::ImageWidth(width)) => {
                Self::Image { asset, width, alt }
            }
            (Self::Image { asset, width, .. }, BlockProp::ImageAlt(alt)) => Self::Image { asset, width, alt },
            (_, prop) => return Err(mismatch(&prop)),
        })
    }

    /// Reads the kind from a block's meta. Errors name the problem; the caller adds which block.
    pub(crate) fn read(meta: &LoroMap) -> Result<Self, (&'static str, String)> {
        let type_name = match value(meta, keys::TYPE) {
            Some(LoroValue::String(s)) => s.to_string(),
            Some(_) => return Err((keys::TYPE, "is not text".into())),
            None => return Err((keys::TYPE, "is missing".into())),
        };
        // Properties sit directly in the block's meta, next to `type` (ADR 0007).
        let prop = |key| value(meta, key);
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

    /// Writes `type` and the kind's properties into a block's meta. Never called for unknown kinds:
    /// the model only creates kinds it knows.
    ///
    /// Properties are plain values in the meta itself, not a nested map: a nested map created in a
    /// later step than its block loses its values when undo re-creates the block and redo replays
    /// the step (ADR 0007).
    pub(crate) fn write(&self, meta: &LoroMap) -> loro::LoroResult<()> {
        meta.insert(keys::TYPE, self.type_name())?;
        match self {
            Self::Heading { level } => meta.insert(keys::LEVEL, i64::from(*level))?,
            Self::Todo { checked } => meta.insert(keys::CHECKED, *checked)?,
            Self::Code { language } => meta.insert(keys::LANGUAGE, language.as_str())?,
            Self::Image { asset, width, alt } => {
                meta.insert(keys::ASSET, asset.to_string())?;
                match width {
                    Some(width) => meta.insert(keys::WIDTH, *width)?,
                    None => meta.delete(keys::WIDTH)?,
                }
                meta.insert(keys::ALT, alt.as_str())?;
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
