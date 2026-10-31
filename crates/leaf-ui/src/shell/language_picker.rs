//! The code block's language label and picker (E07-S09, ADR-054). The label shows at the top right
//! of a code block while the pointer is over it; a click opens a list to filter by typing. Enter picks
//! the best match, a click picks any, and Escape or a click elsewhere closes it. The list and the
//! change are `leaf_editor::code_block`'s; this module draws and forwards.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, InteractiveElement as _, IntoElement,
    KeyBinding, MouseButton, MouseDownEvent, ParentElement as _, Pixels, Point, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window, actions, anchored, deferred, div,
    point, px,
};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{ActiveTheme as _, Sizable as _, v_flex};
use leaf_core::BlockId;
use leaf_editor::{CodeLanguage, find_languages, language_label};

use super::{Shell, block_menu::ROW_GROUP, report};

/// The picker's key context, active while its field has the keyboard.
const CONTEXT: &str = "LeafLanguagePicker";

actions!(
    leaf_language_picker,
    [
        /// Close the picker without changing the language.
        CancelLanguage
    ]
);

/// Binds Escape in the picker. Called once at startup.
pub(super) fn bind(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", CancelLanguage, Some(CONTEXT))]);
}

/// What was chosen: a language id, or `None` to close without changing anything.
pub(super) struct LanguageChosen(Option<&'static str>);

/// A search field over the languages, and the matches.
pub(super) struct LanguagePicker {
    input: Entity<InputState>,
    found: Vec<CodeLanguage>,
    /// The block's language now, ticked in the list.
    current: String,
    _events: Subscription,
}

impl EventEmitter<LanguageChosen> for LanguagePicker {}

impl LanguagePicker {
    fn new(current: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search languages…"));
        let events = cx.subscribe_in(&input, window, |picker, input, event, _, cx| match event {
            InputEvent::Change => {
                picker.found = listed(&input.read(cx).value(), &picker.current);
                cx.notify();
            }
            InputEvent::PressEnter { .. } => {
                if let Some(first) = picker.found.first() {
                    cx.emit(LanguageChosen(Some(first.id)));
                }
            }
            _ => {}
        });
        Self {
            input,
            found: listed("", &current),
            current,
            _events: events,
        }
    }
}

impl Render for LanguagePicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (accent, muted, radius) = (theme.accent, theme.muted_foreground, theme.radius);
        let current = self.current.clone();
        v_flex()
            .id("language-picker")
            .debug_selector(|| "language-picker".into())
            .key_context(CONTEXT)
            .on_action(cx.listener(|_, _: &CancelLanguage, _, cx| cx.emit(LanguageChosen(None))))
            .w(px(220.0))
            .p_1()
            .gap_1()
            .bg(theme.popover)
            .text_color(theme.popover_foreground)
            .border_1()
            .border_color(theme.border)
            .rounded(radius)
            .shadow_lg()
            .text_sm()
            .child(Input::new(&self.input).small())
            .child(
                v_flex()
                    .id("language-list")
                    .max_h(px(260.0))
                    .overflow_y_scroll()
                    .when(self.found.is_empty(), |list| {
                        list.child(div().px_2().py_1().text_color(muted).child("No match"))
                    })
                    .children(self.found.iter().enumerate().map(|(ix, language)| {
                        let id = language.id;
                        div()
                            .id(("language", ix))
                            .debug_selector(move || {
                                format!("language-{}", if id.is_empty() { "plain" } else { id })
                            })
                            .px_2()
                            .py_1()
                            .rounded(radius)
                            .cursor_pointer()
                            .when(ix == 0, |row| row.bg(accent))
                            .hover(|row| row.bg(accent))
                            .flex()
                            .justify_between()
                            .child(language.label)
                            .when(language.id.eq_ignore_ascii_case(&current), |row| {
                                row.child(div().text_color(muted).child("✓"))
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |_, _: &MouseDownEvent, _, cx| {
                                    cx.stop_propagation();
                                    cx.emit(LanguageChosen(Some(id)));
                                }),
                            )
                    })),
            )
    }
}

