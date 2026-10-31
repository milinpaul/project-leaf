//! Drawing a web bookmark as a card (E15-S04): the title, two lines of description and the site's icon
//! and domain on the left, the preview picture on the right. What it shows is `leaf_editor`'s
//! [`WebBookmarkCard`]; parts it lacks are left out, and before a preview it shows the address and the
//! domain. A click opens the address in the system browser; the ⋮⋮ handle, selection and keys treat it
//! as a whole block, like an image.

use std::path::PathBuf;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, ObjectFit,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, StyledImage as _, div,
    img, px,
};
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit_assets::IconName;
use leaf_core::AssetId;
use leaf_editor::{BlockRow, WebBookmarkCard};
use leaf_model::WebPreview;

use super::Shell;
use super::block_row::indent_of;
use super::rich_text::Highlight;

/// The card's height with a preview picture or a description, and with only a title and the address.
const TALL: f32 = 104.0;
const SHORT: f32 = 64.0;

/// The preview picture's width at the side.
const PICTURE_WIDTH: f32 = 220.0;

impl Shell {
    /// A web bookmark row: the card.
    pub(super) fn render_web_bookmark(
        &mut self,
        row: &BlockRow,
        url: &str,
        preview: &WebPreview,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = row.id;
        let card = WebBookmarkCard::new(url, preview, self.blocks.fetching.contains(&id));
        let picture = card.image.and_then(|asset| self.drawable_path(asset));
        let icon = card.favicon.and_then(|asset| self.drawable_path(asset));
        let selected = self.blocks.highlights.get(&id) == Some(&Highlight::Whole);
        let theme = cx.theme();
        let (border, hover, muted, accent, radius) = (
            theme.border,
            theme.muted,
            theme.muted_foreground,
            theme.primary,
            theme.radius,
        );
        let tall = picture.is_some() || card.description.is_some();
        let address = card.address.clone();
        let words = card_words(id, &card, icon, muted);
        let side = picture.map(|path| {
            div()
                .flex_none()
                .w(px(PICTURE_WIDTH))
                .h_full()
                .border_l_1()
                .border_color(border)
                .overflow_hidden()
                .child(img(path).size_full().object_fit(ObjectFit::Cover))
        });
        h_flex()
            .w_full()
            .pl(indent_of(row))
            .py_1()
            .child(
                h_flex()
                    .id(SharedString::from(format!("bookmark-{id}")))
                    .debug_selector(move || format!("bookmark-{id}"))
                    .flex_1()
                    .min_w_0()
                    .h(px(if tall { TALL } else { SHORT }))
                    .overflow_hidden()
                    .rounded(radius)
                    .border_1()
                    .border_color(if selected { accent } else { border })
                    .when(selected, |card| card.bg(accent.opacity(0.08)))
                    .when(card.fetching && !selected, |card| card.opacity(0.75))
                    .cursor_pointer()
                    .hover(|style| style.bg(hover))
                    .tooltip(move |window, cx| {
                        gpui_component::tooltip::Tooltip::new(SharedString::from(address.clone()))
                            .build(window, cx)
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        move |_: &MouseDownEvent, _, cx: &mut gpui::App| {
                            // Not also a click that places the caret or starts a selection.
                            cx.stop_propagation();
                            open_address(&card.address, cx);
                        },
                    )
                    .child(words)
                    .children(side),
            )
            .into_any_element()
    }

    /// A stored picture's file, if it can be drawn (checked once per picture, like an image block's).
    fn drawable_path(&mut self, asset: AssetId) -> Option<PathBuf> {
        let storage = self.storage.as_ref()?;
        self.blocks
            .drawable
            .entry(asset)
            .or_insert_with(|| storage.drawable_asset(asset))
            .as_ref()
            .ok()
            .cloned()
    }
}

/// The card's words: the title, the description and the icon and domain line (with "Fetching preview…"
/// while a fetch is under way).
fn card_words(
    id: leaf_core::BlockId,
    card: &WebBookmarkCard,
    icon: Option<PathBuf>,
    muted: gpui::Hsla,
) -> gpui::Div {
    let favicon = match icon {
        Some(path) => img(path).size(px(16.0)).rounded_sm().into_any_element(),
        None => Icon::new(IconName::Globe)
            .small()
            .text_color(muted)
            .into_any_element(),
    };
    v_flex()
        .flex_1()
        .min_w_0()
        .justify_center()
        .gap_1()
        .px_3()
        .py_2()
        .child(
            div()
                .debug_selector(move || format!("bookmark-title-{id}"))
                .text_sm()
                .font_weight(gpui::FontWeight::MEDIUM)
                .truncate()
                .child(card.title.clone()),
        )
        .children(
            card.description
                .clone()
                .map(|text| div().text_xs().text_color(muted).line_clamp(2).child(text)),
        )
        .child(
            h_flex()
                .gap_1p5()
                .text_xs()
                .min_w_0()
                .child(favicon)
                .child(div().truncate().child(card.domain.clone()))
                .when(card.fetching, |line| {
                    line.child(
                        div()
                            .debug_selector(move || format!("bookmark-fetching-{id}"))
                            .text_color(muted)
                            .child("· Fetching preview…"),
                    )
                }),
        )
}

/// Opens a bookmark's address in the system browser. The address was checked when the bookmark was made
/// (`http` or `https` only, ADR-083), and is checked again, so nothing else is ever handed to the system.
fn open_address(address: &str, cx: &mut gpui::App) {
    if leaf_model::check_web_address(address).is_ok() {
        cx.open_url(address);
    }
}

#[cfg(test)]
mod tests;
