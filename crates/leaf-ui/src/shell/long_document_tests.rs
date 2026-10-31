//! A 3,000-word document typed through the window (E08-S03, Phase 0 acceptance): headings, paragraphs,
//! nested bulleted and numbered lists, to-dos, quotes and code blocks, all made the way a person makes
//! them (Markdown shortcuts, Enter, Tab, Shift+Tab, Backspace to fix typos, Cmd+Enter out of code),
//! then edited in the middle. The page must hold exactly what was typed, block by block, and read
//! back the same from its saved form.
//!
//! This stands in for, but doesn't replace, writing such a document by hand in the app. Each of its
//! thousands of inputs updates the window as a keystroke in the app does (a few milliseconds each),
//! so it takes 20–40 seconds; it is left out of `just test` and run by `just acceptance`.

use gpui::{TestAppContext, VisualTestContext};
use leaf_editor::{BlockStyle, PageView};
use leaf_model::{Caret, PageDoc};

use crate::shell::Shell;

/// How many words to type: the acceptance checklist's "3,000-word document".
const WORDS: usize = 3_000;

/// What a block should be: its depth, a short name for its kind, and its text.
type Row = (usize, &'static str, String);

fn kind(style: &BlockStyle) -> &'static str {
    match style {
        BlockStyle::Paragraph => "p",
        BlockStyle::Heading(_) => "h",
        BlockStyle::Bulleted => "bullet",
        BlockStyle::Numbered(_) => "number",
        BlockStyle::Todo { .. } => "todo",
        BlockStyle::Quote => "quote",
        BlockStyle::Code { .. } => "code",
        _ => "other",
    }
}

fn rows_of(view: &PageView) -> Vec<Row> {
    view.rows()
        .iter()
        .map(|row| (row.depth, kind(&row.style), row.text.clone().unwrap_or_default()))
        .collect()
}

/// Words from a fixed list, in an order that repeats only after a long while.
struct Words(u64);

impl Words {
    const LIST: &'static [&'static str] = &[
        "garden",
        "river",
        "notes",
        "plan",
        "meeting",
        "draft",
        "review",
        "budget",
        "sketch",
        "letter",
        "window",
        "morning",
        "quiet",
        "bright",
        "simple",
        "careful",
        "steady",
        "open",
        "small",
        "clear",
        "write",
        "read",
        "build",
        "carry",
        "follow",
        "gather",
        "listen",
        "measure",
        "notice",
        "remember",
        "café",
        "naïve",
        "über",
        "東京",
        "привет",
        "emoji🙂",
        "a",
        "the",
        "of",
        "and",
    ];

    fn next(&mut self) -> &'static str {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let index = usize::try_from(self.0 >> 33).unwrap_or(0) % Self::LIST.len();
        Self::LIST[index]
    }

    fn sentence(&mut self, words: usize) -> String {
        (0..words).map(|_| self.next()).collect::<Vec<_>>().join(" ")
    }
}

/// Types into the window and keeps the list of blocks it should have made.
struct Typist<'a> {
    cx: &'a mut VisualTestContext,
    expected: Vec<Row>,
    words: usize,
}

