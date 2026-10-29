//! The sidebar's page tree: forwards clicks, the context menu, inline renaming and drag-and-drop to
//! `WorkspaceEditor`, which decides what they mean (where a drop lands, what trashing closes).
//!
//! Drawing the rows is in `row`. Which pages are expanded is saved in the workspace folder's
//! `view.json` as soon as it changes, so the tree reopens the way it was left.

use std::rc::Rc;

use gpui::{
    AnyElement, AppContext as _, Context, DragMoveEvent, Entity, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement, Render, SharedString, Styled, Subscription, UniformListScrollHandle, Window,
    div, uniform_list,
};
use gpui_component::ActiveTheme as _;
use gpui_component::input::{InputEvent, InputState};
use leaf_core::PageId;
use leaf_editor::{DropPlace, SidebarItem};

use super::{Shell, now_ms, report};

mod row;

/// The tree's state between frames: the rows last drawn, the page being renamed and where a drag
/// would drop.
#[derive(Default)]
pub(super) struct TreeState {
    /// The rows of the last frame; the list draws only the visible ones from this.
    rows: Rc<Vec<SidebarItem>>,
    renaming: Option<Rename>,
    /// Where the page being dragged would land, if the pointer is over an allowed place.
    drop_hint: Option<DropPlace>,
    scroll: UniformListScrollHandle,
}

/// An inline rename in progress: the row's text field.
struct Rename {
    page: PageId,
    input: Entity<InputState>,
    _events: Subscription,
}

/// What is carried while a page row is dragged, and the little label that follows the pointer.
#[derive(Clone)]
pub(super) struct DraggedPage {
    pub(super) page: PageId,
    label: SharedString,
}

impl Render for DraggedPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded(cx.theme().radius)
            .bg(cx.theme().popover)
            .border_1()
            .border_color(cx.theme().border)
            .shadow_md()
            .text_sm()
            .opacity(0.9)
            .child(self.label.clone())
    }
}

impl Shell {
    /// Shows or hides `page`'s children.
    pub fn toggle_expanded(&mut self, page: PageId, cx: &mut Context<Self>) {
        self.editor.toggle_expanded(page);
        self.view_changed();
        cx.notify();
    }

    /// "Add page inside": a new untitled page at the end of `parent`, opened with the cursor in its
    /// title.
    pub fn new_page_inside(&mut self, parent: PageId, window: &mut Window, cx: &mut Context<Self>) {
        match self.editor.new_page_in(Some(parent), now_ms()) {
            Ok(_) => {
                self.view_changed();
                self.show_selected(window, cx);
            }
            Err(err) => report("could not create a page", err),
        }
    }

    /// "Rename": turns `page`'s row into a text field holding its title, all selected. Enter or
    /// clicking away keeps the new title; Escape keeps the old one.
    pub fn start_rename(&mut self, page: PageId, window: &mut Window, cx: &mut Context<Self>) {
        let title = match self.editor.workspace().page(page) {
            Ok(meta) => meta.title,
            Err(err) => return report("could not read the page title", err),
        };
        let input = cx.new(|cx| InputState::new(window, cx).default_value(title));
        input.update(cx, |input, cx| {
            input.focus(window, cx);
            input.select_all(window, cx);
        });
        let events = cx.subscribe_in(&input, window, |shell, input, event, window, cx| {
            let current = shell
                .tree
                .renaming
                .as_ref()
                .map(|rename| rename.input.entity_id());
            if current != Some(input.entity_id()) {
                return; // a rename that already ended
            }
            if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                shell.finish_rename(true, window, cx);
            }
        });
        self.tree.renaming = Some(Rename {
            page,
            input,
            _events: events,
        });
        cx.notify();
    }

    /// Ends the inline rename, saving the typed title if `keep`.
    fn finish_rename(&mut self, keep: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(rename) = self.tree.renaming.take() else {
            return;
        };
        if keep {
            let title = rename.input.read(cx).value().to_string();
            if let Err(err) = self.editor.rename(rename.page, &title) {
                report("could not rename the page", err);
            }
            if self.editor.selected() == Some(rename.page) {
                // The open page's heading shows the same title.
                self.title
                    .update(cx, |input, cx| input.set_value(title, window, cx));
            }
        }
        if rename.input.read(cx).focus_handle(cx).is_focused(window) {
            window.focus(&self.focus, cx);
        }
        cx.notify();
    }

    /// "Move to Trash": hides `page` and everything inside it. If the open page went with it, the
    /// window shows no page.
    pub fn trash_page(&mut self, page: PageId, cx: &mut Context<Self>) {
        if self
            .tree
            .renaming
            .as_ref()
            .is_some_and(|rename| rename.page == page)
        {
            self.tree.renaming = None;
        }
        if let Err(err) = self.editor.trash(page, now_ms()) {
            report("could not move the page to the trash", err);
        }
        cx.notify();
    }

    /// Drops a dragged `page` at `place`.
    pub fn drop_page(&mut self, page: PageId, place: DropPlace, cx: &mut Context<Self>) {
        self.tree.drop_hint = None;
        match self.editor.move_page(page, place) {
            Ok(_) => self.view_changed(),
            Err(err) => report("could not move the page", err),
        }
        cx.notify();
    }

    /// Records where a drag of `page` would land, so the row can show it. Only allowed places are
    /// shown: a page can't go inside itself.
    fn hint_drop(&mut self, page: PageId, place: DropPlace, cx: &mut Context<Self>) {
        let hint = self
            .editor
            .can_drop(page, place)
            .unwrap_or(false)
            .then_some(place);
        if self.tree.drop_hint != hint {
            self.tree.drop_hint = hint;
            cx.notify();
        }
    }

    /// Saves which pages are expanded, if that changed since it was last saved.
    fn view_changed(&mut self) {
        let Some(storage) = &mut self.storage else {
            return;
        };
        if let Err(err) = storage.save_expanded(&self.editor.expanded_pages()) {
            report("could not save the sidebar state", err);
        }
    }

    /// The page tree: a list that draws only the rows on screen, so a workspace with hundreds of
    /// pages scrolls as smoothly as one with five. The empty space below the rows takes drops for the
    /// end of the top level.
    pub(super) fn render_tree(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let rows = self.editor.sidebar().unwrap_or_else(|err| {
            report("could not list pages", err);
            Vec::new()
        });
        self.tree.rows = Rc::new(rows);
        if !cx.has_active_drag() {
            self.tree.drop_hint = None;
        }
        let list = uniform_list(
            "page-tree",
            self.tree.rows.len(),
            cx.processor(|shell, range: std::ops::Range<usize>, _, cx| {
                let rows = Rc::clone(&shell.tree.rows);
                range
                    .filter_map(|ix| {
                        rows.get(ix)
                            .map(|item| shell.render_tree_row(ix, item, rows.len(), cx))
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .track_scroll(&self.tree.scroll)
        .size_full();
        div()
            .id("page-tree-area")
            .debug_selector(|| "page-tree-area".into())
            .flex_1()
            .min_h_0()
            .on_drag_move(cx.listener(|shell, event: &DragMoveEvent<DraggedPage>, _, cx| {
                if event.bounds.contains(&event.event.position) {
                    let page = event.drag(cx).page;
                    shell.hint_drop(page, DropPlace::End, cx);
                }
            }))
            .on_drop(cx.listener(|shell, dragged: &DraggedPage, _, cx| {
                shell.drop_page(dragged.page, DropPlace::End, cx);
            }))
            .child(list)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests;
