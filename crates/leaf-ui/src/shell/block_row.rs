//! One block row of the page: indentation, list marker or checkbox, then the text, drawn with its
//! formatting, selection and caret (E07-S02, E07-S03). An image row is a picture frame with its
//! caption below as editable text (E05-S07); the picture itself is drawn by `image_row` (E07-S08).

use gpui::{
    AnyElement, Context, ElementId, FontWeight, InteractiveElement as _, IntoElement, ParentElement, Pixels,
    SharedString, Styled, Window, div, px,
};
use gpui_component::checkbox::Checkbox;
use gpui_component::{ActiveTheme as _, h_flex, v_flex};
use leaf_core::AssetId;
use leaf_editor::{BlockRow, BlockStyle};

use super::rich_text::Look;
use super::{Shell, markers};
use crate::theme;

/// Indent per nesting level; also how far right a drag must go to nest one level deeper.
pub(super) const INDENT_PX: f32 = 24.0;

impl Shell {
    pub(super) fn render_row(
        &mut self,
        row: &BlockRow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let indent = indent_of(row);
        let space_above = space_above(&row.style);
        let line = h_flex()
            .w_full()
            .items_center()
            .gap_2()
            .pl(indent)
            .pt(space_above);
        if let BlockStyle::Image { asset, width, .. } = &row.style {
            return self.render_image_row(row, *asset, *width, window, cx);
        }
        if let BlockStyle::WebBookmark { url, preview } = &row.style {
            return self.render_web_bookmark(row, url, preview, cx);
        }
        if let BlockStyle::Table {
            header_row,
            rows,
            columns,
        } = row.style
        {
            return self.render_table(row, header_row, rows, columns, window, cx);
        }
        if row.text.is_none() {
            let placeholder = match &row.style {
                BlockStyle::Divider => {
                    return line
                        .child(div().h(px(1.0)).w_full().bg(cx.theme().border))
                        .into_any_element();
                }
                BlockStyle::Unknown { type_name } => {
                    format!("[{type_name}: made by a newer Leaf, read-only]")
                }
                _ => String::new(),
            };
            return line
                .child(div().text_color(cx.theme().muted_foreground).child(placeholder))
                .into_any_element();
        }

        let id = row.id;
        let prefix = Self::marker(row, cx);
        let look = look_of(&row.style);
        let highlight = self.blocks.highlights.get(&id).cloned();
        let caret = self.blocks.caret.filter(|c| c.block == id).map(|c| c.at);
        let body = self.render_rich_text(row, &look, highlight.as_ref(), caret, window, cx);
        if let BlockStyle::Callout { tone } = &row.style {
            return Self::render_callout(row, tone, body, cx);
        }
        let line = line.children(prefix).child(body);
        match &row.style {
            // An action item's date chip and priority flag follow its text (E19-S03).
            BlockStyle::Task {
                done,
                due,
                time,
                priority,
            } => {
                let chips = super::task_row::TaskChips {
                    done: *done,
                    due: *due,
                    time: *time,
                    priority: *priority,
                };
                line.child(self.render_task_chips(id, chips, cx))
                    .into_any_element()
            }
            BlockStyle::Code { language } => {
                let label = self.render_language_label(id, language, cx);
                line.relative()
                    .bg(cx.theme().muted)
                    .rounded_md()
                    .px_2()
                    .child(label)
                    .into_any_element()
            }
            _ => line.into_any_element(),
        }
    }

    /// An image: the picture, then the caption, which takes the caret like any text.
    fn render_image_row(
        &mut self,
        row: &BlockRow,
        asset: AssetId,
        width: Option<f64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = row.id;
        let muted = cx.theme().muted_foreground;
        let indent = indent_of(row);
        let picture = self.render_picture(id, asset, width, f32::from(indent), window, cx);
        let caption_look = Look {
            size: Some(px(14.0)),
            line_height: px(22.0),
            ..look_of(&row.style)
        };
        let highlight = self.blocks.highlights.get(&id).cloned();
        let caret = self.blocks.caret.filter(|c| c.block == id).map(|c| c.at);
        let caption = self.render_rich_text(row, &caption_look, highlight.as_ref(), caret, window, cx);
        v_flex()
            .w_full()
            .gap_1()
            .pl(indent_of(row))
            .child(picture)
            .child(div().w_full().text_color(muted).child(caption))
            .into_any_element()
    }