impl Typist<'_> {
    fn text(&mut self, text: &str) {
        self.cx.simulate_input(text);
        self.words += text.split_whitespace().count();
    }

    fn keys(&mut self, keys: &str) {
        self.cx.simulate_keystrokes(keys);
    }

    /// Types `text` into the current (empty) block, with a typo fixed on the way, and records it.
    fn block(&mut self, depth: usize, kind: &'static str, text: &str) {
        let (first, rest) = text.split_at(text.find(' ').unwrap_or(text.len()));
        self.text(first);
        self.cx.simulate_input(" xqz");
        self.keys("backspace backspace backspace backspace");
        self.text(rest);
        self.expected.push((depth, kind, text.to_owned()));
    }

    fn section(&mut self, words: &mut Words) {
        // A heading by shortcut; Enter after it starts a paragraph.
        self.text("## ");
        self.block(0, "h", &words.sentence(3));
        self.keys("enter");
        for _ in 0..3 {
            self.block(0, "p", &words.sentence(60));
            self.keys("enter");
        }
        // A bulleted list nested two deep, then back out.
        self.text("- ");
        self.block(0, "bullet", &words.sentence(6));
        self.keys("enter tab");
        self.block(1, "bullet", &words.sentence(6));
        self.keys("enter tab");
        self.block(2, "bullet", &words.sentence(6));
        self.keys("enter shift-tab");
        self.block(1, "bullet", &words.sentence(6));
        // Enter on the empty item outdents it, then makes it a paragraph at the top level.
        self.keys("enter enter enter");
        // To-dos, ended the same way.
        self.text("[] ");
        self.block(0, "todo", &words.sentence(5));
        self.keys("enter");
        self.block(0, "todo", &words.sentence(5));
        self.keys("enter enter");
        // A quote: Enter is a new line inside it, and Enter on that empty line leaves it (E17-S13).
        self.text("> ");
        self.block(0, "quote", &words.sentence(20));
        self.keys("enter enter");
        // A numbered list.
        self.text("1. ");
        self.block(0, "number", &words.sentence(8));
        self.keys("enter");
        self.block(0, "number", &words.sentence(8));
        self.keys("enter enter");
        // A code block: Enter is a new line inside it, Cmd+Enter leaves it.
        self.text("```");
        let lines = [words.sentence(3), words.sentence(4), words.sentence(2)];
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                self.keys("enter");
            }
            self.text(line);
        }
        self.expected.push((0, "code", lines.join("\n")));
        self.keys("secondary-enter");
    }
}

#[gpui::test]
#[ignore = "slow in a debug build; run by `just acceptance`"]
fn a_three_thousand_word_document_typed_through_the_window_holds_exactly_what_was_typed(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| crate::init(None, cx));
    let (shell, cx) = cx.add_window_view(Shell::new);
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    let first = shell.update(cx, |shell, _| {
        shell.editor.open_page().unwrap().unwrap().ensure_block().unwrap()
    });
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.focus_page_at(Caret { block: first, at: 0 }, window, cx);
        });
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));

    let mut words = Words(2026);
    let mut typist = Typist {
        cx,
        expected: Vec::new(),
        words: 0,
    };
    while typist.words < WORDS {
        typist.section(&mut words);
        typist.cx.update(|window, cx| window.draw(cx).clear(cx));
    }
    let (mut expected, typed) = (typist.expected, typist.words);
    // Leaving the last code block leaves an empty paragraph to type in.
    expected.push((0, "p", String::new()));
    assert!(typed >= WORDS, "typed {typed} words");

    let rows = shell.update(cx, |shell, _| {
        rows_of(&PageView::build(shell.page().unwrap().doc()).unwrap())
    });
    for (i, (got, want)) in rows.iter().zip(&expected).enumerate() {
        assert_eq!(got, want, "block {i}");
    }
    assert_eq!(rows.len(), expected.len());

    // Edits in the middle: split a paragraph with Enter, join it back with Backspace, type a word.
    let middle = shell.update(cx, |shell, _| {
        let page = shell.page().unwrap();
        let rows = page.rows().unwrap();
        rows.iter()
            .filter(|row| kind(&row.style) == "p")
            .nth(5)
            .map(|row| row.id)
            .unwrap()
    });
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.focus_page_at(Caret { block: middle, at: 6 }, window, cx);
        });
    });
    cx.simulate_keystrokes("enter backspace");
    cx.simulate_input("inserted ");
    let edited = shell.update(cx, |shell, _| {
        shell.page().unwrap().doc().plain_text(middle).unwrap().unwrap()
    });
    let index = expected
        .iter()
        .enumerate()
        .filter(|(_, row)| row.1 == "p")
        .nth(5)
        .map(|(i, _)| i)
        .unwrap();
    let mut chars: Vec<char> = expected[index].2.chars().collect();
    chars.splice(6..6, "inserted ".chars());
    expected[index].2 = chars.into_iter().collect();
    assert_eq!(edited, expected[index].2);

    // The saved form reads back as the same page.
    let (now, reloaded) = shell.update(cx, |shell, _| {
        let doc = shell.page().unwrap().doc();
        let bytes = doc.to_bytes().unwrap();
        (
            rows_of(&PageView::build(doc).unwrap()),
            rows_of(&PageView::build(&PageDoc::from_bytes(&bytes).unwrap()).unwrap()),
        )
    });
    assert_eq!(now, expected);
    assert_eq!(reloaded, now);
}
