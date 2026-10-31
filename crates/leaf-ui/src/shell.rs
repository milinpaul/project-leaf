//! The app shell: sidebar on the left, page area on the right.
//!
//! Draws the sidebar view model from `leaf-editor` and forwards clicks and typing back to it. All
//! decisions (what is listed, what "New page" does, when a rename is recorded) live in
//! `WorkspaceEditor`, where they are tested without a window.

use std::fmt::Display;
use std::time::{SystemTime, UNIX_EPOCH};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement, ParentElement,
    Render, Styled, Subscription, Window, div, px,
};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::resizable::{h_resizable, resizable_panel};
use gpui_component::{ActiveTheme as _, v_flex};
use leaf_core::PageId;
use leaf_editor::{UNTITLED, WorkspaceEditor};
use leaf_model::Workspace;
use leaf_store::settings::SIDEBAR_WIDTH_RANGE;

use crate::{settings, theme};

/// Top-level view of a Leaf window.
pub struct Shell {
    editor: WorkspaceEditor,
    /// Title field of the open page. Typing in it renames the page.
    title: Entity<InputState>,
    /// The open page's list, painted layout, caret and drag state.
    blocks: blocks::PageState,
    /// The page's keyboard focus: while it has it, keys and typed text go to the page.
    page_focus: FocusHandle,
    /// The block menu's keyboard focus, while it is open.
    menu_focus: FocusHandle,
    /// The sidebar's page tree: rename in progress, drag hint, scroll position.
    tree: tree::TreeState,
    /// The page icon picker while it is open, and its events.
    icon_picker: Option<(Entity<crate::icon_picker::IconPickerPanel>, Subscription)>,
    /// The cover picker while it is open, and the subscription to what is picked in it.
    cover_picker: Option<(Entity<crate::cover_picker::CoverPickerPanel>, Subscription)>,
    /// The open database page's table and editors (E09-S05), while a database page is open.
    databases: Vec<database_page::DatabasePage>,
    /// The database page whose view the table actions work on (E09-S09: a page can show several inline).
    active_database: Option<leaf_core::PageId>,
    /// The open row page's property editing (E09-S08), while a database row is open as a page.
    row_page: Option<database_page::row_page::RowPage>,
    /// The workspace folder this window saves to; `None` for an in-memory workspace (tests).
    storage: Option<Storage>,
    /// Focus for the window itself, so menu actions and shortcuts reach the shell even when no text
    /// field has the cursor.
    focus: FocusHandle,
    subscriptions: Vec<Subscription>,
}

