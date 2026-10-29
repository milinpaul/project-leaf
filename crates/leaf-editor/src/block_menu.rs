//! The block menu: what the ⋮⋮ handle beside a block (or `Cmd/Ctrl+/`) opens. It offers the block
//! actions (Turn into, Duplicate, Move up, Move down, Delete) for that block, or for the selection
//! when the block is part of it (ADR 0028).
//!
//! Like the slash menu, the editor keeps only what can't be read from the page: which block the menu
//! is for and which entry is highlighted. Which entries are available is worked out each time, so the
//! menu never offers a move the page no longer allows. Keys and clicks both go through here, so the
//! whole menu is testable without a window; the UI only draws it (ADR 0041).

use leaf_core::BlockId;
use leaf_model::BlockKind;

use crate::{BlockAction, EditorError, PageEditor};

/// The open menu as the editor keeps it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OpenMenu {
    block: BlockId,
    /// The highlighted entry of the main list.
    selected: usize,
    /// The highlighted kind while the "Turn into" list is open.
    kind: Option<usize>,
}

/// One entry of the block menu.
#[derive(Debug, Clone, PartialEq)]
pub enum MenuEntry {
    /// Opens the list of kinds.
    TurnInto,
    /// Duplicate the blocks.
    Duplicate,
    /// Move the blocks one place up.
    MoveUp,
    /// Move the blocks one place down.
    MoveDown,
    /// Delete the blocks.
    Delete,
    /// From the "Turn into" list: change the blocks to this kind.
    Kind(BlockKind),
}

impl MenuEntry {
    /// The words shown for the entry.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::TurnInto => "Turn into",
            Self::Duplicate => "Duplicate",
            Self::MoveUp => "Move up",
            Self::MoveDown => "Move down",
            Self::Delete => "Delete",
            Self::Kind(kind) => kind_label(kind),
        }
    }
}

/// An entry as drawn: greyed out when it would do nothing.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuItem {
    /// What it does.
    pub entry: MenuEntry,
    /// Whether it can be chosen.
    pub enabled: bool,
    /// For a kind in the "Turn into" list: whether the menu's block already is this kind.
    pub current: bool,
}

/// The open block menu, ready to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockMenu {
    /// The block whose handle opened it, to place the menu beside.
    pub block: BlockId,
    /// The main entries.
    pub items: Vec<MenuItem>,
    /// Which main entry is highlighted.
    pub selected: usize,
    /// The "Turn into" list and its highlighted kind, while it is open.
    pub kinds: Option<(Vec<MenuItem>, usize)>,
}

/// A key pressed while the menu has the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKey {
    /// Highlight the entry above (wrapping).
    Up,
    /// Highlight the entry below (wrapping).
    Down,
    /// Back from the "Turn into" list.
    Left,
    /// Into the "Turn into" list.
    Right,
    /// Choose the highlighted entry.
    Enter,
    /// Back from the "Turn into" list, or close the menu.
    Escape,
}

/// The kinds "Turn into" offers, in the order shown: the kinds a block's text can take.
#[must_use]
pub fn turn_into_kinds() -> Vec<BlockKind> {
    vec![
        BlockKind::Paragraph,
        BlockKind::Heading { level: 1 },
        BlockKind::Heading { level: 2 },
        BlockKind::Heading { level: 3 },
        BlockKind::BulletedList,
        BlockKind::NumberedList,
        BlockKind::Todo { checked: false },
        BlockKind::Quote,
        BlockKind::Code {
            language: String::new(),
        },
    ]
}

/// The name of a kind in the "Turn into" list.
fn kind_label(kind: &BlockKind) -> &'static str {
    match kind {
        BlockKind::Paragraph => "Text",
        BlockKind::Heading { level: 1 } => "Heading 1",
        BlockKind::Heading { level: 2 } => "Heading 2",
        BlockKind::Heading { .. } => "Heading 3",
        BlockKind::BulletedList => "Bulleted list",
        BlockKind::NumberedList => "Numbered list",
        BlockKind::Todo { .. } => "To-do list",
        BlockKind::Quote => "Quote",
        BlockKind::Code { .. } => "Code",
        BlockKind::Divider => "Divider",
        BlockKind::Image { .. } => "Image",
        BlockKind::Unknown { .. } => "Unknown",
    }
}

/// Whether `a` and `b` are the same kind for the "current" tick (a ticked to-do is still a to-do).
fn same_kind(a: &BlockKind, b: &BlockKind) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
        && match (a, b) {
            (BlockKind::Heading { level: x }, BlockKind::Heading { level: y }) => x == y,
            _ => true,
        }
}

impl PageEditor {
    /// Opens the menu for `block`'s handle, with the first entry highlighted. Closes the slash menu.
    pub fn open_block_menu(&mut self, block: BlockId) -> Result<Option<BlockMenu>, EditorError> {
        if !self.doc.contains(block) {
            return Ok(None);
        }
        self.slash = None;
        self.menu = Some(OpenMenu {
            block,
            selected: 0,
            kind: None,
        });
        self.block_menu()
    }

    /// `Cmd/Ctrl+/`: opens the menu for the caret's block, so it can be reached without a mouse.
    pub fn open_block_menu_at_caret(&mut self) -> Result<Option<BlockMenu>, EditorError> {
        match self.caret()? {
            Some(caret) => self.open_block_menu(caret.block),
            None => Ok(None),
        }
    }

