//! A database inside a page (E09-S09): its block draws the database's title, its sort and filter bar, its table
//! (about ten rows tall, then it scrolls) and "New", all editing exactly as on the database's own page.
//!
//! Each inline database has its own view (inputs, table, menus), kept beside the open page's. The table actions
//! work on the active view, so any press inside an inline database makes it the active one first, and drawing one
//! makes it active for the length of the drawing.
//!
//! The page around it has keys of its own (Backspace deletes text, Cmd+A selects blocks, and so on). The table's
//! wrapper takes the database page's key context, which binds those keys to nothing, so while the table has the
//! keyboard they never reach the page. Clicks inside don't reach the page either.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement, KeyBinding, MouseButton, MouseDownEvent,
    NoAction, ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    px,
};
use gpui_component::button::ButtonVariants as _;
use gpui_component::table::DataTable;
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, Size, h_flex, v_flex};
use gpui_kit_assets::IconName;
use leaf_core::{BlockId, PageId};
use leaf_editor::BlockRow;

use super::super::block_row::indent_of;
use super::super::{Shell, now_ms, report};
use super::{CONTEXT, ROW_HEIGHT, TABLE_BORDERS};

/// How many rows an inline table shows before it scrolls.
const INLINE_ROWS: usize = 10;

/// The page's keys that mean nothing inside a database's table, so they never reach the page around an inline
/// one. Enter, Escape, undo and redo are the table's own (`super::bind`).
pub(super) fn bind(cx: &mut App) {
    const EVERYWHERE: [&str; 17] = [
        "backspace",
        "shift-backspace",
        "delete",
        "tab",
        "shift-tab",
        "left",
        "right",
        "up",
        "down",
        "home",
        "end",
        "shift-left",
        "shift-right",
        "shift-up",
        "shift-down",
        "shift-home",
        "shift-end",
    ];
    const MAC: [&str; 22] = [
        "cmd-enter",
        "cmd-a",
        "cmd-b",
        "cmd-i",
        "cmd-e",
        "cmd-shift-s",
        "cmd-c",
        "cmd-x",
        "cmd-v",
        "cmd-/",
        "cmd-shift-d",
        "cmd-shift-p",
        "alt-left",
        "alt-right",
        "cmd-left",
        "cmd-right",
        "alt-shift-left",
        "alt-shift-right",
        "cmd-shift-left",
        "cmd-shift-right",
        "ctrl-a",
        "ctrl-e",
    ];
    const OTHERS: [&str; 16] = [
        "ctrl-enter",
        "ctrl-a",
        "ctrl-b",
        "ctrl-i",
        "ctrl-e",
        "ctrl-shift-s",
        "ctrl-c",
        "ctrl-x",
        "ctrl-v",
        "ctrl-/",
        "ctrl-shift-d",
        "ctrl-shift-p",
        "ctrl-left",
        "ctrl-right",
        "ctrl-shift-left",
        "ctrl-shift-right",
    ];
    let own: &[&str] = if cfg!(target_os = "macos") { &MAC } else { &OTHERS };
    cx.bind_keys(
        EVERYWHERE
            .iter()
            .chain(own)
            .map(|key| KeyBinding::new(key, NoAction, Some(CONTEXT))),
    );
}

impl Shell {
    /// The block `row` showing the database page `page`.
    pub(in crate::shell) fn render_inline_database(
        &mut self,
        row: &BlockRow,
        page: PageId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let indent = indent_of(row);
        let database = self.editor.workspace().page(page).ok().filter(|_| {
            self.editor
                .workspace()
                .is_in_trash(page)
                .is_ok_and(|trashed| !trashed)
        });
        let Some((title, id)) = database.and_then(|meta| Some((meta.title, meta.database?))) else {
            let words = if self.editor.workspace().contains(page) {
                "This database is in the Trash."
            } else {
                "This database was deleted."
            };
            return div()
                .pl(indent)
                .py_1()
                .text_color(cx.theme().muted_foreground)
                .child(words)
                .into_any_element();
        };
        self.ensure_database_view(page, id, window, cx);
        // Drawn as the active view, so the bar and table are this database's; the active one is put back after.
        let active = self.active_database.replace(page);
        let body = self.render_inline_body(row.id, page, title, cx);
        self.active_database = active;
        h_flex().w_full().pl(indent).py_1().child(body).into_any_element()
    }

