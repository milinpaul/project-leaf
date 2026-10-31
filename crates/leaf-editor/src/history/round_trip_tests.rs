//! Undo and redo over whole editing actions (E08-S03): random sequences of what a person does in the
//! editor (typing, typing over a selection, Enter, Backspace, Delete, Tab, pasting text, cut and
//! paste, bold, block menu actions and dragging blocks), each of which may change several blocks at
//! once. Undoing them all must give back every earlier page exactly, one action per step, and redoing
//! them all must replay them exactly. The model's own commands are checked the same way in
//! `leaf-model`'s `page_invariants` test; this checks the actions built on them.

use leaf_core::BlockId;
use leaf_model::{BlockKind, Caret, MarkKind, PageCommand, PageDoc, PageOutcome, TextSpan};
use proptest::prelude::*;

use crate::{BlockAction, Key, PageEditor, RowBox};

/// Everything stored about the page that a person could see: every block in reading order with its
/// depth, identity, kind and formatted text.
type Outline = Vec<(usize, BlockId, BlockKind, Option<Vec<TextSpan>>)>;

fn outline(editor: &PageEditor) -> Outline {
    fn walk(doc: &PageDoc, blocks: Vec<BlockId>, depth: usize, out: &mut Outline) {
        for block in blocks {
            out.push((depth, block, doc.kind(block).unwrap(), doc.text(block).unwrap()));
            walk(doc, doc.children(block).unwrap(), depth + 1, out);
        }
    }
    let mut out = Vec::new();
    walk(editor.doc(), editor.doc().root_blocks(), 0, &mut out);
    out
}

/// A small page with most kinds of block, nesting and a code block, loaded from bytes so building it
/// is not an undo step.
fn page() -> PageEditor {
    let blocks = [
        (0, BlockKind::Heading { level: 1 }, "Plan"),
        (0, BlockKind::Paragraph, "first words here"),
        (0, BlockKind::BulletedList, "one"),
        (1, BlockKind::BulletedList, "one inside"),
        (0, BlockKind::Todo { checked: false }, "call back"),
        (0, BlockKind::Quote, "said once"),
        (0, BlockKind::Paragraph, "last line"),
    ];
    let mut doc = PageDoc::new();
    let mut parents: Vec<BlockId> = Vec::new();
    for (depth, kind, text) in blocks {
        parents.truncate(depth);
        let parent = parents.last().copied();
        let index = parent.map_or_else(|| doc.root_blocks().len(), |p| doc.children(p).unwrap().len());
        let Ok(PageOutcome::BlockCreated(id)) = doc.apply(PageCommand::InsertBlock {
            parent,
            index,
            kind,
            text: text.into(),
        }) else {
            panic!("insert failed");
        };
        parents.push(id);
    }
    let mut doc = PageDoc::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    doc.set_undo_merge_interval(0);
    PageEditor::new(doc)
}

/// A place on the page picked by two numbers: a block (by its position among the rows now) and an
/// offset in its text, both wrapped into range.
#[derive(Debug, Clone, Copy)]
struct Spot {
    block: usize,
    at: usize,
}

impl Spot {
    fn caret(self, editor: &PageEditor) -> Caret {
        let rows = editor.rows().unwrap();
        let row = &rows[self.block % rows.len()];
        let len = row.text.as_deref().map_or(0, |t| t.chars().count());
        Caret {
            block: row.id,
            at: self.at % (len + 1),
        }
    }

    fn block(self, editor: &PageEditor) -> BlockId {
        self.caret(editor).block
    }
}

#[derive(Debug, Clone)]
enum Step {
    Type(Spot, String),
    TypeOver(Spot, Spot),
    Press(Spot, Key),
    PressOver(Spot, Spot, Key),
    Paste(Spot, String),
    CutAndPaste(Spot, Spot, Spot),
    Bold(Spot, Spot),
    Action(Spot, usize, BlockAction),
    Drag(Spot, usize, f32, f32),
}

fn spot() -> impl Strategy<Value = Spot> {
    (0..20_usize, 0..30_usize).prop_map(|(block, at)| Spot { block, at })
}

fn key() -> impl Strategy<Value = Key> {
    prop_oneof![
        Just(Key::Enter),
        Just(Key::Backspace),
        Just(Key::Delete),
        Just(Key::Tab),
        Just(Key::ShiftTab),
    ]
}

