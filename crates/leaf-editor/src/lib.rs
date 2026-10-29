//! UI-free editor logic: cursor, selection, key handling, slash menu and clipboard.
//!
//! The editor turns key presses and mouse actions into model `Command`s, and turns documents into
//! view models that the UI only has to draw. All editing behaviour lives here as plain Rust so it
//! can be tested exhaustively without opening a window; the UI layer stays thin enough that it
//! needs almost no tests of its own.
//!
//! **Boundaries:** core crate, wasm-ok. No GPUI, no `leaf-ui`, no file system, threads or SQLite.
//! Depends on `leaf-core` and `leaf-model`. Never writes to Loro directly; it issues commands.

mod caret;
mod input;
mod page_editor;
mod view;
mod workspace_editor;

pub use caret::{LineLayout, Motion, Vertical};
pub use input::{Preedit, TextRange};
pub use page_editor::{Backspace, Caret, PageEditor};
pub use view::{BlockRow, BlockStyle, PageView};
pub use workspace_editor::{EditorError, SidebarItem, UNTITLED, WorkspaceEditor};
