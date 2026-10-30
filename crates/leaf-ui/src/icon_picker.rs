//! The page icon picker: a search field with Random and Remove, over a grid of emoji in sections. It
//! draws `leaf_editor::EmojiPicker` and tells the shell what was picked; it never changes a page itself.
//! While the search field has the keyboard, the arrow keys move the highlight through the grid and
//! Enter picks it (E17-S04); the field keeps typing, Backspace and the rest.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, ParentElement, Render, ScrollStrategy,
    SharedString, Styled, Subscription, UniformListScrollHandle, Window, div, px, uniform_list,
};
use gpui_component::button::ButtonVariants as _;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::input::{MoveDown, MoveLeft, MoveRight, MoveUp};
use gpui_component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit_assets::IconName;
use leaf_editor::{EmojiChoice, EmojiPicker, EmojiSection, GridMove};

/// Emoji per grid row.
const COLUMNS: usize = 10;

/// Size of an emoji cell, and the height of every grid row (titles too), in pixels.
const CELL: f32 = 36.0;

/// The grid's height before it scrolls, in pixels.
const GRID_HEIGHT: f32 = 324.0;

/// What the owner chose in the picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IconPicked {
    /// This emoji becomes the page's icon.
    Emoji(&'static str),
    /// The page has no icon any more.
    Remove,
}

/// One row of the grid: a section title, or up to [`COLUMNS`] emoji of `section` from position `start`.
#[derive(Debug, Clone)]
enum Row {
    Title(&'static str),
    Emojis {
        section: usize,
        start: usize,
        emojis: Vec<EmojiChoice>,
    },
}

/// The picker's content, shown in a popover under the page icon.
pub struct IconPickerPanel {
    picker: EmojiPicker,
    rows: Vec<Row>,
    input: Entity<InputState>,
    /// Whether the page has an icon, so Remove has something to remove.
    has_icon: bool,
    scroll: UniformListScrollHandle,
    _events: Subscription,
}

impl EventEmitter<IconPicked> for IconPickerPanel {}

impl IconPickerPanel {
    /// A picker for a page that has an icon already (`has_icon`) or not.
    pub fn new(has_icon: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search emoji…"));
        let events = cx.subscribe_in(&input, window, |panel, input, event, _, cx| match event {
            InputEvent::Change => {
                let query = input.read(cx).value().to_string();
                panel.picker.set_query(&query);
                panel.rows = rows(panel.picker.sections());
                panel.scroll.scroll_to_item(0, ScrollStrategy::Top);
                cx.notify();
            }
            // Enter takes the highlighted emoji: the best match, so "rocket ⏎" is enough, or wherever the
            // arrows moved it.
            InputEvent::PressEnter { .. } => {
                if let Some(chosen) = panel.picker.highlighted_choice() {
                    cx.emit(IconPicked::Emoji(chosen.emoji));
                }
            }
            _ => {}
        });
        let picker = EmojiPicker::new();
        Self {
            rows: rows(picker.sections()),
            picker,
            input,
            has_icon,
            scroll: UniformListScrollHandle::new(),
            _events: events,
        }
    }

    /// An arrow key: moves the highlight and scrolls its row into view.
    fn move_highlight(&mut self, step: GridMove, cx: &mut Context<Self>) {
        self.picker.move_highlight(step, COLUMNS);
        if let Some(row) = self.picker.highlighted().and_then(|at| row_of(&self.rows, at)) {
            self.scroll.scroll_to_item(row, ScrollStrategy::Nearest);
        }
        cx.notify();
    }

    fn render_row(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (muted, hover, radius) = (theme.muted_foreground, theme.secondary_hover, theme.radius);
        let highlighted = self.picker.highlighted();
        match self.rows.get(ix) {
            Some(Row::Title(title)) => div()
                .h(px(CELL))
                .px_1()
                .pt_3()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(muted)
                .child(*title)
                .into_any_element(),
            Some(Row::Emojis {
                section,
                start,
                emojis,
            }) => h_flex()
                .h(px(CELL))
                .children(emojis.iter().enumerate().map(|(col, choice)| {
                    let emoji = choice.emoji;
                    let at = (*section, start + col);
                    div()
                        .id(SharedString::from(format!("emoji-{ix}-{col}")))
                        .debug_selector(move || format!("emoji-{emoji}"))
                        .size(px(CELL))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(radius)
                        .text_size(px(22.0))
                        .when(highlighted == Some(at), |cell| cell.bg(hover))
                        .child(emoji)
                        .cursor_pointer()
                        .on_mouse_move(cx.listener(move |panel, _, _, cx| {
                            if panel.picker.highlighted() != Some(at) {
                                panel.picker.highlight(at.0, at.1);
                                cx.notify();
                            }
                        }))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |_, _: &MouseDownEvent, _, cx| {
                                cx.emit(IconPicked::Emoji(emoji));
                            }),
                        )
                }))
                .into_any_element(),
            None => div().into_any_element(),
        }
    }
}

