//! The Leaf document model: Loro documents, the block schema, commands, undo and invariants.
//!
//! This is the only crate that writes to Loro. Every change to a page or the page tree is a
//! `Command` defined here, and each command is one Loro commit and one undo step. Funnelling all
//! writes through one place is what lets us test every edit, check invariants after it, and
//! later sync documents between devices without surprises.
//!
//! **Boundaries:** core crate, wasm-ok. No GPUI, no `leaf-ui`, no file system, threads or SQLite.
//! Depends only on `leaf-core`. Unknown data from newer app versions is preserved, never dropped.

mod command;
mod error;
mod page_meta;
mod workspace;

pub use command::{Outcome, WorkspaceCommand};
pub use error::ModelError;
pub use page_meta::PageMeta;
pub use workspace::Workspace;
