//! The shell's side of the Trash: putting pages back, deleting them for good (and their documents on
//! disk), and the banner over a page that is in the trash. What each action means lives in
//! `WorkspaceEditor`; the Trash dialog itself is `crate::trash_dialog`.

use gpui::{
    AnyElement, ClickEvent, Context, InteractiveElement as _, IntoElement, ParentElement, Styled, Window, div,
};
use gpui_component::button::ButtonVariants as _;
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex};
use gpui_kit_assets::IconName;
use leaf_core::PageId;

use super::{Shell, report};

impl Shell {
    /// Takes `page` out of the trash. If it is the open page, the banner goes.
    pub fn restore_page(&mut self, page: PageId, cx: &mut Context<Self>) {
        if let Err(err) = self.editor.restore(page) {
            report("could not restore the page", err);
        }
        cx.notify();
    }

    /// Deletes `page` (in the trash) and everything inside it for good, on disk too.
    pub fn delete_page_forever(&mut self, page: PageId, cx: &mut Context<Self>) {
        match self.editor.delete_forever(page) {
            Ok(deleted) => self.remove_from_disk(&deleted),
            Err(err) => report("could not delete the page", err),
        }
        cx.notify();
    }

    /// Deletes everything in the trash for good, on disk too.
    pub fn empty_trash(&mut self, cx: &mut Context<Self>) {
        match self.editor.empty_trash() {
            Ok(deleted) => self.remove_from_disk(&deleted),
            Err(err) => report("could not empty the trash", err),
        }
        cx.notify();
    }

    /// Opens the Trash dialog.
    pub fn open_trash(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        crate::trash_dialog::open(cx.entity().downgrade(), &self.editor, window, cx);
    }

    /// A strip over a page that is in the trash, offering to put it back or delete it for good.
    pub(super) fn render_trash_banner(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let page = self.editor.selected()?;
        if !self.editor.selected_in_trash().unwrap_or(false) {
            return None;
        }
        let danger = cx.theme().danger;
        let restore = crate::theme::button("banner-restore")
            .debug_selector(|| "banner-restore".into())
            .label("Restore page")
            .outline()
            .small()
            .on_click(cx.listener(move |shell, _: &ClickEvent, _, cx| shell.restore_page(page, cx)));
        let delete = crate::theme::button("banner-delete")
            .debug_selector(|| "banner-delete".into())
            .label("Delete forever")
            .danger()
            .small()
            .on_click(cx.listener(move |shell, _: &ClickEvent, window, cx| {
                let title = shell.editor.selected_title().ok().flatten().unwrap_or_default();
                let title = if title.is_empty() {
                    leaf_editor::UNTITLED.to_owned()
                } else {
                    title
                };
                let handle = cx.entity().downgrade();
                crate::trash_dialog::confirm_delete(handle, page, &title, None, window, cx);
            }));
        Some(
            h_flex()
                .w_full()
                .px_4()
                .py_2()
                .gap_3()
                .rounded(cx.theme().radius)
                .bg(danger.opacity(0.10))
                .text_sm()
                .text_color(danger)
                .child(Icon::new(IconName::Trash).small())
                .child(div().flex_1().child("This page is in the trash."))
                .child(restore)
                .child(delete)
                .into_any_element(),
        )
    }
}
