//! The Leaf document model: Loro documents, the block schema, commands, undo and invariants.
//!
//! This is the only crate that writes to Loro. Every change to a page or the page tree is a
//! `Command` defined here, and each command is one Loro commit and one undo step. Funnelling all
//! writes through one place is what lets us test every edit, check invariants after it, and
//! later sync documents between devices without surprises.
//!
//! **Boundaries:** core crate, wasm-ok. No GPUI, no `leaf-ui`, no file system, threads or SQLite.
//! Depends only on `leaf-core`. Unknown data from newer app versions is preserved, never dropped.

mod block_kind;
mod command;
mod error;
mod page_command;
mod page_doc;
mod page_meta;
mod saved;
mod text;
mod workspace;

pub use block_kind::{BlockKind, BlockProp};
pub use command::{Outcome, WorkspaceCommand};
pub use error::ModelError;
pub use page_command::{PageCommand, PageOutcome};
pub use page_doc::{Caret, Changes, History, InvariantViolation, PageDoc};
pub use page_meta::PageMeta;
pub use text::{Mark, MarkKind, Marks, TextSpan};
pub use workspace::{DEFAULT_UNDO_MERGE_MS, Workspace};
