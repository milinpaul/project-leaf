//! The app shell: sidebar on the left, page area on the right.
//!
//! Draws the sidebar view model from `leaf-editor` and forwards clicks and typing back to it. All
//! decisions (what is listed, what "New page" does, when a rename is recorded) live in
//! `WorkspaceEditor`, where they are tested without a window.

use std::fmt::Display;
use std::time::{SystemTime, UNIX_EPOCH};

use gpui::{
    AppContext as _, ClickEvent, Context, Entity, FontWeight, IntoElement, ParentElement, Render, Styled,
    Subscription, Window, div, rems,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::sidebar::{Sidebar, SidebarGroup, SidebarMenu, SidebarMenuItem};
use gpui_component::{ActiveTheme as _, h_flex, v_flex};
use leaf_core::PageId;
use leaf_editor::{SidebarItem, UNTITLED, WorkspaceEditor};
use leaf_model::Workspace;

/// Top-level view of a Leaf window.
pub struct Shell {
    editor: WorkspaceEditor,
    /// Title field of the open page. Typing in it renames the page.
    title: Entity<InputState>,
    /// The open page's block inputs (interim editor, E07-S10).
    blocks: blocks::BlockInputs,
    /// The workspace folder this window saves to; `None` for an in-memory workspace (tests).
    storage: Option<Storage>,
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
        let mut shell = Self::build(workspace, Some(storage), window, cx);
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
        let subscriptions = vec![cx.subscribe_in(&title, window, Self::on_title_event)];
        Self {
            editor: WorkspaceEditor::new(workspace),
            title,
            blocks: blocks::BlockInputs::default(),
            storage,
            subscriptions,
        }
    }

    /// The editor behind this window, for reading its state.
    #[must_use]
    pub fn editor(&self) -> &WorkspaceEditor {
        &self.editor
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
            Ok(()) => {
                self.load_selected_page();
                self.show_selected(window, cx);
            }
            Err(err) => report("could not open the page", err),
        }
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
        cx.notify();
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

    fn sidebar_row(item: SidebarItem, cx: &mut Context<Self>) -> SidebarMenuItem {
        let label = match item.icon {
            Some(icon) => format!("{icon} {}", item.label),
            None => item.label,
        };
        let shell = cx.entity().downgrade();
        let page = item.page;
        SidebarMenuItem::new(label)
            .active(item.selected)
            .on_click(move |_, window, cx| {
                // The shell outlives its own sidebar rows; if it is gone the window is closing anyway.
                let _ = shell.update(cx, |shell, cx| shell.open(page, window, cx));
            })
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let items = self.editor.sidebar().unwrap_or_else(|err| {
            report("could not list pages", err);
            Vec::new()
        });
        let rows: Vec<_> = items
            .into_iter()
            .map(|item| Self::sidebar_row(item, cx))
            .collect();
        let new_page = Button::new("new-page")
            .label("New page")
            .ghost()
            .on_click(cx.listener(|shell, _: &ClickEvent, window, cx| shell.new_page(window, cx)));
        let sidebar = Sidebar::new("sidebar")
            .header(new_page)
            .child(SidebarGroup::new("Pages").child(SidebarMenu::new().children(rows)));

        let page = v_flex().flex_1().h_full().p_6().gap_3();
        let page = if self.editor.selected().is_some() {
            // The title reads as the page's heading 1, not a form field: no box, large and bold.
            page.child(
                Input::new(&self.title)
                    .appearance(false)
                    .focus_bordered(false)
                    .text_3xl()
                    .font_weight(FontWeight::BOLD)
                    // The input's own height and line height fit body text; size them to the heading.
                    .h_12()
                    .line_height(rems(2.25))
                    .px_0(),
            )
            .child(self.render_blocks(window, cx))
        } else {
            page.child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child("Create a page with New page."),
            )
        };
        h_flex()
            .size_full()
            .items_start()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(sidebar)
            .child(page)
    }
}

mod blocks;
mod storage;

pub use storage::Storage;

impl Drop for Shell {
    /// Closing the window saves everything, even if the app keeps running.
    fn drop(&mut self) {
        self.save(true);
    }
}

/// Current time in Unix milliseconds. The UI owns the clock so the model never reads it.
fn now_ms() -> i64 {
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
        cx.update(crate::init);
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
        let first = shell.read_with(cx, |shell, _| shell.focused_block().unwrap());
        cx.simulate_keystrokes("enter");
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert_eq!(blocks(&shell, cx).len(), 2);

        cx.simulate_keystrokes("backspace");
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert_eq!(blocks(&shell, cx), vec![(0, "Keep".into())]);
        assert_eq!(shell.read_with(cx, |shell, _| shell.focused_block()), Some(first));
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