fn action() -> impl Strategy<Value = BlockAction> {
    prop_oneof![
        Just(BlockAction::TurnInto(BlockKind::Paragraph)),
        Just(BlockAction::TurnInto(BlockKind::Heading { level: 2 })),
        Just(BlockAction::TurnInto(BlockKind::NumberedList)),
        Just(BlockAction::TurnInto(BlockKind::Todo { checked: false })),
        Just(BlockAction::Duplicate),
        Just(BlockAction::Delete),
        Just(BlockAction::MoveUp),
        Just(BlockAction::MoveDown),
    ]
}

/// Typed and pasted text: plain letters only, so no Markdown shortcut fires as a separate step (a
/// shortcut is two undo steps on purpose: undo turns the block back into the typed text).
fn words() -> impl Strategy<Value = String> {
    "[a-z]{1,4}( [a-z]{1,4})?"
}

/// Pasted text: one line, or several that become blocks, some in Markdown.
fn pasted() -> impl Strategy<Value = String> {
    prop_oneof![
        words(),
        Just("one\ntwo".to_owned()),
        Just("# Title\n- item\n- [ ] task\nplain".to_owned()),
        Just("**bold** and `code`".to_owned()),
    ]
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        3 => (spot(), words()).prop_map(|(at, text)| Step::Type(at, text)),
        2 => (spot(), spot()).prop_map(|(a, b)| Step::TypeOver(a, b)),
        3 => (spot(), key()).prop_map(|(at, key)| Step::Press(at, key)),
        1 => (spot(), spot(), prop_oneof![Just(Key::Backspace), Just(Key::Enter)])
            .prop_map(|(a, b, key)| Step::PressOver(a, b, key)),
        2 => (spot(), pasted()).prop_map(|(at, text)| Step::Paste(at, text)),
        1 => (spot(), spot(), spot()).prop_map(|(a, b, to)| Step::CutAndPaste(a, b, to)),
        1 => (spot(), spot()).prop_map(|(a, b)| Step::Bold(a, b)),
        2 => (spot(), 1..3_usize, action()).prop_map(|(at, count, action)| Step::Action(at, count, action)),
        2 => (spot(), 1..3_usize, -10.0_f32..200.0, -10.0_f32..80.0)
            .prop_map(|(at, count, y, x)| Step::Drag(at, count, y, x)),
    ]
}

/// Rows 20 high, one under another, as the window would lay them out.
fn layout(editor: &PageEditor) -> Vec<RowBox> {
    #[expect(clippy::cast_precision_loss, reason = "a few test rows")]
    let rows = editor
        .rows()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(i, row)| RowBox {
            block: row.id,
            top: i as f32 * 20.0,
            bottom: (i + 1) as f32 * 20.0,
        })
        .collect();
    rows
}

/// `count` blocks in reading order from `from`'s row on.
fn run_of(editor: &PageEditor, from: Spot, count: usize) -> Vec<BlockId> {
    let rows = editor.rows().unwrap();
    let first = from.block % rows.len();
    rows[first..(first + count).min(rows.len())]
        .iter()
        .map(|row| row.id)
        .collect()
}

