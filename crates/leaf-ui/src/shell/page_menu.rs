//! A page's menu, drawn from `leaf_editor`'s `page_menu` list (E18-S04): the top bar's `⋯` and the
//! sidebar's right-click menu both build their popup here and run the chosen action through
//! [`Shell::run_page_action`], so neither has its own copy of what the actions do.

use gpui::{
    App, Context, InteractiveElement as _, IntoElement as _, ParentElement as _, Styled as _, WeakEntity,
    Window, px,
};
use gpui_component::button::ButtonVariants as _;
use gpui_component::menu::{PopupMenu, PopupMenuItem};
use gpui_component::notification::Notification;
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, WindowExt as _, h_flex};
use gpui_kit_assets::IconName;
use leaf_core::PageId;
use leaf_editor::{MenuPlace, PageAction, PageMenuEntry, PageMenuIcon};

use super::top_bar::local_offset_minutes;
use super::{Shell, now_ms, report};

/// The menu's width, and each row's height: taller than the widget's default for an easier target.
const MENU_WIDTH: f32 = 220.0;
const ROW_HEIGHT: f32 = 32.0;

/// Fills `menu` with `page`'s entries for `place`, each running its action on `shell`, then the
/// footer's lines.
pub(super) fn page_menu(
    menu: PopupMenu,
    shell: &WeakEntity<Shell>,
    page: PageId,
    place: MenuPlace,
    cx: &App,
) -> PopupMenu {
    let now = now_ms();
    let model = shell.upgrade().map(|shell| {
        shell
            .read(cx)
            .editor()
            .page_menu(page, place, now, local_offset_minutes(now))
    });
    let model = match model {
        Some(Ok(model)) => model,
        Some(Err(err)) => {
            report("could not build the page menu", err);
            return menu;
        }
        None => return menu, // the window is closing
    };
    let mut menu = menu.min_w(px(MENU_WIDTH));
    for entry in model.entries {
        if entry.separated {
            menu = menu.separator();
        }
        let shell = shell.clone();
        let action = entry.action;
        menu = menu.item(
            row(&entry)
                .disabled(!entry.enabled)
                .on_click(move |_, window, cx| {
                    let _ = shell.update(cx, |shell, cx| {
                        shell.run_page_action(page, action, place, window, cx);
                    });
                }),
        );
    }
    if !model.footer.is_empty() {
        menu = menu.separator();
        for line in model.footer {
            menu = menu.label(line);
        }
    }
    menu
}

/// One entry: icon and label (red when it takes something away), and its shortcut if it has one.
fn row(entry: &PageMenuEntry) -> PopupMenuItem {
    let (icon, label, danger, shortcut) = (icon_of(entry.icon), entry.label, entry.danger, entry.shortcut);
    PopupMenuItem::element(move |_, cx| {
        let theme = cx.theme();
        let (text, icon_colour) = if danger {
            (theme.danger, theme.danger)
        } else {
            (theme.popover_foreground, theme.muted_foreground)
        };
        h_flex()
            .w_full()
            .h(px(ROW_HEIGHT))
            .cursor_pointer()
            .px_1()
            .gap_2p5()
            .text_color(text)
            .child(Icon::new(icon).small().text_color(icon_colour))
            .child(gpui::div().flex_1().child(label))
            .children(shortcut.map(|keys| {
                gpui::div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(keys)
            }))
            .into_any_element()
    })
}

/// The icon for each of the editor's pictures.
fn icon_of(icon: PageMenuIcon) -> IconName {
    match icon {
        PageMenuIcon::Add => IconName::Plus,
        PageMenuIcon::Star => IconName::Star,
        PageMenuIcon::StarOff => IconName::StarOff,
        PageMenuIcon::Rename => IconName::PencilLine,
        PageMenuIcon::Trash => IconName::Trash,
    }
}

impl Shell {
    /// Runs a page menu's `action` on `page`, opened at `place`.
    pub(super) fn run_page_action(
        &mut self,
        page: PageId,
        action: PageAction,
        place: MenuPlace,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            PageAction::AddInside => self.new_page_inside(page, window, cx),
            PageAction::ToggleFavourite => self.toggle_favourite(page, cx),
            // In the top bar the open page's own title is the place to type; in the sidebar, its row.
            PageAction::Rename if place == MenuPlace::TopBar && self.editor.selected() == Some(page) => {
                self.title.update(cx, |input, cx| {
                    input.focus(window, cx);
                    input.select_all(window, cx);
                });
                cx.notify();
            }
            PageAction::Rename => self.start_rename(page, window, cx),
            PageAction::MoveToTrash => self.trash_from_menu(page, window, cx),
        }
    }

    /// Moves `page` to the trash, opens the nearest page still there if it was open, and offers to
    /// undo with a notice.
    fn trash_from_menu(&mut self, page: PageId, window: &mut Window, cx: &mut Context<Self>) {
        let was_open = self.editor.selected();
        self.end_rename_of(page);
        match self.editor.trash_and_open_next(page, now_ms()) {
            Ok(Some(_)) => self.load_and_show_selected(window, cx),
            Ok(None) => {}
            Err(err) => return report("could not move the page to the trash", err),
        }
        let shell = cx.entity().downgrade();
        let notice = Notification::new()
            .id1::<TrashNotice>(page.to_string())
            .message("Moved to Trash")
            .action(move |_, _, _| {
                let shell = shell.clone();
                crate::theme::button("undo-trash")
                    .debug_selector(|| "undo-trash".into())
                    .label("Undo")
                    .ghost()
                    .small()
                    .on_click(move |_, window, cx| {
                        let _ = shell.update(cx, |shell, cx| shell.undo_trash(page, was_open, window, cx));
                    })
            })
            .autohide(true);
        window.push_notification(notice, cx);
        cx.notify();
    }

    /// Takes `page` back out of the trash and reopens what was open before.
    fn undo_trash(
        &mut self,
        page: PageId,
        was_open: Option<PageId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(err) = self.editor.restore(page) {
            return report("could not restore the page", err);
        }
        window.remove_notification::<TrashNotice>(cx);
        if let Some(open) = was_open {
            self.open(open, window, cx);
        }
        cx.notify();
    }
}

/// Tags the "Moved to Trash" notice, so undoing it can close it.
struct TrashNotice;

#[cfg(test)]
mod tests;
