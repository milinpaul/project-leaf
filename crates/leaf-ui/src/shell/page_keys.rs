//! The page's keyboard: which key does what (`PRODUCT_SPEC.md` "Editor behaviour" and the inline
//! shortcuts), bound in the page's key context so they only act while the page has the keyboard.
//! Every action is one call into `PageEditor`; the UI then scrolls the caret into view and redraws.
//!
//! `secondary!` is Cmd on macOS and Ctrl elsewhere.

use gpui::{App, ClipboardEntry, ClipboardItem, Context, InteractiveElement, KeyBinding, Window, actions};
use leaf_editor::{Fragment, Key, KeyOutcome, Motion, PasteKind, Vertical, paste_kind};
use leaf_model::MarkKind;

use super::image::ImageSource;
use super::{Shell, report};

/// A shortcut with Cmd on macOS and Ctrl elsewhere.
macro_rules! secondary {
    ($key:literal) => {
        if cfg!(target_os = "macos") {
            concat!("cmd-", $key)
        } else {
            concat!("ctrl-", $key)
        }
    };
}

/// The page's key context: these bindings act only inside it.
pub(super) const CONTEXT: &str = "LeafPage";

actions!(
    leaf_page,
    [
        /// Split the block, or apply the slash menu's entry.
        Enter,
        /// Leave a code block.
        ModEnter,
        /// Delete backwards.
        Backspace,
        /// Delete forwards.
        Delete,
        /// Nest the block.
        Tab,
        /// Un-nest the block.
        ShiftTab,
        /// One character left.
        Left,
        /// One character right.
        Right,
        /// To the previous word.
        WordLeft,
        /// To the next word.
        WordRight,
        /// To the start of the line.
        LineStart,
        /// To the end of the line.
        LineEnd,
        /// Extend the selection one character left.
        SelectLeft,
        /// Extend the selection one character right.
        SelectRight,
        /// Extend the selection to the previous word.
        SelectWordLeft,
        /// Extend the selection to the next word.
        SelectWordRight,
        /// Extend the selection to the start of the line.
        SelectLineStart,
        /// Extend the selection to the end of the line.
        SelectLineEnd,
        /// One line up.
        Up,
        /// One line down.
        Down,
        /// Extend the selection one line up.
        SelectUp,
        /// Extend the selection one line down.
        SelectDown,
        /// The block's text, then every block.
        SelectAll,
        /// Close the slash menu, or drop the selection.
        Escape,
        /// Bold.
        Bold,
        /// Italic.
        Italic,
        /// Inline code.
        InlineCode,
        /// Strikethrough.
        Strike,
        /// Undo.
        Undo,
        /// Redo.
        Redo,
        /// Copy.
        Copy,
        /// Cut.
        Cut,
        /// Paste.
        Paste,
    ]
);