/// Does one step. Errors are fine (an action refused where it doesn't apply); what matters is that
/// whatever it did is one undo step.
fn run(editor: &mut PageEditor, step: Step) {
    match step {
        Step::Type(at, text) => {
            let caret = at.caret(editor);
            let _ = editor.set_caret(caret).and_then(|_| editor.insert_text(&text));
        }
        Step::TypeOver(a, b) => {
            let (a, b) = (a.caret(editor), b.caret(editor));
            let _ = editor.select_between(a, b).and_then(|()| editor.insert_text("x"));
        }
        Step::Press(at, key) => {
            let caret = at.caret(editor);
            let _ = editor.set_caret(caret).and_then(|_| editor.press(key));
        }
        Step::PressOver(a, b, key) => {
            let (a, b) = (a.caret(editor), b.caret(editor));
            let _ = editor.select_between(a, b).and_then(|()| editor.press(key));
        }
        Step::Paste(at, text) => {
            let caret = at.caret(editor);
            let _ = editor.set_caret(caret).and_then(|_| editor.paste_text(&text));
        }
        Step::CutAndPaste(..) => panic!("run step by step by the test"),
        Step::Bold(a, b) => {
            let (a, b) = (a.caret(editor), b.caret(editor));
            let _ = editor
                .select_between(a, b)
                .and_then(|()| editor.toggle_mark(MarkKind::Bold));
        }
        Step::Action(at, count, action) => {
            let blocks = run_of(editor, at, count);
            let _ = editor
                .select_blocks(&blocks)
                .and_then(|()| editor.block_action(at.block(editor), action));
        }
        Step::Drag(at, count, y, x) => {
            let dragged = run_of(editor, at, count);
            let Ok(dragged) = editor
                .action_targets(dragged[0])
                .map(|t| if t.len() > 1 { t } else { dragged.clone() })
            else {
                return;
            };
            let rows = layout(editor);
            if let Ok(Some(drop)) = editor.drop_target(&dragged, &rows, x, y, 24.0) {
                let _ = editor.drop_blocks(&dragged, &drop);
            }
        }
    }
}

/// One random sequence of actions: undo them all, checking each earlier page comes back exactly, then
/// redo them all.
fn round_trip(steps: Vec<Step>) -> Result<(), TestCaseError> {
    let mut editor = page();
    let mut history = vec![outline(&editor)];
    // Whether an action wrote anything is read from the document's saved form, not from how the
    // page looks: typing "x" over a selected "x" looks the same but is still an edit and a step.
    let mut written = editor.doc().to_bytes().unwrap();
    let mut record = |editor: &PageEditor, history: &mut Vec<Outline>| {
        let now = editor.doc().to_bytes().unwrap();
        if now != written {
            written = now;
            history.push(outline(editor));
        }
    };
    for step in steps {
        // A cut and paste is two actions: check after each one.
        if let Step::CutAndPaste(a, b, to) = step {
            let (a, b) = (a.caret(&editor), b.caret(&editor));
            if let Ok(Some(cut)) = editor.select_between(a, b).and_then(|()| editor.cut()) {
                record(&editor, &mut history);
                let _ = editor
                    .set_caret(to.caret(&editor))
                    .and_then(|_| editor.paste(&cut));
            }
        } else {
            run(&mut editor, step);
        }
        prop_assert_eq!(editor.doc().check_invariants(), Ok(()));
        record(&editor, &mut history);
    }

    let last = history.len() - 1;
    for (i, expected) in history[..last].iter().enumerate().rev() {
        prop_assert!(editor.undo().unwrap(), "undo {} of {}", last - i, last);
        prop_assert_eq!(&outline(&editor), expected, "after undoing back to step {}", i);
    }
    prop_assert!(!editor.undo().unwrap(), "one undo step per action, no more");
    for (i, expected) in history.iter().enumerate().skip(1) {
        prop_assert!(editor.redo().unwrap());
        prop_assert_eq!(&outline(&editor), expected, "after redoing step {}", i);
    }
    prop_assert!(!editor.redo().unwrap());
    Ok(())
}

/// The cases are shared by four tests that run side by side (E08-S17): each runs a quarter of
/// `PROPTEST_CASES` (10,000 on CI's long run) or of 128 by default, so the whole set takes a quarter of
/// the time. Each part draws its own random sequences.
fn round_trip_part() {
    let total = std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|cases| cases.parse::<u32>().ok())
        .unwrap_or(128);
    let config = ProptestConfig {
        cases: total.div_ceil(4),
        failure_persistence: None,
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(config);
    if let Err(failure) = runner.run(&proptest::collection::vec(step(), 1..20), round_trip) {
        panic!("{failure}");
    }
}

#[test]
fn undoing_every_action_gives_back_each_earlier_page_and_redo_replays_them_part_1() {
    round_trip_part();
}

#[test]
fn undoing_every_action_gives_back_each_earlier_page_and_redo_replays_them_part_2() {
    round_trip_part();
}

#[test]
fn undoing_every_action_gives_back_each_earlier_page_and_redo_replays_them_part_3() {
    round_trip_part();
}

#[test]
fn undoing_every_action_gives_back_each_earlier_page_and_redo_replays_them_part_4() {
    round_trip_part();
}