impl Shell {
    /// Builds the shell for a new window around an empty, in-memory workspace that is never
    /// saved. Used by tests; the app uses [`Shell::with_folder`].
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::build(Workspace::new(), None, window, cx)
    }

    /// Builds the shell for a workspace opened from disk with [`Storage::open`], saving every
    /// change back (autosave, plus a full save on blur, quit and window close).
    pub fn with_storage(
        storage: Storage,
        workspace: Workspace,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let expanded = storage.expanded();
        let last_page = storage.open_page();
        let mut shell = Self::build(workspace, Some(storage), window, cx);
        shell.editor.restore_expanded(expanded);
        // The page that was open when the workspace was last closed opens again (E06-S19).
        if shell.editor.restore_open_page(last_page) {
            shell.load_and_show_selected(window, cx);
        }
        shell.start_autosave(window, cx);
        shell
    }

    fn build(
        workspace: Workspace,
        storage: Option<Storage>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let title = cx.new(|cx| InputState::new(window, cx).placeholder(UNTITLED));
        let subscriptions = vec![
            cx.subscribe_in(&title, window, Self::on_title_event),
            crate::theme::follow_system(window),
            Self::track_window(window, cx),
        ];
        Self::start_edited_clock(cx);
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            editor: WorkspaceEditor::new(workspace),
            title,
            blocks: blocks::PageState::default(),
            page_focus: cx.focus_handle(),
            menu_focus: cx.focus_handle(),
            tree: tree::TreeState::default(),
            icon_picker: None,
            cover_picker: None,
            databases: Vec::new(),
            active_database: None,
            row_page: None,
            storage,
            focus,
            subscriptions,
        }
    }

    /// The editor behind this window, for reading its state.
    #[must_use]
    pub fn editor(&self) -> &WorkspaceEditor {
        &self.editor
    }

    /// The editor behind this window, for tools and tests that script the page (the screenshot tool
    /// places the caret and selection with it).
    pub fn editor_mut(&mut self) -> &mut WorkspaceEditor {
        &mut self.editor
    }

    /// "New page": creates an untitled page, opens it, and puts the cursor in its title.
    pub fn new_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.editor.new_page(now_ms()) {
            Ok(_) => self.show_selected(window, cx),
            Err(err) => report("could not create a page", err),
        }
    }

    /// Opens `page` from the sidebar.
    pub fn open(&mut self, page: PageId, window: &mut Window, cx: &mut Context<Self>) {
        match self.editor.select(page) {
            Ok(()) => self.load_and_show_selected(window, cx),
            Err(err) => report("could not open the page", err),
        }
    }

    /// Puts the keyboard in the page title.
    pub(super) fn focus_title(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.title.update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }

    /// Shows the page the editor has selected: loads its blocks and fills in its title.
    pub(super) fn load_and_show_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.load_selected_page();
        self.show_selected(window, cx);
    }

    /// Loads the open page's title into the title field and focuses it. `set_value` does not emit
    /// change events, so this never records a rename.
    fn show_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let title = match self.editor.selected_title() {
            Ok(title) => title.unwrap_or_default(),
            Err(err) => return report("could not read the page title", err),
        };
        self.title.update(cx, |input, cx| {
            input.set_value(title, window, cx);
            input.focus(window, cx);
        });
        self.remember_open_page();
        cx.notify();
    }

    /// Records the open page in the workspace's view state, so it opens again next launch (E06-S19).
    /// Every way of opening a page ends in `show_selected`, which calls this.
    fn remember_open_page(&mut self) {
        let page = self.editor.selected();
        if let Some(storage) = &mut self.storage
            && let Err(err) = storage.save_open_page(page)
        {
            report("could not remember the open page", err);
        }
    }

    fn on_title_event(
        &mut self,
        input: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Change => {
                let text = input.read(cx).value().to_string();
                if let Err(err) = self.editor.rename_selected(&text) {
                    report("could not rename the page", err);
                }
                cx.notify();
            }
            // Enter in the title moves into the page's first block.
            InputEvent::PressEnter { .. } => self.focus_first_block(window, cx),
            _ => {}
        }
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sidebar = self.render_sidebar(cx);

        let open = self.editor.selected();
        let page = if let Some(open) = open {
            let bar = self.render_top_bar(open, cx);
            v_flex()
                .size_full()
                .child(bar)
                .child(div().flex_1().min_h_0().child(if self.showing_database() {
                    self.render_database_page(window, cx)
                } else {
                    self.render_page(window, cx)
                }))
                .into_any_element()
        } else {
            column(
                div()
                    .pt_16()
                    .text_color(cx.theme().muted_foreground)
                    .child("Create a page with the + beside Pages."),
                false,
            )
            .into_any_element()
        };
        let page = v_flex().flex_1().size_full().child(page);
        let layout = settings::get(cx);
        let body = if layout.sidebar_hidden() {
            // With a page open, the ☰ button sits in its top bar; with none, it floats in the corner.
            div()
                .relative()
                .size_full()
                .child(page)
                .when(open.is_none(), |body| body.child(Self::render_reopen_button(cx)))
                .into_any_element()
        } else {
            // Dragging the divider resizes the sidebar; the width is remembered for the next launch.
            let (min, max) = SIDEBAR_WIDTH_RANGE;
            h_resizable("shell")
                .child(
                    resizable_panel()
                        .size(px(layout.sidebar_width()))
                        .size_range(px(min)..px(max))
                        .fixed()
                        .child(sidebar),
                )
                .child(resizable_panel().child(page))
                .on_resize(|state, _, cx| {
                    if let Some(width) = state.read(cx).sizes().first().copied() {
                        settings::update(cx, |settings| settings.set_sidebar_width(width.into()));
                    }
                })
                .into_any_element()
        };
        Self::with_actions(div().track_focus(&self.focus), cx)
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(body)
    }
}