    /// What sits before a row's text: its list marker, checkbox or quote bar.
    fn marker(row: &BlockRow, cx: &mut Context<Self>) -> Option<AnyElement> {
        let id = row.id;
        let shell = cx.entity().downgrade();
        match &row.style {
            BlockStyle::Bulleted => Some(div().w_4().child(markers::bullet(row.depth)).into_any_element()),
            BlockStyle::Numbered(n) => Some(
                div()
                    .min_w_4()
                    .child(markers::number(*n, row.depth))
                    .into_any_element(),
            ),
            BlockStyle::Task { done, .. } => Some(Self::render_task_check(id, *done, cx)),
            BlockStyle::Todo { checked } => Some(
                Checkbox::new(ElementId::from(SharedString::from(format!("todo-{id}"))))
                    .checked(*checked)
                    .cursor_pointer()
                    .on_click(move |_, _, cx| {
                        let _ = shell.update(cx, |shell, cx| shell.toggle_todo(id, cx));
                    })
                    .into_any_element(),
            ),
            // As tall as the quote's text, which can have several lines (E17-S13).
            BlockStyle::Quote => Some(
                div()
                    .debug_selector(move || format!("quote-bar-{id}"))
                    .w(px(3.0))
                    .min_h_6()
                    .self_stretch()
                    .bg(cx.theme().border)
                    .into_any_element(),
            ),
            _ => None,
        }
    }
}

/// How far a row's content is indented.
pub(super) fn indent_of(row: &BlockRow) -> Pixels {
    px(INDENT_PX * f32::from(u16::try_from(row.depth).unwrap_or(u16::MAX)))
}

/// Headings get room above them, so a page reads as sections rather than a wall of lines.
fn space_above(style: &BlockStyle) -> Pixels {
    match style {
        BlockStyle::Heading(1) => px(18.0),
        BlockStyle::Heading(_) => px(12.0),
        _ => px(0.0),
    }
}

/// Where a row's ⋮⋮ handle goes: the left edge of its content, and how far down to sit level with
/// the first line of text.
pub(super) fn handle_place(row: &BlockRow) -> (Pixels, Pixels) {
    let look = look_of(&row.style);
    let first_line = look.line_height + look.pad * 2.0;
    // A callout's text starts below its box's padding.
    let inset = if matches!(row.style, BlockStyle::Callout { .. }) {
        px(super::callout::GAP_Y + super::callout::PAD_Y)
    } else {
        px(0.0)
    };
    (
        indent_of(row),
        space_above(&row.style) + inset + (first_line - px(20.0)) / 2.0,
    )
}

/// How a block kind's text looks as a whole. One-line rows come out 28 px tall, as the inputs were.
fn look_of(style: &BlockStyle) -> Look {
    let body = Look {
        size: None,
        line_height: px(24.0),
        pad: px(2.0),
        weight: FontWeight::NORMAL,
        mono: false,
        done: false,
    };
    match style {
        BlockStyle::Heading(level) => {
            let size = theme::HEADING_SIZES[usize::from((*level).clamp(1, 3) - 1)];
            Look {
                size: Some(px(size)),
                line_height: px((size * 1.3).round()),
                pad: px(5.0),
                weight: FontWeight::SEMIBOLD,
                ..body
            }
        }
        BlockStyle::Code { .. } => Look {
            size: Some(px(14.0)),
            line_height: px(22.0),
            pad: px(3.0),
            mono: true,
            ..body
        },
        BlockStyle::Todo { checked: true } | BlockStyle::Task { done: true, .. } => {
            Look { done: true, ..body }
        }
        _ => body,
    }
}

#[cfg(test)]
mod tests;