/// The languages to list for `query`. Before anything is typed, the block's `current` language comes
/// first, so it is the highlighted one Enter keeps.
fn listed(query: &str, current: &str) -> Vec<CodeLanguage> {
    let mut found = find_languages(query);
    if query.trim().is_empty()
        && let Some(at) = found.iter().position(|l| l.id.eq_ignore_ascii_case(current))
    {
        let language = found.remove(at);
        found.insert(0, language);
    }
    found
}

/// The open picker: for which block, and where.
pub(super) struct OpenLanguage {
    view: Entity<LanguagePicker>,
    block: BlockId,
    at: Point<Pixels>,
    _choice: Subscription,
}

impl Shell {
    /// Opens the picker for code block `block` with its top-left corner at `at`.
    pub fn open_language_picker(
        &mut self,
        block: BlockId,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(Ok(leaf_model::BlockKind::Code { language: current })) =
            self.page().map(|page| page.doc().kind(block))
        else {
            return;
        };
        let view = cx.new(|cx| LanguagePicker::new(current, window, cx));
        let choice = cx.subscribe_in(&view, window, |shell, _, chosen: &LanguageChosen, window, cx| {
            shell.language_chosen(chosen.0, window, cx);
        });
        view.update(cx, |picker, cx| {
            picker.input.update(cx, |input, cx| input.focus(window, cx));
        });
        self.blocks.language = Some(OpenLanguage {
            view,
            block,
            at,
            _choice: choice,
        });
        cx.notify();
    }

    /// Opens the picker for code block `block` under its label, as drawn in the last frame. Used by
    /// the screenshot tool.
    pub fn open_language_picker_beside(
        &mut self,
        block: BlockId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let at = self.blocks.drawn.bounds_of(block).map_or_else(
            || point(px(0.0), px(0.0)),
            |b| point(b.right() - px(228.0), b.top() + px(28.0)),
        );
        self.open_language_picker(block, at, window, cx);
    }

    /// Whether the language picker is open.
    #[must_use]
    pub fn language_picker_open(&self) -> bool {
        self.blocks.language.is_some()
    }

    fn language_chosen(&mut self, language: Option<&str>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = self.blocks.language.take() else {
            return;
        };
        if let Some(language) = language
            && let Some(Err(err)) = self
                .page()
                .map(|page| page.set_code_language(open.block, language))
        {
            report("could not set the code language", err);
        }
        self.focus_page(window, cx);
    }

    /// The label at a code block's top right: its language, opening the picker when clicked. Shown
    /// while the pointer is over the row, or while this block's picker is open.
    pub(super) fn render_language_label(
        &self,
        block: BlockId,
        language: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let open = self.blocks.language.as_ref().is_some_and(|o| o.block == block);
        div()
            .id(SharedString::from(format!("code-language-{block}")))
            .debug_selector(move || format!("code-language-{block}"))
            .absolute()
            .top_1()
            .right_1()
            .px_2()
            .rounded_sm()
            .text_xs()
            .bg(theme.muted)
            .text_color(theme.muted_foreground)
            .cursor_pointer()
            .opacity(if open { 1.0 } else { 0.0 })
            .group_hover(ROW_GROUP, |s| s.opacity(1.0))
            .hover(|s| s.text_color(theme.foreground))
            .child(language_label(language))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |shell, event: &MouseDownEvent, window, cx| {
                    // Not also a click that places the caret in the code.
                    cx.stop_propagation();
                    let at = point(event.position.x - px(160.0), event.position.y + px(14.0));
                    shell.open_language_picker(block, at, window, cx);
                }),
            )
            .into_any_element()
    }

    /// The picker, floating under the label.
    pub(super) fn render_language_picker(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let open = self.blocks.language.as_ref()?;
        let view = open.view.clone();
        Some(
            deferred(
                anchored()
                    .position(open.at)
                    .snap_to_window_with_margin(px(8.0))
                    .child(
                        div()
                            .on_mouse_down_out(cx.listener(|shell, _: &MouseDownEvent, window, cx| {
                                shell.language_chosen(None, window, cx);
                            }))
                            .child(view),
                    ),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests;