mod backlinks;
mod block_drag;
mod block_menu;
mod block_row;
mod blocks;
mod bookmark_card;
mod callout;
mod cover;
mod database_page;
mod favourites;
mod format_bar;
mod found;
mod header;
mod image;
mod image_row;
mod ime;
mod language_picker;
mod link_menu_popover;
mod link_popover;
#[cfg(test)]
mod long_document_tests;
mod markers;
mod menu_place;
mod page_keys;
mod page_list;
#[cfg(test)]
mod page_list_window_tests;
mod page_menu;
mod rich_text;
#[cfg(test)]
mod scrollbar_tests;
mod select_scroll;
mod sidebar;
mod sidebar_search;
mod sidebar_toggle;
mod slash_popover;
mod smooth_scroll;
mod storage;
mod table_row;
mod task_pickers;
mod task_row;
mod top_bar;
mod trash;
mod tree;
mod web_bookmark;
mod window;

pub(crate) use database_page::bind as bind_database_keys;
pub(crate) use page_keys::bind as bind_page_keys;
pub use storage::Storage;

impl Drop for Shell {
    /// Closing the window saves everything, even if the app keeps running.
    fn drop(&mut self) {
        self.save(true);
    }
}

/// A centred column no wider than a comfortable line of text, with the page's side margins; for a
/// full-width page (E18-S06), the whole page area within those margins. Every row of the page sits
/// in one, so the virtual list can scroll the full width of the page area.
fn column(content: impl IntoElement, full_width: bool) -> gpui::Div {
    let inner = div().w_full().px_10().child(content);
    div().w_full().flex().justify_center().child(if full_width {
        inner
    } else {
        inner.max_w(px(theme::READING_WIDTH))
    })
}

/// Current time in Unix milliseconds. The UI owns the clock so the model never reads it.
pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| i64::try_from(since.as_millis()).unwrap_or(i64::MAX))
}

/// UI actions have no caller to return errors to. They only fail on a corrupt workspace, so log
/// and keep the window usable. A proper error surface arrives with notifications (E06).
fn report(what: &str, err: impl Display) {
    eprintln!("leaf: {what}: {err}");
}

#[cfg(test)]
mod tests {
    use super::Shell;
    use gpui::{TestAppContext, VisualTestContext};
    use leaf_editor::UNTITLED;

    fn open_shell(cx: &mut TestAppContext) -> (gpui::Entity<Shell>, &mut VisualTestContext) {
        cx.update(|cx| crate::init(None, cx));
        let (shell, cx) = cx.add_window_view(Shell::new);
        cx.update(|window, cx| window.draw(cx).clear(cx));
        (shell, cx)
    }

    fn labels(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext) -> Vec<String> {
        shell.read_with(cx, |shell, _| {
            shell
                .editor()
                .sidebar()
                .unwrap()
                .into_iter()
                .map(|i| i.label)
                .collect()
        })
    }

    #[gpui::test]
    fn starts_with_no_pages(cx: &mut TestAppContext) {
        let (shell, cx) = open_shell(cx);
        assert_eq!(labels(&shell, cx), Vec::<String>::new());
    }

    #[gpui::test]
    fn new_page_then_typing_names_it_in_the_sidebar(cx: &mut TestAppContext) {
        let (shell, cx) = open_shell(cx);
        cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
        assert_eq!(labels(&shell, cx), vec![UNTITLED]);

        cx.simulate_input("Ideas");
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert_eq!(labels(&shell, cx), vec!["Ideas"]);
    }

    #[gpui::test]
    fn opening_a_page_loads_its_title_without_renaming(cx: &mut TestAppContext) {
        let (shell, cx) = open_shell(cx);
        cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
        cx.simulate_input("First");
        let first = shell.read_with(cx, |shell, _| shell.editor().selected().unwrap());
        cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
        cx.simulate_input("Second");

        let before = shell.read_with(cx, |shell, _| shell.editor().workspace().to_bytes().unwrap());
        cx.update(|window, cx| shell.update(cx, |shell, cx| shell.open(first, window, cx)));
        let title = shell.read_with(cx, |shell, cx| shell.title.read(cx).value().to_string());
        assert_eq!(title, "First");
        let after = shell.read_with(cx, |shell, _| shell.editor().workspace().to_bytes().unwrap());
        assert_eq!(before, after, "opening a page records no change");
        assert_eq!(labels(&shell, cx), vec!["First", "Second"]);
    }

