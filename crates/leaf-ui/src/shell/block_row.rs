//! One block row of the page: indentation, list marker or checkbox, and the text input (interim
//! editor, E07-S10; the rich text element replaces the input in E07-S02).

use gpui::{
    AnyElement, Context, ElementId, FontWeight, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, ParentElement, SharedString, Styled, Window, div, px,
};
use gpui_component::checkbox::Checkbox;
use gpui_component::input::Input;
use gpui_component::{ActiveTheme as _, h_flex};
use leaf_editor::{BlockRow, BlockStyle};

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

        let state = self.block_input(row.id, window, cx);
        state.update(cx, |input, cx| {
            if input.value() != text {
                input.set_value(text, window, cx);
            }
        });
        // Body rows are 28px: the input's default height is sized for form fields, which spaces blocks too far apart.
        let input = Input::new(&state)
            .appearance(false)
            .focus_bordered(false)
            .px_0()
            .h_7();
        let muted = cx.theme().muted_foreground;
        let input = match &row.style {
            BlockStyle::Heading(level) => {
                let size = theme::HEADING_SIZES[usize::from((*level).clamp(1, 3) - 1)];
                let height = px((size * 1.3).round());
                input
                    .text_size(px(size))
                    .font_weight(FontWeight::SEMIBOLD)
                    .line_height(height)
                    .h(height + px(10.0))
            }
            BlockStyle::Code { .. } => input.font_family(cx.theme().mono_font_family.clone()).text_sm(),
            BlockStyle::Todo { checked: true } => input.text_color(muted),
            _ => input,
        };

        let id = row.id;
        let prefix = Self::marker(row, cx);
        // Inputs can't draw strikethrough, so a done to-do shows as struck-through text until it is
        // clicked for editing.
        let done_and_resting =
            matches!(row.style, BlockStyle::Todo { checked: true }) && self.blocks.focused != Some(id);
        let body = if done_and_resting {
            div()
                .flex_1()
                .h_7()
                .flex()
                .items_center()
                .text_color(muted)
                .line_through()
                .child(state.read(cx).value().to_string())
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |shell, _: &MouseDownEvent, window, cx| {
                        shell.focus_block(id, window, cx);
                    }),
                )
                .into_any_element()
        } else {
            div().flex_1().child(input).into_any_element()
        };
        let line = line.children(prefix).child(body);
        match &row.style {
            BlockStyle::Code { .. } => line.bg(cx.theme().muted).rounded_md().px_2().into_any_element(),
            _ => line.into_any_element(),
        }
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
