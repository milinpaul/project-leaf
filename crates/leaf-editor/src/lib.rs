//! UI-free editor logic: cursor, selection, key handling, slash menu, block menu, dragging blocks,
//! clipboard, tables, quick switcher and search bar.
//!
//! The editor turns key presses and mouse actions into model `Command`s, and turns documents into
//! view models that the UI only has to draw. All editing behaviour lives here as plain Rust so it
//! can be tested exhaustively without opening a window; the UI layer stays thin enough that it
//! needs almost no tests of its own.
//!
//! **Boundaries:** core crate, wasm-ok. No GPUI, no `leaf-ui`, no file system, threads or SQLite.
//! Depends on `leaf-core` and `leaf-model`. Never writes to Loro directly; it issues commands.

mod block_actions;
mod block_menu;
mod bookmark_card;
mod caret;
mod clipboard;
mod code_block;
mod cover;
mod database_view;
mod drag;
mod edited;
mod emoji_picker;
mod format;
mod format_bar;
mod fuzzy;
mod history;
mod image;
mod input;
mod keys;
mod link;
mod link_menu;
mod page_editor;
mod page_link;
mod search_bar;
mod selection;
mod shortcuts;
mod slash;
mod switcher;
mod table;
mod table_edit;
mod task_due;
mod task_edit;
mod view;
mod web_bookmark;
mod workspace_editor;

pub use block_actions::BlockAction;
pub use block_menu::{BlockMenu, HoverChange, MenuEntry, MenuItem, MenuKey, turn_into_kinds};
pub use bookmark_card::{WebBookmarkCard, domain_of};
pub use caret::{LineLayout, Motion, Vertical};
pub use clipboard::{FRAGMENT_VERSION, Fragment, FragmentBlock};
pub use code_block::{CODE_INDENT, CODE_LANGUAGES, CodeLanguage, find_languages, language_label};
pub use cover::{BuiltinCover, CoverSection, PageCover};
pub use database_view::{
    COLUMN_TYPES, CellDisplay, CellEditor, Chip, ColumnMenu, DEFAULT_COLUMN_WIDTH, DatabaseEditor,
    FIRST_SCREEN_ROWS, FilterChip, FilterInput, HiddenColumn, OPTION_COLORS, OptionChoice, OptionPicker,
    QueryBar, RowProperty, SortChip, TableView, TextCommit, ViewColumn, ViewRow, date_label, direction_label,
    filter_ops, op_label, type_label,
};
pub use drag::{BlockDrop, RowBox};
pub use edited::{edited_ago, edited_on};
pub use emoji_picker::{
    EmojiChoice, EmojiPicker, EmojiSection, GridMove, RESULTS_TITLE as EMOJI_RESULTS_TITLE,
};
pub use format_bar::{FormatBar, KindChoice, MarkButton};
pub use fuzzy::{FuzzyMatch, fuzzy_match};
pub use image::{MIN_IMAGE_WIDTH, PasteKind, image_display_size, paste_kind, resized_image_width};
pub use input::Preedit;
pub use keys::{Key, KeyOutcome};
pub use link::openable_url;
pub use page_editor::{Backspace, Caret, PageEditor};
pub use page_link::{DrawnLink, LinkState, LinkTarget, LinkedText};
pub use search_bar::{
    BlockMatch, Destination, ExcerptRow, Marked, PageRow, SEARCH_DELAY_MS, SearchBar, SearchHit, SearchKey,
    SearchOutcome, SearchRequest, SearchRow, Shortcut, ShortcutAction, shortcut_action,
};
pub use selection::{Selection, TextRange};
pub use slash::{SlashAction, SlashItem, SlashMenu};
pub use switcher::{MAX_SWITCHER_RESULTS, QuickSwitcher, SwitcherResult};
pub use table_edit::TableEdit;
pub use task_due::{add_days, due_label, is_overdue};
pub use task_edit::{DatePicker, parse_time, priority_choices};
pub use view::{BlockRow, BlockStyle, PageView};
pub use web_bookmark::{BOOKMARK_OFFER_CHOICES, BookmarkOffer};
pub use workspace_editor::{
    BacklinkRow, BacklinkSource, Crumb, DropPlace, EditorError, LinkChoice, LinkMenu, MAX_CRUMBS,
    MAX_LINK_CHOICES, MenuPlace, PageAction, PageMenu, PageMenuEntry, PageMenuIcon, RECENT_LIMIT,
    SidebarItem, TrashItem, TrashList, UNTITLED, WorkspaceEditor, backlinks_label, trashed_ago,
};