/// Binds the page's keys. Called once at startup.
pub(crate) fn bind(cx: &mut App) {
    let c = Some(CONTEXT);
    let mut bindings = vec![
        KeyBinding::new("enter", Enter, c),
        KeyBinding::new(secondary!("enter"), ModEnter, c),
        KeyBinding::new("backspace", Backspace, c),
        KeyBinding::new("shift-backspace", Backspace, c),
        KeyBinding::new("delete", Delete, c),
        KeyBinding::new("tab", Tab, c),
        KeyBinding::new("shift-tab", ShiftTab, c),
        KeyBinding::new("left", Left, c),
        KeyBinding::new("right", Right, c),
        KeyBinding::new("home", LineStart, c),
        KeyBinding::new("end", LineEnd, c),
        KeyBinding::new("shift-left", SelectLeft, c),
        KeyBinding::new("shift-right", SelectRight, c),
        KeyBinding::new("shift-home", SelectLineStart, c),
        KeyBinding::new("shift-end", SelectLineEnd, c),
        KeyBinding::new("up", Up, c),
        KeyBinding::new("down", Down, c),
        KeyBinding::new("shift-up", SelectUp, c),
        KeyBinding::new("shift-down", SelectDown, c),
        KeyBinding::new(secondary!("a"), SelectAll, c),
        KeyBinding::new("escape", Escape, c),
        KeyBinding::new(secondary!("b"), Bold, c),
        KeyBinding::new(secondary!("i"), Italic, c),
        KeyBinding::new(secondary!("e"), InlineCode, c),
        KeyBinding::new(secondary!("shift-s"), Strike, c),
        KeyBinding::new(secondary!("z"), Undo, c),
        KeyBinding::new(secondary!("shift-z"), Redo, c),
        KeyBinding::new(secondary!("c"), Copy, c),
        KeyBinding::new(secondary!("x"), Cut, c),
        KeyBinding::new(secondary!("v"), Paste, c),
    ];
    if cfg!(target_os = "macos") {
        bindings.extend([
            KeyBinding::new("alt-left", WordLeft, c),
            KeyBinding::new("alt-right", WordRight, c),
            KeyBinding::new("cmd-left", LineStart, c),
            KeyBinding::new("cmd-right", LineEnd, c),
            KeyBinding::new("alt-shift-left", SelectWordLeft, c),
            KeyBinding::new("alt-shift-right", SelectWordRight, c),
            KeyBinding::new("cmd-shift-left", SelectLineStart, c),
            KeyBinding::new("cmd-shift-right", SelectLineEnd, c),
            KeyBinding::new("ctrl-a", LineStart, c),
            KeyBinding::new("ctrl-e", LineEnd, c),
        ]);
    } else {
        bindings.extend([
            KeyBinding::new("ctrl-left", WordLeft, c),
            KeyBinding::new("ctrl-right", WordRight, c),
            KeyBinding::new("ctrl-shift-left", SelectWordLeft, c),
            KeyBinding::new("ctrl-shift-right", SelectWordRight, c),
            KeyBinding::new("ctrl-y", Redo, c),
        ]);
    }
    cx.bind_keys(bindings);
    super::block_menu::bind(cx, CONTEXT, secondary!("/"));
    super::link_popover::bind(cx);
    super::language_picker::bind(cx);
    super::task_pickers::bind(cx, CONTEXT, secondary!("shift-d"), secondary!("shift-p"));
}

impl Shell {
    /// Attaches every page action to `element`.
    pub(super) fn with_page_actions<E: InteractiveElement>(element: E, cx: &mut Context<Self>) -> E {
        element
            .on_action(cx.listener(|s, _: &Enter, w, cx| s.key(Key::Enter, w, cx)))
            .on_action(cx.listener(|s, _: &ModEnter, w, cx| s.key(Key::ModEnter, w, cx)))
            .on_action(cx.listener(|s, _: &Backspace, w, cx| s.key(Key::Backspace, w, cx)))
            .on_action(cx.listener(|s, _: &Delete, w, cx| s.key(Key::Delete, w, cx)))
            .on_action(cx.listener(|s, _: &Tab, w, cx| s.key(Key::Tab, w, cx)))
            .on_action(cx.listener(|s, _: &ShiftTab, w, cx| s.key(Key::ShiftTab, w, cx)))
            .on_action(cx.listener(|s, _: &Left, w, cx| s.motion(Motion::Left, false, w, cx)))
            .on_action(cx.listener(|s, _: &Right, w, cx| s.motion(Motion::Right, false, w, cx)))
            .on_action(cx.listener(|s, _: &WordLeft, w, cx| s.motion(Motion::WordLeft, false, w, cx)))
            .on_action(cx.listener(|s, _: &WordRight, w, cx| s.motion(Motion::WordRight, false, w, cx)))
            .on_action(cx.listener(|s, _: &LineStart, w, cx| s.motion(Motion::LineStart, false, w, cx)))
            .on_action(cx.listener(|s, _: &LineEnd, w, cx| s.motion(Motion::LineEnd, false, w, cx)))
            .on_action(cx.listener(|s, _: &SelectLeft, w, cx| s.motion(Motion::Left, true, w, cx)))
            .on_action(cx.listener(|s, _: &SelectRight, w, cx| s.motion(Motion::Right, true, w, cx)))
            .on_action(cx.listener(|s, _: &SelectWordLeft, w, cx| s.motion(Motion::WordLeft, true, w, cx)))
            .on_action(cx.listener(|s, _: &SelectWordRight, w, cx| s.motion(Motion::WordRight, true, w, cx)))
            .on_action(cx.listener(|s, _: &SelectLineStart, w, cx| s.motion(Motion::LineStart, true, w, cx)))
            .on_action(cx.listener(|s, _: &SelectLineEnd, w, cx| s.motion(Motion::LineEnd, true, w, cx)))
            .on_action(cx.listener(|s, _: &Up, w, cx| s.vertical(Vertical::Up, false, w, cx)))
            .on_action(cx.listener(|s, _: &Down, w, cx| s.vertical(Vertical::Down, false, w, cx)))
            .on_action(cx.listener(|s, _: &SelectUp, w, cx| s.vertical(Vertical::Up, true, w, cx)))
            .on_action(cx.listener(|s, _: &SelectDown, w, cx| s.vertical(Vertical::Down, true, w, cx)))
            .on_action(cx.listener(Self::on_select_all))
            .on_action(cx.listener(Self::on_escape))
            .on_action(cx.listener(|s, _: &Bold, w, cx| s.format(MarkKind::Bold, w, cx)))
            .on_action(cx.listener(|s, _: &Italic, w, cx| s.format(MarkKind::Italic, w, cx)))
            .on_action(cx.listener(|s, _: &InlineCode, w, cx| s.format(MarkKind::Code, w, cx)))
            .on_action(cx.listener(|s, _: &Strike, w, cx| s.format(MarkKind::Strike, w, cx)))
            .on_action(cx.listener(|s, _: &Undo, w, cx| s.history(true, w, cx)))
            .on_action(cx.listener(|s, _: &Redo, w, cx| s.history(false, w, cx)))
            .on_action(cx.listener(Self::on_copy))
            .on_action(cx.listener(Self::on_cut))
            .on_action(cx.listener(Self::on_paste))
    }

