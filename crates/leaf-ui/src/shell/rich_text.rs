//! A block's text drawn with its formatting, the selection highlight and the editor's caret
//! (E07-S02, ADR 0038).
//!
//! The text is GPUI's `StyledText`, built from the block's runs: bold, italic, struck through,
//! inline code in the monospace face on a tint, links in the link colour and underlined. The
//! selection is drawn as a background on the selected pieces, so it sits under the glyphs. An overlay
//! painted after the text records the measured layout (for [`PaintedLayout`]) and draws the caret.
//!
//! The mouse places the caret (shift extends the selection), drags select, a double click selects a
//! word and a triple click the block's text. An input method's draft is drawn underlined in place.

mod painted_layout;
mod pieces;
#[cfg(test)]
mod window_tests;

pub(super) use painted_layout::PaintedLayout;
pub(super) use pieces::{Highlight, highlights, with_draft};

use gpui::{
    AnyElement, Context, FontStyle, FontWeight, Hsla, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, ParentElement, Pixels, SharedString, StrikethroughStyle, Styled,
    StyledText, TextRun, UnderlineStyle, Window, canvas, div, fill, px, size,
};
use gpui_component::ActiveTheme as _;
use leaf_editor::BlockRow;
use leaf_model::{Caret, Marks, TextSpan};

use super::Shell;
use pieces::{Piece, byte_of, pieces};

/// How a block's text looks as a whole, before its own formatting.
pub(super) struct Look {
    /// Text size; `None` for body text.
    pub size: Option<Pixels>,
    /// Height of one line.
    pub line_height: Pixels,
    /// Space above and below the text, so a one-line row is as tall as the input it replaces.
    pub pad: Pixels,
    /// Weight of unformatted text (headings are semibold).
    pub weight: FontWeight,
    /// Code blocks: the monospace face throughout.
    pub mono: bool,
    /// A ticked to-do: muted and struck through.
    pub done: bool,
}

