//! Every character boundary of a painted row with where it is drawn, worked out in one pass over
//! GPUI's glyphs (E17-S08).
//!
//! GPUI answers "where is this byte drawn?" one byte at a time, and each answer walks the line from
//! its start, so asking about every boundary of a long paragraph cost time proportional to its length
//! squared: a 38,000-character paste froze clicks and selection for over a minute. This gives the same
//! answers as [`gpui::TextLayout::position_for_index`] for all boundaries at once.

use gpui::{Pixels, Point, TextLayout, point};

/// A row's character boundaries in order, each with its drawn position, and the top of each visual line.
pub(super) struct Boundaries {
    /// `(character offset, position)` for each boundary GPUI can place, in order.
    pub(super) points: Vec<(usize, Point<Pixels>)>,
    /// The top of each visual line, in order.
    pub(super) tops: Vec<Pixels>,
}

impl Boundaries {
    /// Works out every boundary of `text` as laid out in `layout`.
    pub(super) fn of(text: &str, layout: &TextLayout) -> Self {
        let origin = layout.bounds().origin;
        let line_height = layout.line_height();
        let mut points = Vec::with_capacity(text.len() + 1);
        let mut boundaries = text
            .char_indices()
            .map(|(byte, _)| byte)
            .chain([text.len()])
            .enumerate()
            .peekable();
        let (mut line_y, mut line_start) = (origin.y, 0);
        for line in layout.line_layouts() {
            let unwrapped = line.unwrapped_layout.as_ref();
            // x of the first glyph at or after a byte, as `LineLayout::x_for_index` finds it: the first
            // glyph whose index (or any earlier glyph's) reaches the byte, so a running maximum works
            // even where glyphs aren't in text order.
            let mut glyphs = Vec::new();
            let mut reach = 0;
            for glyph in unwrapped.runs.iter().flat_map(|run| &run.glyphs) {
                reach = reach.max(glyph.index);
                glyphs.push((reach, glyph.position.x));
            }
            let x_at = |byte: usize, from: &mut usize| {
                while *from < glyphs.len() && glyphs[*from].0 < byte {
                    *from += 1;
                }
                glyphs.get(*from).map_or(unwrapped.width, |glyph| glyph.1)
            };
            // Where each visual line ends: its wrap glyph's index, then the end of the line.
            let ends: Vec<usize> = line
                .wrap_boundaries()
                .iter()
                .map(|wrap| unwrapped.runs[wrap.run_ix].glyphs[wrap.glyph_ix].index)
                .chain([line.len()])
                .collect();
            let line_end = line_start + line.len();
            let mut visual = 0;
            let (mut sweep, mut start_sweep) = (0, 0);
            let mut start_x = x_at(0, &mut start_sweep);
            while let Some(&(offset, byte)) = boundaries.peek() {
                if byte > line_end {
                    break;
                }
                boundaries.next();
                let within = byte - line_start;
                // A boundary exactly at a wrap belongs to the line it ends, as GPUI places it.
                while visual + 1 < ends.len() && within > ends[visual] {
                    start_x = x_at(ends[visual], &mut start_sweep);
                    visual += 1;
                }
                let x = x_at(within, &mut sweep) - start_x;
                let y = line_y + line_height * visual_count(visual);
                points.push((offset, point(origin.x + x, y)));
            }
            line_y += line_height * visual_count(ends.len());
            line_start = line_end + 1;
        }
        let mut tops: Vec<Pixels> = points.iter().map(|(_, at)| at.y).collect();
        tops.sort_by(|a, b| f32::from(*a).total_cmp(&f32::from(*b)));
        tops.dedup_by(|a, b| (*a - *b).abs() < gpui::px(0.5));
        Self { points, tops }
    }

    /// Where the boundary at character `offset` is drawn, if it was.
    pub(super) fn position(&self, offset: usize) -> Option<Point<Pixels>> {
        self.points
            .binary_search_by_key(&offset, |(at, _)| *at)
            .ok()
            .map(|ix| self.points[ix].1)
    }
}

/// A count of visual lines as a multiplier for pixel sizes.
fn visual_count(lines: usize) -> f32 {
    f32::from(u16::try_from(lines).unwrap_or(u16::MAX))
}
