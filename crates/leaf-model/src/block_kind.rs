//! The block schema: which kinds of block exist, what each stores, and what it may contain.
//!
//! Matches the "Block types" table in `docs/architecture/BLOCK_MODEL.md`. A block's kind is stored
//! as a `type` string plus a `props` map in its meta, so a newer app can add kinds without
//! breaking older ones: anything unrecognised reads as [`BlockKind::Unknown`] and is left untouched.

use leaf_core::AssetId;
use loro::{LoroMap, LoroValue};

use crate::page_meta::value;
use crate::{CalloutTone, DueDate, DueTime, ModelError, Priority};

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
    /// Whether a table's first row is a header row, drawn emphasised.
    TableHeaderRow(bool),
    /// Ticks an action item off, or back on.
    TaskDone(bool),
    /// An action item's due date; `None` clears it, and its time with it.
    TaskDue(Option<DueDate>),
    /// An action item's due time; it needs a due date. `None` clears it.
    TaskTime(Option<DueTime>),
    /// An action item's priority, or `None` for none.
    TaskPriority(Option<Priority>),
    /// A callout's tone.
    CalloutTone(CalloutTone),
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
            Self::TableHeaderRow(_) => "header row",
            Self::TaskDone(_) => "done",
            Self::TaskDue(_) => "due date",
            Self::TaskTime(_) => "due time",
            Self::TaskPriority(_) => "priority",
            Self::CalloutTone(_) => "tone",
        }
    }
}

