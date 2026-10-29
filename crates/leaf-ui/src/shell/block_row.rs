//! One block row of the page: indentation, list marker or checkbox, then the text, drawn with its
//! formatting (E07-S02). The block being typed in shows a text input instead, until typing moves into
//! the text view (E07-S03).

use gpui::{
    AnyElement, Context, ElementId, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window,
    div, px,
};
use gpui_component::checkbox::Checkbox;
use gpui_component::input::Input;
use gpui_component::{ActiveTheme as _, h_flex};
use leaf_editor::{BlockRow, BlockStyle};

use super::rich_text::Look;
use super::{Shell, markers};
use crate::theme;

/// Indent per nesting level.
const INDENT_PX: f32 = 24.0;

impl Shell {
    pub(super) fn render_row(
        &mut self,
        row: &BlockRow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let indent = px(INDENT_PX * f32::from(u16::try_from(row.depth).unwrap_or(u16::MAX)));
        // Headings get room above them, so a page reads as sections rather than a wall of lines.
        let space_above = match row.style {
            BlockStyle::Heading(1) => px(18.0),
            BlockStyle::Heading(_) => px(12.0),
            _ => px(0.0),
        };
        let line = h_flex()
            .w_full()
            .items_center()
            .gap_2()
            .pl(indent)
            .pt(space_above);
        let Some(text) = row.text.clone() else {
            let placeholder = match &row.style {
                BlockStyle::Divider => {
                    return line
                        .child(div().h(px(1.0)).w_full().bg(cx.theme().border))
                        .into_any_element();
                }
                BlockStyle::Image { alt } => format!("[image] {alt}"),
                BlockStyle::Unknown { type_name } => {
                    format!("[{type_name}: made by a newer Leaf, read-only]")
                }
                _ => String::new(),
            };
            return line
                .child(div().text_color(cx.theme().muted_foreground).child(placeholder))
                .into_any_element();
        };

        let id = row.id;
        let prefix = Self::marker(row, cx);
        let look = look_of(&row.style);
        let body = if self.blocks.focused == Some(id) {
            self.render_row_input(row, &text, &look, window, cx)
        } else {
            let highlight = self.blocks.highlights.get(&id).cloned();
            let caret = self.blocks.caret.filter(|c| c.block == id).map(|c| c.at);
            self.render_rich_text(row, &look, highlight.as_ref(), caret, window, cx)
        };
        let line = line.children(prefix).child(body);
        match &row.style {
            BlockStyle::Code { .. } => line.bg(cx.theme().muted).rounded_md().px_2().into_any_element(),
            _ => line.into_any_element(),
        }
    }

    /// The block being typed in: a text input styled like the text view it stands in for (interim,
    /// until typing moves into the text view in E07-S03).
    fn render_row_input(
        &mut self,
        row: &BlockRow,
        text: &str,
        look: &Look,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state = self.block_input(row.id, window, cx);
        state.update(cx, |input, cx| {
            if input.value() != text {
                input.set_value(text.to_owned(), window, cx);
            }
        });
        let input = Input::new(&state)
            .appearance(false)
            .focus_bordered(false)
            .px_0()
            .line_height(look.line_height)
            .h(look.line_height + look.pad * 2.0)
            .font_weight(look.weight);
        let input = match look.size {
            Some(size) => input.text_size(size),
            None => input,
        };
        let input = if look.mono {
            input.font_family(cx.theme().mono_font_family.clone())
        } else {
            input
        };
        let input = if look.done {
            input.text_color(cx.theme().muted_foreground)
        } else {
            input
        };
        div().flex_1().min_w_0().child(input).into_any_element()
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
            BlockStyle::Todo { checked } => Some(
                Checkbox::new(ElementId::from(SharedString::from(format!("todo-{id}"))))
                    .checked(*checked)
                    .on_click(move |_, _, cx| {
                        let _ = shell.update(cx, |shell, cx| shell.toggle_todo(id, cx));
                    })
                    .into_any_element(),
            ),
            BlockStyle::Quote => Some(div().w(px(3.0)).h_6().bg(cx.theme().border).into_any_element()),
            _ => None,
        }
    }
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
        BlockStyle::Todo { checked: true } => Look { done: true, ..body },
        _ => body,
    }
}