    /// The open page's blocks as (depth, text).
    fn blocks(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext) -> Vec<(usize, String)> {
        shell.read_with(cx, |shell, _| {
            let page = shell.editor().selected().unwrap();
            let rows = shell.editor().page_editor(page).unwrap().rows().unwrap();
            rows.into_iter()
                .map(|row| (row.depth, row.text.unwrap_or_default()))
                .collect()
        })
    }

    #[gpui::test]
    fn typing_enter_and_tab_edit_the_page_blocks(cx: &mut TestAppContext) {
        let (shell, cx) = open_shell(cx);
        cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
        cx.simulate_keystrokes("enter"); // from the title into the first block
        cx.simulate_input("Hello");
        cx.simulate_keystrokes("enter");
        cx.simulate_input("World");
        cx.simulate_keystrokes("tab");
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert_eq!(blocks(&shell, cx), vec![(0, "Hello".into()), (1, "World".into())]);

        cx.simulate_keystrokes("shift-tab");
        assert_eq!(blocks(&shell, cx), vec![(0, "Hello".into()), (0, "World".into())]);
    }

    #[gpui::test]
    fn backspace_in_an_empty_block_removes_it_and_focuses_the_one_above(cx: &mut TestAppContext) {
        let (shell, cx) = open_shell(cx);
        cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
        cx.simulate_keystrokes("enter");
        cx.simulate_input("Keep");
        let first = shell.update(cx, |shell, _| shell.caret_block().unwrap());
        cx.simulate_keystrokes("enter");
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert_eq!(blocks(&shell, cx).len(), 2);

        cx.simulate_keystrokes("backspace");
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert_eq!(blocks(&shell, cx), vec![(0, "Keep".into())]);
        assert_eq!(shell.update(cx, |shell, _| shell.caret_block()), Some(first));
        cx.simulate_keystrokes("backspace");
        assert_eq!(
            blocks(&shell, cx),
            vec![(0, "Kee".into())],
            "a non-empty block gets an ordinary backspace"
        );
    }

    #[gpui::test]
    fn enter_on_an_empty_list_item_ends_the_list(cx: &mut TestAppContext) {
        let (shell, cx) = open_shell(cx);
        cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
        cx.simulate_keystrokes("enter");
        cx.update(|window, cx| {
            shell.update(cx, |shell, cx| {
                shell.turn_focused_into(leaf_model::BlockKind::BulletedList, window, cx);
            });
        });
        cx.simulate_input("Item");
        cx.simulate_keystrokes("enter enter");
        cx.simulate_input("After");
        assert_eq!(
            blocks(&shell, cx),
            vec![(0, "Item".into()), (0, "After".into())],
            "no stray empty bullet"
        );
        let styles: Vec<leaf_editor::BlockStyle> = shell.read_with(cx, |shell, _| {
            let page = shell.editor().selected().unwrap();
            shell
                .editor()
                .page_editor(page)
                .unwrap()
                .rows()
                .unwrap()
                .into_iter()
                .map(|row| row.style)
                .collect()
        });
        assert_eq!(
            styles,
            vec![
                leaf_editor::BlockStyle::Bulleted,
                leaf_editor::BlockStyle::Paragraph
            ]
        );
    }

    #[gpui::test]
    fn enter_mid_text_splits_and_backspace_at_start_joins_again(cx: &mut TestAppContext) {
        let (shell, cx) = open_shell(cx);
        cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
        cx.simulate_keystrokes("enter");
        cx.simulate_input("Hello world");
        cx.simulate_keystrokes("left left left left left left enter");
        assert_eq!(
            blocks(&shell, cx),
            vec![(0, "Hello".into()), (0, " world".into())]
        );

        cx.simulate_keystrokes("backspace");
        assert_eq!(
            blocks(&shell, cx),
            vec![(0, "Hello world".into())],
            "joined back together"
        );
        cx.simulate_input("!");
        assert_eq!(
            blocks(&shell, cx),
            vec![(0, "Hello! world".into())],
            "the caret sits at the join"
        );
    }
}