/// Every known prop key, for removing the ones a new kind no longer uses.
pub(crate) const KNOWN_PROP_KEYS: [&str; 12] = [
    keys::LEVEL,
    keys::CHECKED,
    keys::LANGUAGE,
    keys::ASSET,
    keys::WIDTH,
    keys::ALT,
    keys::HEADER_ROW,
    keys::DONE,
    keys::DUE,
    keys::DUE_TIME,
    keys::PRIORITY,
    keys::TONE,
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
    pub const HEADER_ROW: &str = "header_row";
    pub const DONE: &str = "done";
    pub const DUE: &str = "due";
    pub const DUE_TIME: &str = "due_time";
    pub const PRIORITY: &str = "priority";
    pub const TONE: &str = "tone";
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
    /// A grid of rich-text cells (E16, ADR-061). Its children are [`BlockKind::TableRow`]s, theirs
    /// [`BlockKind::TableCell`]s; every row has the same number of cells. Built and reshaped only with
    /// the table commands ([`PageCommand::InsertTable`](crate::PageCommand::InsertTable) and friends);
    /// cell text is edited with the usual text commands.
    Table {
        /// Whether the first row is a header row.
        header_row: bool,
    },
    /// One row of a table. Has no text; holds the row's cells in column order.
    TableRow,
    /// One cell of a table row: text with the usual marks, nothing nested.
    TableCell,
    /// An action item (E19, ADR-068): something to do, with an optional due date and time and a
    /// priority, ticked off when done. Drawn differently from a to-do; notes can nest under it.
    Task {
        /// Whether it is done.
        done: bool,
        /// The day it is due.
        due: Option<DueDate>,
        /// The time of day it is due; only with a due date.
        time: Option<DueTime>,
        /// How important it is.
        priority: Option<Priority>,
    },
    /// A box with an icon that makes its text stand out (E21, ADR-070): rich text, nothing nested,
    /// drawn in its tone's colours.
    Callout {
        /// Its colour scheme and icon.
        tone: CalloutTone,
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
            Self::Table { .. } => "table",
            Self::TableRow => "table_row",
            Self::TableCell => "table_cell",
            Self::Task { .. } => "task",
            Self::Callout { .. } => "callout",
            Self::Unknown { type_name } => type_name,
        }
    }

    /// Whether other blocks may be nested inside this one by the general block commands (insert,
    /// move, indent). Unknown kinds are read-only, so no. Tables and rows do have children, but only
    /// rows and cells, managed by the table commands: see [`BlockKind::is_table_part`].
    #[must_use]
    pub fn allows_children(&self) -> bool {
        matches!(
            self,
            Self::Paragraph
                | Self::BulletedList
                | Self::NumberedList
                | Self::Todo { .. }
                | Self::Quote
                | Self::Task { .. }
        )
    }

    /// Whether the block has a text container (the caption, for images).
    #[must_use]
    pub fn has_text(&self) -> bool {
        !matches!(
            self,
            Self::Divider | Self::Unknown { .. } | Self::Table { .. } | Self::TableRow
        )
    }

    /// Whether this is a row or cell, which only exist inside a table and only change through the
    /// table commands (their text through the text commands). The general block commands refuse them,
    /// so a table always stays a whole grid.
    #[must_use]
    pub fn is_table_part(&self) -> bool {
        matches!(self, Self::TableRow | Self::TableCell)
    }

    /// The only kind a block of this kind may hold as children, for the kinds that hold a fixed
    /// kind: rows in a table, cells in a row.
    #[must_use]
    pub(crate) fn fixed_child(&self) -> Option<Self> {
        match self {
            Self::Table { .. } => Some(Self::TableRow),
            Self::TableRow => Some(Self::TableCell),
            _ => None,
        }
    }

    /// Whether its text may carry inline marks.
    #[must_use]
    pub fn allows_marks(&self) -> bool {
        self.has_text() && !matches!(self, Self::Code { .. })
    }

    /// Checks this kind can be written by a general command (`InsertBlock`, `SetBlockKind`): no unknown
    /// kinds (the model only creates what it knows), heading levels 1 to 3, and no table kinds (the
    /// table commands build those, so a table is always a whole grid).
    pub(crate) fn check_creatable(&self) -> Result<(), ModelError> {
        match self {
            Self::Unknown { type_name } => Err(ModelError::InvalidKind(format!(
                "cannot create a block of unknown type {type_name:?}"
            ))),
            Self::Heading { level } if !(1..=3).contains(level) => Err(ModelError::InvalidKind(format!(
                "heading level must be 1, 2 or 3, not {level}"
            ))),
            Self::Table { .. } | Self::TableRow | Self::TableCell => Err(ModelError::InvalidKind(
                "tables are made with InsertTable and reshaped with the table commands".into(),
            )),
            Self::Task {
                due: None,
                time: Some(_),
                ..
            } => Err(ModelError::TimeWithoutDate),
            Self::Callout {
                tone: CalloutTone::Other(name),
            } if name.is_empty() => Err(ModelError::InvalidKind("a callout's tone can't be empty".into())),
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
            Self::Table { .. } => &[keys::HEADER_ROW],
            Self::Task { .. } => &[keys::DONE, keys::DUE, keys::DUE_TIME, keys::PRIORITY],
            Self::Callout { .. } => &[keys::TONE],
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
            (Self::Table { .. }, BlockProp::TableHeaderRow(header_row)) => Self::Table { header_row },
            (Self::Callout { .. }, BlockProp::CalloutTone(tone)) => Self::Callout { tone },
            (
                Self::Task {
                    done,
                    due,
                    time,
                    priority,
                },
                prop @ (BlockProp::TaskDone(_)
                | BlockProp::TaskDue(_)
                | BlockProp::TaskTime(_)
                | BlockProp::TaskPriority(_)),
            ) => task_with(done, due, time, priority, prop)?,
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
        // Properties sit directly in the block's meta, next to `type` (ADR-007).
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
            "table" => match prop(keys::HEADER_ROW) {
                Some(LoroValue::Bool(header_row)) => Self::Table { header_row },
                None => Self::Table { header_row: false },
                Some(_) => return Err((keys::HEADER_ROW, "is not true or false".into())),
            },
            "table_row" => Self::TableRow,
            "table_cell" => Self::TableCell,
            "task" => read_task(meta)?,
            "callout" => match prop(keys::TONE) {
                Some(LoroValue::String(tone)) => Self::Callout {
                    tone: CalloutTone::parse(&tone),
                },
                None => Self::Callout {
                    tone: CalloutTone::Info,
                },
                Some(_) => return Err((keys::TONE, "is not text".into())),
            },
            _ => Self::Unknown { type_name },
        })
    }

    /// Writes `type` and the kind's properties into a block's meta. Never called for unknown kinds:
    /// the model only creates kinds it knows.
    ///
    /// Properties are plain values in the meta itself, not a nested map: a nested map created in a
    /// later step than its block loses its values when undo re-creates the block and redo replays
    /// the step (ADR-007).
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
            Self::Table { header_row } => meta.insert(keys::HEADER_ROW, *header_row)?,
            Self::Callout { tone } => meta.insert(keys::TONE, tone.as_str())?,
            Self::Task {
                done,
                due,
                time,
                priority,
            } => {
                meta.insert(keys::DONE, *done)?;
                write_optional(meta, keys::DUE, due.map(|d| d.to_string()))?;
                write_optional(meta, keys::DUE_TIME, time.map(|t| t.to_string()))?;
                write_optional(meta, keys::PRIORITY, priority.map(|p| p.as_str().to_owned()))?;
            }
            // No props to write. Unknown kinds are never created, so never written either.
            Self::Paragraph
            | Self::BulletedList
            | Self::NumberedList
            | Self::Quote
            | Self::Divider
            | Self::TableRow
            | Self::TableCell
            | Self::Unknown { .. } => {}
        }
        Ok(())
    }
}