    /// The open menu, if there is one. It closes by itself when its block is gone (undone, or
    /// deleted by another device).
    pub fn block_menu(&mut self) -> Result<Option<BlockMenu>, EditorError> {
        let Some(open) = self.menu else {
            return Ok(None);
        };
        if !self.doc.contains(open.block) {
            self.menu = None;
            return Ok(None);
        }
        let kind = self.doc.kind(open.block)?;
        let item = |entry: MenuEntry, enabled: bool| MenuItem {
            entry,
            enabled,
            current: false,
        };
        let items = vec![
            item(MenuEntry::TurnInto, !matches!(kind, BlockKind::Unknown { .. })),
            item(MenuEntry::Duplicate, true),
            item(MenuEntry::MoveUp, self.can_move(open.block, true)?),
            item(MenuEntry::MoveDown, self.can_move(open.block, false)?),
            item(MenuEntry::Delete, true),
        ];
        let kinds = open.kind.map(|selected| {
            let list = turn_into_kinds()
                .into_iter()
                .map(|k| MenuItem {
                    current: same_kind(&k, &kind),
                    entry: MenuEntry::Kind(k),
                    enabled: true,
                })
                .collect();
            (list, selected)
        });
        Ok(Some(BlockMenu {
            block: open.block,
            items,
            selected: open.selected,
            kinds,
        }))
    }

    /// Closes the menu without doing anything.
    pub fn close_block_menu(&mut self) {
        self.menu = None;
    }

    /// A key while the menu is open. Up and Down skip greyed-out entries. Returns the menu, or `None`
    /// once it has closed (Escape, or an entry was chosen).
    pub fn block_menu_key(&mut self, key: MenuKey) -> Result<Option<BlockMenu>, EditorError> {
        let Some(menu) = self.block_menu()? else {
            return Ok(None);
        };
        let Some(open) = &mut self.menu else {
            return Ok(None);
        };
        match (key, &menu.kinds) {
            (MenuKey::Up | MenuKey::Down, Some((kinds, at))) => {
                open.kind = Some(step(kinds, *at, key == MenuKey::Up));
            }
            (MenuKey::Up | MenuKey::Down, None) => {
                open.selected = step(&menu.items, menu.selected, key == MenuKey::Up);
            }
            (MenuKey::Left | MenuKey::Escape, Some(_)) => open.kind = None,
            (MenuKey::Escape, None) => self.menu = None,
            // Nothing to go back to, or no list to go into.
            (MenuKey::Left, None) | (MenuKey::Right, Some(_)) => {}
            (MenuKey::Right, None) => {
                if menu
                    .items
                    .get(menu.selected)
                    .is_some_and(|i| i.entry == MenuEntry::TurnInto)
                {
                    return self.choose(MenuEntry::TurnInto);
                }
            }
            (MenuKey::Enter, Some((kinds, at))) => {
                if let Some(item) = kinds.get(*at) {
                    return self.choose(item.entry.clone());
                }
            }
            (MenuKey::Enter, None) => {
                if let Some(item) = menu.items.get(menu.selected) {
                    return self.choose(item.entry.clone());
                }
            }
        }
        self.block_menu()
    }

    /// The mouse over `entry`: highlights it. Greyed-out entries aren't highlighted, and moving onto
    /// another main entry closes the "Turn into" list.
    pub fn highlight(&mut self, entry: &MenuEntry) -> Result<(), EditorError> {
        let Some(menu) = self.block_menu()? else {
            return Ok(());
        };
        let Some(open) = &mut self.menu else {
            return Ok(());
        };
        let find = |items: &[MenuItem]| items.iter().position(|i| &i.entry == entry && i.enabled);
        if let Some(at) = menu.kinds.as_ref().and_then(|(kinds, _)| find(kinds)) {
            open.kind = Some(at);
        } else if let Some(at) = find(&menu.items) {
            open.selected = at;
            if *entry != MenuEntry::TurnInto {
                open.kind = None;
            }
        }
        Ok(())
    }

    /// A click on `entry`: "Turn into" opens or closes the list of kinds; anything else runs the
    /// block action (one undo step) and closes the menu. Greyed-out entries do nothing.
    pub fn choose(&mut self, entry: MenuEntry) -> Result<Option<BlockMenu>, EditorError> {
        let Some(menu) = self.block_menu()? else {
            return Ok(None);
        };
        let enabled = match &entry {
            MenuEntry::Kind(_) => menu.kinds.is_some(),
            other => menu.items.iter().any(|i| &i.entry == other && i.enabled),
        };
        if !enabled {
            return Ok(Some(menu));
        }
        let action = match entry {
            MenuEntry::TurnInto => {
                if let Some(open) = &mut self.menu {
                    open.kind = match open.kind {
                        Some(_) => None,
                        None => Some(0),
                    };
                }
                return self.block_menu();
            }
            MenuEntry::Duplicate => BlockAction::Duplicate,
            MenuEntry::MoveUp => BlockAction::MoveUp,
            MenuEntry::MoveDown => BlockAction::MoveDown,
            MenuEntry::Delete => BlockAction::Delete,
            MenuEntry::Kind(kind) => BlockAction::TurnInto(kind),
        };
        self.menu = None;
        self.block_action(menu.block, action)?;
        Ok(None)
    }
}

/// The next enabled entry above or below `from`, wrapping; `from` itself if none is enabled.
fn step(items: &[MenuItem], from: usize, up: bool) -> usize {
    let count = items.len();
    (1..=count)
        .map(|n| {
            if up {
                (from + count * 2 - n) % count
            } else {
                (from + n) % count
            }
        })
        .find(|&i| items[i].enabled)
        .unwrap_or(from)
}

#[cfg(test)]
mod tests;