impl Shell {
    /// `row`'s text with formatting, `highlight` drawn as the selection and, at `caret` (a character
    /// offset), the caret when it is in its visible half of the blink.
    pub(super) fn render_rich_text(
        &self,
        row: &BlockRow,
        look: &Look,
        highlight: Option<&Highlight>,
        caret: Option<usize>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = row.id;
        let real_spans = row.spans.clone().unwrap_or_else(|| {
            vec![TextSpan {
                text: row.text.clone().unwrap_or_default(),
                marks: Marks::default(),
            }]
        });
        // An input method's draft shows in place of what it will replace, underlined; the selection it
        // covers is hidden meanwhile.
        let draft = self.blocks.draft.as_ref().filter(|d| d.at.block == id);
        let (spans, draft_range, caret, highlight) = match draft {
            Some(draft) => {
                let (spans, range) = with_draft(&real_spans, &draft.at.range, &draft.text);
                let at = range.start + draft.cursor;
                (spans, Some(range), Some(at), None)
            }
            None => (real_spans, None, caret, highlight),
        };
        let text: String = spans.iter().map(|s| s.text.as_str()).collect();
        let len = text.chars().count();
        let selected = match highlight {
            Some(Highlight::Text(range)) => Some(range.clone()),
            Some(Highlight::Whole) => Some(0..len),
            None => None,
        };
        let runs: Vec<TextRun> = pieces(&spans, selected.as_ref(), draft_range.as_ref())
            .iter()
            .map(|piece| Self::run(piece, look, window, cx))
            .collect();
        let styled = StyledText::new(SharedString::from(text.clone())).with_runs(runs);
        let layout = styled.layout().clone();

        let painted = self.blocks.painted.clone();
        let caret_byte = caret
            .filter(|_| self.blocks.caret_on)
            .map(|at| byte_of(&text, at));
        let caret_color = cx.theme().caret;
        let recorded_text = text.clone();
        let overlay = canvas(
            {
                let layout = layout.clone();
                move |_, _, _| {
                    // The text was measured and placed just before this: record it for Up/Down.
                    painted.record(id, &recorded_text, layout.clone());
                    caret_byte
                        .and_then(|byte| layout.position_for_index(byte))
                        .map(|at| (at, layout.line_height()))
                }
            },
            move |_, caret, window, _| {
                if let Some((at, height)) = caret {
                    window.paint_quad(fill(gpui::Bounds::new(at, size(px(2.0), height)), caret_color));
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();

        let block = div()
            .relative()
            .flex_1()
            .min_w_0()
            .py(look.pad)
            .line_height(look.line_height)
            .min_h(look.line_height + look.pad * 2.0)
            .child(styled)
            .child(overlay)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |shell, event: &MouseDownEvent, window, cx| {
                    shell.on_text_mouse_down(id, len, event, window, cx);
                }),
            )
            .on_mouse_move(cx.listener(move |shell, event: &MouseMoveEvent, window, cx| {
                shell.on_text_drag(id, event, window, cx);
            }));
        let block = match look.size {
            Some(size) => block.text_size(size),
            None => block,
        };
        block.into_any_element()
    }

    /// A click in `block`'s text: the caret goes there (shift: the selection extends there); a double
    /// click selects the word, a triple click the block's text. A drag starts from here.
    fn on_text_mouse_down(
        &mut self,
        block: leaf_core::BlockId,
        len: usize,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let at = self
            .blocks
            .painted
            .offset_at(block, event.position)
            .unwrap_or(len);
        let here = Caret { block, at };
        let Some(page) = self.page() else { return };
        let result = match event.click_count {
            2 => page.select_word_at(here).map(|_| ()),
            n if n >= 3 => page.select_block_text(block).map(|_| ()),
            _ if event.modifiers.shift => match page.selection().ok().flatten() {
                Some(leaf_editor::Selection::Text { start, end }) => {
                    let caret = page.caret().ok().flatten();
                    let tail = if caret == Some(start) { end } else { start };
                    page.select_between(tail, here)
                }
                _ => match page.caret().ok().flatten() {
                    Some(from) => page.select_between(from, here),
                    None => page.set_caret(here).map(|_| ()),
                },
            },
            _ => page.set_caret(here).map(|_| ()),
        };
        if let Err(err) = result {
            super::report("could not place the caret", err);
        }
        self.blocks.drag = (event.click_count <= 1 && !event.modifiers.shift).then_some(here);
        self.focus_page(window, cx);
        self.restart_blink();
    }

    /// Dragging over `block`'s text with the button down selects from where the drag started.
    fn on_text_drag(
        &mut self,
        block: leaf_core::BlockId,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.pressed_button != Some(MouseButton::Left) {
            return;
        }
        let Some(from) = self.blocks.drag else { return };
        let Some(at) = self.blocks.painted.offset_at(block, event.position) else {
            return;
        };
        if let Some(page) = self.page() {
            let _ = page.select_between(from, Caret { block, at });
        }
        cx.notify();
    }

    /// The text run for one piece: the block's look, then the piece's own formatting.
    fn run(piece: &Piece, look: &Look, window: &Window, cx: &Context<Self>) -> TextRun {
        let theme = cx.theme();
        let marks = &piece.marks;
        let mut font = window.text_style().font();
        font.weight = if marks.bold { FontWeight::BOLD } else { look.weight };
        if marks.italic {
            font.style = FontStyle::Italic;
        }
        if look.mono || marks.code {
            font.family = theme.mono_font_family.clone();
        }
        let color: Hsla = if marks.link.is_some() {
            theme.link
        } else if look.done {
            theme.muted_foreground
        } else {
            theme.foreground
        };
        let background = if piece.selected {
            // At full strength: the theme's selection colour arrives translucent, which is too faint
            // behind text on the warm page background.
            Some(Hsla {
                a: 1.0,
                ..theme.selection
            })
        } else if marks.code && !look.mono {
            Some(theme.muted)
        } else {
            None
        };
        let underline = if piece.draft {
            Some(UnderlineStyle {
                thickness: px(1.0),
                color: None,
                wavy: false,
            })
        } else {
            marks.link.as_ref().map(|_| UnderlineStyle {
                thickness: px(1.0),
                color: Some(theme.link),
                wavy: false,
            })
        };
        TextRun {
            len: piece.bytes.len(),
            font,
            color,
            background_color: background,
            underline,
            strikethrough: (marks.strike || look.done).then_some(StrikethroughStyle {
                thickness: px(1.0),
                color: None,
            }),
        }
    }
}