/// Writes `value` under `key`, or removes the key for `None`.
pub(crate) fn write_optional(meta: &LoroMap, key: &str, value: Option<String>) -> loro::LoroResult<()> {
    match value {
        Some(value) => meta.insert(key, value.as_str()),
        None if meta.get(key).is_some() => meta.delete(key),
        None => Ok(()),
    }
}

/// Reads an action item's props. A missing prop is "not set"; one that is there but unreadable is an
/// error, as for the other kinds, so it is never quietly overwritten.
fn read_task(meta: &LoroMap) -> Result<BlockKind, (&'static str, String)> {
    let prop = |key: &str| value(meta, key);
    let done = match prop(keys::DONE) {
        Some(LoroValue::Bool(done)) => done,
        None => false,
        Some(_) => return Err((keys::DONE, "is not true or false".into())),
    };
    let due: Option<DueDate> = text_prop(keys::DUE, prop(keys::DUE))?;
    // A time on no day means nothing. Edits never store one, but two devices can: one clears the date
    // while the other sets the time (E17-S11). It reads as no time, so both agree; the stored value is
    // kept until the next change to the item writes its props as they read.
    let time: Option<DueTime> = text_prop(keys::DUE_TIME, prop(keys::DUE_TIME))?.filter(|_| due.is_some());
    let priority: Option<Priority> = text_prop(keys::PRIORITY, prop(keys::PRIORITY))?;
    Ok(BlockKind::Task {
        done,
        due,
        time,
        priority,
    })
}

/// Reads an optional text prop with `FromStr`; a value that isn't text, or doesn't parse, is an error.
fn text_prop<T: std::str::FromStr<Err = ModelError>>(
    key: &'static str,
    value: Option<LoroValue>,
) -> Result<Option<T>, (&'static str, String)> {
    match value {
        Some(LoroValue::String(text)) => text
            .parse()
            .map(Some)
            .map_err(|e: ModelError| (key, e.to_string())),
        None => Ok(None),
        Some(_) => Err((key, "is not text".into())),
    }
}

/// A task with one field changed. Clearing the date clears the time: a time on no day means nothing.
fn task_with(
    mut done: bool,
    mut due: Option<DueDate>,
    mut time: Option<DueTime>,
    mut priority: Option<Priority>,
    prop: BlockProp,
) -> Result<BlockKind, ModelError> {
    match prop {
        BlockProp::TaskDone(value) => done = value,
        BlockProp::TaskDue(value) => {
            due = value;
            time = time.filter(|_| due.is_some());
        }
        BlockProp::TaskTime(Some(_)) if due.is_none() => return Err(ModelError::TimeWithoutDate),
        BlockProp::TaskTime(value) => time = value,
        BlockProp::TaskPriority(value) => priority = value,
        other => {
            return Err(ModelError::PropNotForKind {
                prop: other.name(),
                kind: "task".to_owned(),
            });
        }
    }
    Ok(BlockKind::Task {
        done,
        due,
        time,
        priority,
    })
}