    /// After any page action: the caret is shown solid, kept in view, and the page redrawn.
    pub(super) fn changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pick_image_if_asked(window, cx);
        self.restart_blink();
        self.reveal_caret();
        window.invalidate_character_coordinates();
        cx.notify();
    }

    fn key(&mut self, key: Key, window: &mut Window, cx: &mut Context<Self>) {
        // Enter with the page-link menu open links the highlighted page (E10-S05).
        if key == Key::Enter
            && self
                .editor
                .link_menu()
                .ok()
                .flatten()
                .is_some_and(|menu| !menu.choices.is_empty())
        {
            return self.apply_link_menu(None, window, cx);
        }
        match self.page().map(|page| page.press(key)) {
            Some(Ok(KeyOutcome::SelectBlock(block))) => self.blocks.list.reveal(block),
            Some(Err(err)) => report("could not apply the key", err),
            _ => {}
        }
        self.changed(window, cx);
    }

    fn motion(&mut self, motion: Motion, extend: bool, window: &mut Window, cx: &mut Context<Self>) {
        let moved = self.page().map(|page| {
            if extend {
                page.extend(motion).map(|_| ())
            } else {
                page.move_caret(motion).map(|_| ())
            }
        });
        if let Some(Err(err)) = moved {
            report("could not move the caret", err);
        }
        self.changed(window, cx);
    }

    /// Up and Down: the slash menu's highlight while it is open; otherwise the caret by painted lines.
    /// Up on the first line of the first block goes to the title.
    fn vertical(&mut self, direction: Vertical, extend: bool, window: &mut Window, cx: &mut Context<Self>) {
        let painted = self.blocks.painted.clone();
        let first_row = self.blocks.list.rows.first().map(|row| row.id);
        if !extend && self.editor.link_menu().ok().flatten().is_some() {
            let _ = self.editor.link_menu_move(direction);
            return self.changed(window, cx);
        }
        let Some(page) = self.page() else { return };
        if !extend && page.slash_menu().ok().flatten().is_some() {
            let _ = page.slash_move(direction);
            return self.changed(window, cx);
        }
        let caret = page.caret().ok().flatten();
        let at_top = caret.is_some_and(|c| {
            use leaf_editor::LineLayout as _;
            Some(c.block) == first_row && painted.line_of(c.block, c.at) == 0
        });
        if direction == Vertical::Up && !extend && at_top {
            self.focus_title(window, cx);
            return;
        }
        let moved = if extend {
            page.extend_vertically(direction, &painted).map(|_| ())
        } else {
            page.move_vertically(direction, &painted).map(|_| ())
        };
        if let Err(err) = moved {
            report("could not move the caret", err);
        }
        self.changed(window, cx);
    }

    fn on_select_all(&mut self, _: &SelectAll, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(Err(err)) = self.page().map(leaf_editor::PageEditor::select_all) {
            report("could not select", err);
        }
        self.changed(window, cx);
    }

    fn on_escape(&mut self, _: &Escape, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(page) = self.page() {
            if page.link_query().ok().flatten().is_some() {
                page.link_menu_close();
            } else if page.slash_menu().ok().flatten().is_some() {
                page.slash_close();
            } else if let Some(caret) = page.caret().ok().flatten() {
                // Collapse the selection to the caret.
                let _ = page.set_caret(caret);
            }
        }
        self.changed(window, cx);
    }

    fn format(&mut self, kind: MarkKind, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(Err(err)) = self.page().map(|page| page.toggle_mark(kind)) {
            report("could not format", err);
        }
        self.changed(window, cx);
    }

    fn history(&mut self, undo: bool, window: &mut Window, cx: &mut Context<Self>) {
        let done = self
            .page()
            .map(|page| if undo { page.undo() } else { page.redo() });
        if let Some(Err(err)) = done {
            report("could not undo or redo", err);
        }
        self.changed(window, cx);
    }

    /// Copy puts Markdown on the clipboard for other apps, with Leaf's own fragment alongside so a
    /// paste inside Leaf keeps everything (ADR 0030, ADR 0032).
    fn on_copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(Ok(Some(fragment))) = self.page().map(leaf_editor::PageEditor::copy) {
            cx.write_to_clipboard(self.clipboard_item(&fragment));
        }
    }

    fn on_cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(Ok(Some(fragment))) = self.page().map(leaf_editor::PageEditor::cut) {
            cx.write_to_clipboard(self.clipboard_item(&fragment));
        }
        self.changed(window, cx);
    }

    fn on_paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        let fragment = item.metadata().and_then(|json| Fragment::from_json(json));
        let contents = Pasted::read(&item);
        let pasted = match paste_kind(fragment.is_some(), Some(&contents.text), contents.has_image()) {
            PasteKind::Fragment => {
                fragment.and_then(|fragment| self.page().map(|page| page.paste(&fragment)))
            }
            PasteKind::Text => {
                // `[[Title]]` pastes as a link when exactly one page has the title (E10-S03).
                let titles = self.editor().titles_for_links().unwrap_or_default();
                let find = |title: &str| titles.get(title).copied();
                self.page()
                    .map(|page| page.paste_text_linking(&contents.text, &find))
            }
            PasteKind::Image => {
                let inserted = match (&contents.image, &contents.file) {
                    (Some(bytes), _) => self.insert_image(&ImageSource::Bytes(bytes), window, cx),
                    (None, Some(path)) => self.insert_image(&ImageSource::File(path), window, cx),
                    (None, None) => false,
                };
                // A copied file that isn't a picture pastes as its path, as it always did.
                match &contents.file {
                    Some(path) if !inserted => {
                        let path = path.display().to_string();
                        self.page().map(|page| page.paste_text(&path))
                    }
                    _ => None,
                }
            }
            PasteKind::Nothing => None,
        };
        if let Some(Err(err)) = pasted {
            report("could not paste", err);
        }
        self.changed(window, cx);
    }
}