/// The sections as grid rows: each title, then its emoji in rows of [`COLUMNS`].
fn rows(sections: &[EmojiSection]) -> Vec<Row> {
    let mut rows = Vec::new();
    for (index, section) in sections.iter().enumerate() {
        rows.push(Row::Title(section.title));
        rows.extend(
            section
                .emojis
                .chunks(COLUMNS)
                .enumerate()
                .map(|(n, chunk)| Row::Emojis {
                    section: index,
                    start: n * COLUMNS,
                    emojis: chunk.to_vec(),
                }),
        );
    }
    rows
}

/// The grid row holding the emoji at (section, position).
fn row_of(rows: &[Row], (section, at): (usize, usize)) -> Option<usize> {
    rows.iter().position(|row| {
        matches!(row, Row::Emojis { section: s, start, emojis }
            if *s == section && (*start..start + emojis.len()).contains(&at))
    })
}

impl Focusable for IconPickerPanel {
    fn focus_handle(&self, cx: &gpui::App) -> FocusHandle {
        self.input.read(cx).focus_handle(cx)
    }
}

impl Render for IconPickerPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let random = crate::theme::button("random-icon")
            .debug_selector(|| "random-icon".into())
            .label("Random")
            .ghost()
            .small()
            .on_click(cx.listener(|_, _, _, cx| {
                cx.emit(IconPicked::Emoji(EmojiPicker::random(seed()).emoji));
            }));
        let remove = crate::theme::button("remove-icon")
            .debug_selector(|| "remove-icon".into())
            .label("Remove")
            .ghost()
            .small()
            .disabled(!self.has_icon)
            .on_click(cx.listener(|_, _, _, cx| cx.emit(IconPicked::Remove)));
        let body = if self.rows.is_empty() {
            div()
                .h(px(GRID_HEIGHT))
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(muted)
                .child("No emoji match")
                .into_any_element()
        } else {
            uniform_list(
                "emoji-grid",
                self.rows.len(),
                cx.processor(|panel, range: std::ops::Range<usize>, _, cx| {
                    range.map(|ix| panel.render_row(ix, cx)).collect::<Vec<_>>()
                }),
            )
            .track_scroll(&self.scroll)
            .h(px(GRID_HEIGHT))
            .into_any_element()
        };
        v_flex()
            .w(px(CELL * 10.0 + 24.0))
            .gap_2()
            // The search field binds the arrows to its caret; in the picker they move through the grid.
            .capture_action(cx.listener(|panel, _: &MoveUp, _, cx| {
                cx.stop_propagation();
                panel.move_highlight(GridMove::Up, cx);
            }))
            .capture_action(cx.listener(|panel, _: &MoveDown, _, cx| {
                cx.stop_propagation();
                panel.move_highlight(GridMove::Down, cx);
            }))
            .capture_action(cx.listener(|panel, _: &MoveLeft, _, cx| {
                cx.stop_propagation();
                panel.move_highlight(GridMove::Left, cx);
            }))
            .capture_action(cx.listener(|panel, _: &MoveRight, _, cx| {
                cx.stop_propagation();
                panel.move_highlight(GridMove::Right, cx);
            }))
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        div().flex_1().child(
                            Input::new(&self.input)
                                .small()
                                .prefix(Icon::new(IconName::Search).small().text_color(muted)),
                        ),
                    )
                    .child(random)
                    .child(remove),
            )
            .child(body)
    }
}

/// Something that changes between clicks, for the Random button. The UI may read the clock; the
/// picker logic in `leaf-editor` never does.
fn seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_nanos() % u128::from(u64::MAX)).unwrap_or(0)
        })
}

#[cfg(test)]
mod tests {
    use super::{COLUMNS, Row, row_of, rows};
    use leaf_editor::EmojiPicker;

    #[test]
    fn sections_become_a_title_row_then_full_rows_of_emoji() {
        let picker = EmojiPicker::new();
        let grid = rows(picker.sections());
        assert!(matches!(grid[0], Row::Title("Smileys & emotion")));
        assert!(matches!(&grid[1], Row::Emojis { section: 0, start: 0, emojis } if emojis.len() == COLUMNS));
        assert_eq!(row_of(&grid, (0, 0)), Some(1));
        assert_eq!(row_of(&grid, (0, COLUMNS + 3)), Some(2));
        assert_eq!(
            row_of(&grid, (1, 0)),
            grid.iter()
                .position(|row| matches!(row, Row::Emojis { section: 1, .. }))
        );
        let titles = grid.iter().filter(|row| matches!(row, Row::Title(_))).count();
        assert_eq!(titles, picker.sections().len());
        assert!(rows(&[]).is_empty());
    }
}