    fn render_inline_body(
        &mut self,
        block: BlockId,
        page: PageId,
        title: String,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(table) = self.database_view().map(|d| d.table.clone()) else {
            return div().into_any_element();
        };
        let rows = table.read(cx).delegate().view.rows.len().min(INLINE_ROWS);
        let height = ROW_HEIGHT * (f32::from(u16::try_from(rows).unwrap_or(u16::MAX)) + 1.0) + TABLE_BORDERS;
        let query_bar = self.render_query_bar(cx);
        let selected = self.blocks.highlights.contains_key(&block);
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let accent = theme.primary;
        let heading = h_flex()
            .id("inline-database-title")
            .debug_selector(move || format!("inline-database-title-{page}"))
            .gap_2()
            .items_center()
            .h(px(ROW_HEIGHT))
            .group("inline-database-title")
            .child(Icon::new(IconName::Table).small().text_color(muted))
            .child(
                div()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(if title.is_empty() { muted } else { theme.foreground })
                    .child(if title.is_empty() {
                        SharedString::from("Untitled")
                    } else {
                        SharedString::from(title)
                    }),
            )
            .child(
                crate::theme::button("inline-database-open")
                    .debug_selector(move || format!("inline-database-open-{page}"))
                    .ghost()
                    .xsmall()
                    .icon(IconName::ExternalLink)
                    .label("Open")
                    .on_click(cx.listener(move |shell, _, window, cx| shell.open(page, window, cx))),
            )
            // A press beside the title selects the whole block, so Backspace or the block menu can delete it.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |shell, _: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    shell.select_database_block(block, window, cx);
                }),
            );
        let new_row = h_flex()
            .id("inline-database-new-row")
            .debug_selector(move || format!("inline-database-new-row-{page}"))
            .h(px(ROW_HEIGHT))
            .px_3()
            .gap_2()
            .items_center()
            .text_color(muted)
            .cursor_pointer()
            .hover(|s| s.bg(cx.theme().muted))
            .child(Icon::new(IconName::Plus).small())
            .child("New")
            .on_click(cx.listener(|shell, _, window, cx| shell.add_database_row(window, cx)));
        Self::with_database_actions(
            v_flex()
                .id(SharedString::from(format!("inline-database-{page}")))
                .debug_selector(move || format!("inline-database-{page}"))
                .key_context(CONTEXT)
                .w_full()
                .min_w_0()
                .gap_1()
                .rounded(cx.theme().radius)
                .border_1()
                .border_color(if selected {
                    accent
                } else {
                    gpui::transparent_black()
                })
                .when(selected, |d| d.bg(accent.opacity(0.08)))
                // Every press inside works on this database, and none reaches the page around it.
                .capture_any_mouse_down(cx.listener(move |shell, _, _, _| shell.activate_database(page)))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation()),
            cx,
        )
        .child(heading)
        .child(query_bar)
        .child(
            div().w_full().h(px(height)).child(
                DataTable::new(&table)
                    .with_size(Size::Medium)
                    .bordered(true)
                    .header_row_height(px(ROW_HEIGHT))
                    .scrollbar_visible(true, true),
            ),
        )
        .child(new_row)
        .into_any_element()
    }

    /// Selects the inline database's block, giving the page the keyboard.
    fn select_database_block(&mut self, block: BlockId, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(Err(err)) = self.page().map(|page| page.select_blocks(&[block])) {
            report("could not select the database", err);
        }
        self.focus_page(window, cx);
        self.changed(window, cx);
    }

    /// After a page change: makes the database `/database` asked for, and trashes or brings back the databases
    /// whose blocks left the page or came back. The sidebar shows the result on the next drawing.
    pub(in crate::shell) fn follow_inline_databases(&mut self) {
        if let Err(err) = self.editor.make_requested_database(now_ms()) {
            report("could not add the database", err);
        }
        if let Err(err) = self.editor.sync_inline_databases(now_ms()) {
            report("could not move the database to or from the Trash", err);
        }
    }

    /// Leaves out the views of databases that aren't shown any more. Kept: the open page's own, those inline on it,
    /// and the one the open page is a row of (its properties edit through it).
    pub(in crate::shell) fn prune_database_views(&mut self) {
        let Some(open) = self.editor.selected() else {
            self.databases.clear();
            self.active_database = None;
            return;
        };
        let workspace = self.editor.workspace();
        let around = workspace.parent(open).ok().flatten();
        self.databases.retain(|d| {
            d.page == open || Some(d.page) == around || workspace.parent(d.page).ok().flatten() == Some(open)
        });
        if self
            .active_database
            .is_some_and(|page| !self.databases.iter().any(|d| d.page == page))
        {
            self.active_database = None;
        }
    }
}