impl Shell {
    /// What copying `fragment` puts on the clipboard: Markdown as text (page links named `[[Title]]`), the
    /// fragment itself as metadata.
    fn clipboard_item(&self, fragment: &Fragment) -> ClipboardItem {
        let named = fragment.with_link_names(|page| self.editor().link_target(page).name());
        ClipboardItem::new_string_with_metadata(named.to_markdown(), fragment.to_json())
    }
}

/// What a clipboard item holds, by kind: text, a picture, a copied file.
struct Pasted {
    /// The text entries, joined; empty when there are none. Unlike `ClipboardItem::text`, a copied
    /// file's path is not counted as text, so a copied picture pastes as a picture.
    text: String,
    /// The first picture's bytes.
    image: Option<Vec<u8>>,
    /// The first copied file.
    file: Option<std::path::PathBuf>,
}

impl Pasted {
    fn read(item: &ClipboardItem) -> Self {
        let mut pasted = Self {
            text: String::new(),
            image: None,
            file: None,
        };
        for entry in item.entries() {
            match entry {
                ClipboardEntry::String(string) => pasted.text.push_str(string.text()),
                ClipboardEntry::Image(image) if pasted.image.is_none() => {
                    pasted.image = Some(image.bytes().to_vec());
                }
                ClipboardEntry::ExternalPaths(paths) if pasted.file.is_none() => {
                    pasted.file = paths.paths().first().cloned();
                }
                _ => {}
            }
        }
        pasted
    }

    fn has_image(&self) -> bool {
        self.image.is_some() || self.file.is_some()
    }
}
