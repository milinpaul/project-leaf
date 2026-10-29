//! The interim block editor (E07-S10): the open page's blocks, one plain text input each.
//!
//! Draws `BlockRow`s from `leaf-editor` and forwards typing and keys to `PageEditor`, which decides
//! what they mean. Keys the input would otherwise handle itself (Tab, Backspace in an empty block, the
//! arrows) are caught in the capture phase on the page area. The rich text element (E07-S02) replaces
//! the inputs; the editor logic stays.

use std::collections::HashMap;

use gpui::{
    AnyElement, AppContext as _, ClickEvent, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement, SharedString, Styled, Subscription, Window, div, list, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{
    Backspace as BackspaceKey, IndentInline, InputEvent, InputState, MoveDown, MoveUp, OutdentInline,
};
use gpui_component::{h_flex, v_flex};
use leaf_core::BlockId;
use leaf_editor::{Backspace, Caret, PageEditor};
use leaf_model::BlockKind;

use super::column;
use super::page_list::{HEADER_ITEMS, PageList};
use super::{Shell, report};

/// The block inputs of the open page and which one has focus.
#[derive(Default)]
pub(super) struct BlockInputs {
    pub(super) inputs: HashMap<BlockId, Entity<InputState>>,
    subscriptions: HashMap<BlockId, Subscription>,
    pub(super) focused: Option<BlockId>,
    /// The page's rows as a virtual list: only those on screen are laid out.
    pub(super) list: PageList,
}

/// The "Turn into" toolbar: label and the kind each button applies.
fn toolbar_kinds() -> [(&'static str, BlockKind); 9] {
    [
        ("Text", BlockKind::Paragraph),
        ("H1", BlockKind::Heading { level: 1 }),
        ("H2", BlockKind::Heading { level: 2 }),
        ("H3", BlockKind::Heading { level: 3 }),
        ("• List", BlockKind::BulletedList),
        ("1. List", BlockKind::NumberedList),
        ("To-do", BlockKind::Todo { checked: false }),
        ("Quote", BlockKind::Quote),
        (
            "Code",
            BlockKind::Code {
                language: String::new(),
            },
        ),
    ]
}

impl Shell {
    /// The open page's editor, reporting (and hiding) errors so the window stays usable.
    fn page(&mut self) -> Option<&mut PageEditor> {
        self.editor.open_page().unwrap_or_else(|err| {
            report("could not open the page's blocks", err);
            None
        })
    }

    /// The block that currently has the keyboard, if any.
    #[must_use]
    pub fn focused_block(&self) -> Option<BlockId> {
        self.blocks.focused
    }

    /// The input for `id`, created and wired up the first time it is needed.
    pub(super) fn block_input(
        &mut self,
        id: BlockId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some(input) = self.blocks.inputs.get(&id) {
            return input.clone();
        }
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Write something…"));
        let subscription = cx.subscribe_in(&input, window, move |shell, input, event, window, cx| {
            shell.on_block_event(id, input, event, window, cx);
        });
        self.blocks.inputs.insert(id, input.clone());
        self.blocks.subscriptions.insert(id, subscription);
        input
    }

    /// Puts the keyboard in `id`'s input.
    pub fn focus_block(&mut self, id: BlockId, window: &mut Window, cx: &mut Context<Self>) {
        let text = self
            .page()
            .and_then(|page| page.doc().plain_text(id).ok().flatten())
            .unwrap_or_default();
        let input = self.block_input(id, window, cx);
        input.update(cx, |input, cx| {
            if input.value() != text {
                input.set_value(text, window, cx);
            }
            input.focus(window, cx);
        });
        self.blocks.focused = Some(id);
        // The block may be scrolled out of view (arrow keys, Enter at the bottom of the screen).
        self.blocks.list.reveal(id);
        cx.notify();
    }

    /// Sets a block's input to the model's text without emitting change events.
    fn sync_input(&mut self, id: BlockId, window: &mut Window, cx: &mut Context<Self>) {
        let text = self
            .page()
            .and_then(|page| page.doc().plain_text(id).ok().flatten())
            .unwrap_or_default();
        if let Some(input) = self.blocks.inputs.get(&id) {
            input.update(cx, |input, cx| {
                if input.value() != text {
                    input.set_value(text, window, cx);
                }
            });
        }
    }

    /// Focuses a block and puts the text cursor at `caret.at` (a Unicode scalar position).
    fn focus_caret(&mut self, caret: Caret, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_block(caret.block, window, cx);
        if let Some(input) = self.blocks.inputs.get(&caret.block) {
            input.update(cx, |input, cx| {
                let byte = byte_offset(&input.value(), caret.at);
                input.set_selected_range(byte..byte, cx);
            });
        }
    }

    /// How many blocks the open page has, as last drawn.
    #[must_use]
    pub fn page_block_count(&self) -> usize {
        self.blocks.list.rows.len()
    }

    /// Scrolls the open page so block number `index` (in reading order, from 0) is at the top. Used
    /// to show a search result in place, and by the screenshot tool.
    pub fn scroll_page_to(&mut self, index: usize, cx: &mut Context<Self>) {
        self.blocks.list.scroll_to_block(index);
        cx.notify();
    }

    /// Focuses the open page's first block, so a freshly opened page is ready for typing.
    pub(super) fn focus_first_block(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let first = self.page().and_then(|page| page.ensure_block().ok());
        if let Some(first) = first {
            self.focus_block(first, window, cx);
        }
    }

    fn on_block_event(
        &mut self,
        id: BlockId,
        input: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Change => {
                let text = input.read(cx).value().to_string();
                if let Some(Err(err)) = self.page().map(|page| page.set_text(id, &text)) {
                    report("could not edit the block", err);
                }
                cx.notify();
            }
            InputEvent::PressEnter { .. } => {
                let at = {
                    let input = input.read(cx);
                    char_offset(&input.value(), input.cursor())
                };
                match self.page().map(|page| page.enter_at(id, at)) {
                    Some(Ok(caret)) => {
                        // The split shortened this block. Refresh its input now: it may still report a
                        // late change with its old text, which would otherwise undo the split.
                        self.sync_input(id, window, cx);
                        self.focus_caret(caret, window, cx);
                    }
                    Some(Err(err)) => report("could not split the block", err),
                    None => {}
                }
            }
            InputEvent::Focus => self.blocks.focused = Some(id),
            InputEvent::Blur => {}
        }
    }

    /// Backspace with the cursor at the very start of a block (and nothing selected) is a block
    /// action; anywhere else it is an ordinary backspace for the input.
    fn on_backspace(&mut self, _: &BackspaceKey, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.blocks.focused else { return };
        let at_start = self.blocks.inputs.get(&id).is_some_and(|input| {
            let input = input.read(cx);
            input.cursor() == 0 && input.selected_range().is_empty()
        });
        if !at_start {
            return;
        }
        match self.page().map(|page| page.backspace_at_start(id)) {
            Some(Ok(Backspace::Moved(caret))) => {
                cx.stop_propagation();
                self.focus_caret(caret, window, cx);
            }
            // Selecting a divider or image arrives with block selection (E04-S06); for now, do nothing.
            Some(Ok(Backspace::Select(_))) => cx.stop_propagation(),
            Some(Ok(Backspace::Nothing)) | None => {}
            Some(Err(err)) => report("could not merge the block", err),
        }
    }

    fn on_indent(&mut self, _: &IndentInline, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        self.with_focused(window, cx, PageEditor::indent, "could not indent");
    }

    fn on_outdent(&mut self, _: &OutdentInline, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        self.with_focused(window, cx, PageEditor::outdent, "could not outdent");
    }

    fn on_move_up(&mut self, _: &MoveUp, window: &mut Window, cx: &mut Context<Self>) {
        self.move_focus(window, cx, PageEditor::previous);
    }

    fn on_move_down(&mut self, _: &MoveDown, window: &mut Window, cx: &mut Context<Self>) {
        self.move_focus(window, cx, PageEditor::next);
    }

    fn move_focus(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        neighbour: impl Fn(&PageEditor, BlockId) -> Result<Option<BlockId>, leaf_editor::EditorError>,
    ) {
        let Some(id) = self.blocks.focused else { return };
        if let Some(Ok(Some(target))) = self.page().map(|page| neighbour(page, id)) {
            cx.stop_propagation();
            self.focus_block(target, window, cx);
        }
    }

    /// Runs an editor action on the focused block, then keeps the keyboard there.
    fn with_focused(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        action: impl FnOnce(&mut PageEditor, BlockId) -> Result<(), leaf_editor::EditorError>,
        failure: &str,
    ) {
        let Some(id) = self.blocks.focused else { return };
        if let Some(Err(err)) = self.page().map(|page| action(page, id)) {
            report(failure, err);
        }
        self.focus_block(id, window, cx);
    }

    /// Applies a toolbar kind to the focused block.
    pub fn turn_focused_into(&mut self, kind: BlockKind, window: &mut Window, cx: &mut Context<Self>) {
        self.with_focused(
            window,
            cx,
            |page, id| page.turn_into(id, kind),
            "could not change the block",
        );
    }

    /// Ticks or unticks a to-do.
    pub fn toggle_todo(&mut self, id: BlockId, cx: &mut Context<Self>) {
        if let Some(Err(err)) = self.page().map(|page| page.toggle_checked(id)) {
            report("could not tick the to-do", err);
        }
        cx.notify();
    }

    /// The open page: the "Turn into" toolbar, then the title and blocks as a virtual list that
    /// scrolls. Only the rows on screen are built, so only they get text inputs.
    pub(super) fn render_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        // Only blocks that changed since the last frame are re-read (E04-S01).
        let rows = match self.page().map(|page| page.view().map(<[_]>::to_vec)) {
            Some(Ok(rows)) => rows,
            Some(Err(err)) => {
                report("could not read the page's blocks", err);
                Vec::new()
            }
            None => Vec::new(),
        };
        // Drop inputs of blocks that are gone (deleted, or another page's).
        let live: std::collections::HashSet<BlockId> = rows.iter().map(|row| row.id).collect();
        self.blocks.inputs.retain(|id, _| live.contains(id));
        self.blocks.subscriptions.retain(|id, _| live.contains(id));
        self.blocks.list.sync(self.editor.selected(), rows);

        let toolbar = h_flex()
            .gap_1()
            .children(toolbar_kinds().into_iter().map(|(label, kind)| {
                Button::new(SharedString::from(format!("turn-into-{label}")))
                    .label(label)
                    .ghost()
                    .on_click(cx.listener(move |shell, _: &ClickEvent, window, cx| {
                        shell.turn_focused_into(kind.clone(), window, cx);
                    }))
            }));
        let rows = list(self.blocks.list.state.clone(), cx.processor(Self::render_item)).size_full();

        v_flex()
            .size_full()
            .capture_action(cx.listener(Self::on_backspace))
            .capture_action(cx.listener(Self::on_indent))
            .capture_action(cx.listener(Self::on_outdent))
            .capture_action(cx.listener(Self::on_move_up))
            .capture_action(cx.listener(Self::on_move_down))
            .child(column(div().pt_2().pb_1().child(toolbar)))
            .child(rows)
            .into_any_element()
    }

    /// List item `item`: the page header (icon and title) first, then one block each. The header is
    /// part of the list so it scrolls away with the page.
    fn render_item(&mut self, item: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let content = if item < HEADER_ITEMS {
            div()
                .pt_16()
                .pb_3()
                .child(self.render_page_header(cx))
                .into_any_element()
        } else if let Some(row) = self.blocks.list.row(item).cloned() {
            self.render_row(&row, window, cx)
        } else {
            div().into_any_element()
        };
        column(div().py(px(1.0)).child(content)).into_any_element()
    }
}

/// Converts a Unicode scalar position to a UTF-8 byte offset in `text` (inputs count bytes).
fn byte_offset(text: &str, chars: usize) -> usize {
    text.char_indices()
        .nth(chars)
        .map_or(text.len(), |(byte, _)| byte)
}

/// Converts a UTF-8 byte offset from an input to a Unicode scalar position (the model counts characters).
fn char_offset(text: &str, byte: usize) -> usize {
    text.get(..byte)
        .map_or_else(|| text.chars().count(), |head| head.chars().count())
}
