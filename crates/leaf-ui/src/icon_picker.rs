//! The page icon picker: a search field with Random and Remove, over a grid of emoji in sections. It
//! draws `leaf_editor::EmojiPicker` and tells the shell what was picked; it never changes a page itself.

use gpui::{
    AnyElement, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, ParentElement, Render, ScrollStrategy,
    SharedString, Styled, Subscription, UniformListScrollHandle, Window, div, px, uniform_list,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit_assets::IconName;
use leaf_editor::{EmojiChoice, EmojiPicker, EmojiSection};

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

/// One row of the grid: a section title, or up to [`COLUMNS`] emoji.
#[derive(Debug, Clone)]
enum Row {
    Title(&'static str),
    Emojis(Vec<EmojiChoice>),
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
            // Enter takes the best match, so "rocket ⏎" is enough.
            InputEvent::PressEnter { .. } => {
                if let Some(first) = panel.picker.first() {
                    cx.emit(IconPicked::Emoji(first.emoji));
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

    fn render_row(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (muted, hover, radius) = (theme.muted_foreground, theme.secondary_hover, theme.radius);
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
            Some(Row::Emojis(emojis)) => h_flex()
                .h(px(CELL))
                .children(emojis.iter().enumerate().map(|(col, choice)| {
                    let emoji = choice.emoji;
                    div()
                        .id(SharedString::from(format!("emoji-{ix}-{col}")))
                        .debug_selector(move || format!("emoji-{emoji}"))
                        .size(px(CELL))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(radius)
                        .text_size(px(22.0))
                        .hover(|style| style.bg(hover))
                        .child(emoji)
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
    for section in sections {
        rows.push(Row::Title(section.title));
        rows.extend(
            section
                .emojis
                .chunks(COLUMNS)
                .map(|chunk| Row::Emojis(chunk.to_vec())),
        );
    }
    rows
}

impl Focusable for IconPickerPanel {
    fn focus_handle(&self, cx: &gpui::App) -> FocusHandle {
        self.input.read(cx).focus_handle(cx)
    }
}

impl Render for IconPickerPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let random = Button::new("random-icon")
            .debug_selector(|| "random-icon".into())
            .label("Random")
            .ghost()
            .small()
            .on_click(cx.listener(|_, _, _, cx| {
                cx.emit(IconPicked::Emoji(EmojiPicker::random(seed()).emoji));
            }));
        let remove = Button::new("remove-icon")
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
    use super::{COLUMNS, Row, rows};
    use leaf_editor::EmojiPicker;

    #[test]
    fn sections_become_a_title_row_then_full_rows_of_emoji() {
        let picker = EmojiPicker::new();
        let grid = rows(picker.sections());
        assert!(matches!(grid[0], Row::Title("Smileys & emotion")));
        assert!(matches!(&grid[1], Row::Emojis(row) if row.len() == COLUMNS));
        let titles = grid.iter().filter(|row| matches!(row, Row::Title(_))).count();
        assert_eq!(titles, picker.sections().len());
        assert!(rows(&[]).is_empty());
    }
}
