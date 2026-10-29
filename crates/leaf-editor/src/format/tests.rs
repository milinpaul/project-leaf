//! Tests for mark toggles and links (E04-S07).

use leaf_core::BlockId;
use leaf_model::{BlockKind, Caret, History, Mark, MarkKind, Marks, PageCommand, PageDoc, PageOutcome};

use crate::{Motion, PageEditor, TextRange};

fn page(blocks: &[(BlockKind, &str)]) -> (PageEditor, Vec<BlockId>) {
    let mut doc = PageDoc::new();
    let ids = blocks
        .iter()
        .map(|(kind, text)| {
            let index = doc.root_blocks().len();
            let Ok(PageOutcome::BlockCreated(id)) = doc.apply(PageCommand::InsertBlock {
                parent: None,
                index,
                kind: kind.clone(),
                text: (*text).into(),
            }) else {
                panic!("insert failed");
            };
            id
        })
        .collect();
    let mut doc = PageDoc::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    doc.set_undo_merge_interval(0);
    (PageEditor::new(doc), ids)
}

fn para(text: &str) -> (BlockKind, &str) {
    (BlockKind::Paragraph, text)
}

fn code(text: &str) -> (BlockKind, &str) {
    (
        BlockKind::Code {
            language: String::new(),
        },
        text,
    )
}

fn at(block: BlockId, at: usize) -> Caret {
    Caret { block, at }
}

/// The block's text as (text, marks) runs.
fn runs(editor: &PageEditor, id: BlockId) -> Vec<(String, Marks)> {
    editor
        .doc()
        .text(id)
        .unwrap()
        .unwrap()
        .into_iter()
        .map(|span| (span.text, span.marks))
        .collect()
}

fn plain(text: &str) -> (String, Marks) {
    (text.into(), Marks::default())
}

fn bold(text: &str) -> (String, Marks) {
    (
        text.into(),
        Marks {
            bold: true,
            ..Marks::default()
        },
    )
}

fn linked(text: &str, href: &str) -> (String, Marks) {
    (
        text.into(),
        Marks {
            link: Some(href.into()),
            ..Marks::default()
        },
    )
}

fn mark(editor: &mut PageEditor, id: BlockId, range: std::ops::Range<usize>, mark: Mark) {
    editor
        .doc
        .apply(PageCommand::SetMark { id, range, mark })
        .unwrap();
}

// ---- Toggles over a selection ----

#[test]
fn a_toggle_adds_the_mark_unless_the_whole_selection_has_it() {
    let (mut editor, ids) = page(&[para("one two three")]);
    let id = ids[0];
    mark(&mut editor, id, 4..7, Mark::Bold);
    // Partly bold: Cmd+B makes all of it bold.
    editor.select(id, 0..7).unwrap();
    editor.toggle_mark(MarkKind::Bold).unwrap();
    assert_eq!(runs(&editor, id), vec![bold("one two"), plain(" three")]);
    // All bold now: Cmd+B removes it.
    editor.toggle_mark(MarkKind::Bold).unwrap();
    assert_eq!(runs(&editor, id), vec![plain("one two three")]);
    assert_eq!(
        editor.selection().unwrap(),
        Some(crate::Selection::Text {
            start: at(id, 0),
            end: at(id, 7)
        }),
        "the selection stays"
    );
}

#[test]
fn a_toggle_across_blocks_is_one_undo_step_and_skips_code() {
    let (mut editor, ids) = page(&[para("first"), code("let x;"), para("last")]);
    editor.select_between(at(ids[0], 2), at(ids[2], 2)).unwrap();
    editor.toggle_mark(MarkKind::Italic).unwrap();
    let italic = |t: &str| {
        (
            t.to_owned(),
            Marks {
                italic: true,
                ..Marks::default()
            },
        )
    };
    assert_eq!(runs(&editor, ids[0]), vec![plain("fi"), italic("rst")]);
    assert_eq!(
        runs(&editor, ids[1]),
        vec![plain("let x;")],
        "code takes no marks"
    );
    assert_eq!(runs(&editor, ids[2]), vec![italic("la"), plain("st")]);
    assert_eq!(
        editor.doc.undo().unwrap(),
        History::Done {
            caret: Some(at(ids[2], 2))
        }
    );
    assert_eq!(runs(&editor, ids[0]), vec![plain("first")]);
    assert_eq!(
        runs(&editor, ids[2]),
        vec![plain("last")],
        "both blocks in one step"
    );
}

#[test]
fn code_blocks_do_not_count_when_deciding() {
    // The paragraph parts are all struck through; the code block in between can't be. So the
    // whole selection counts as struck, and the toggle removes it.
    let (mut editor, ids) = page(&[para("ab"), code("c"), para("de")]);
    mark(&mut editor, ids[0], 0..2, Mark::Strike);
    mark(&mut editor, ids[2], 0..2, Mark::Strike);
    editor.select_between(at(ids[0], 0), at(ids[2], 2)).unwrap();
    editor.toggle_mark(MarkKind::Strike).unwrap();
    assert_eq!(runs(&editor, ids[0]), vec![plain("ab")]);
    assert_eq!(runs(&editor, ids[2]), vec![plain("de")]);
}

#[test]
fn toggles_on_selected_blocks_format_all_their_text() {
    let (mut editor, ids) = page(&[para("a"), para("b")]);
    editor.select_blocks(&ids).unwrap();
    editor.toggle_mark(MarkKind::Code).unwrap();
    let coded = |t: &str| {
        (
            t.to_owned(),
            Marks {
                code: true,
                ..Marks::default()
            },
        )
    };
    assert_eq!(runs(&editor, ids[0]), vec![coded("a")]);
    assert_eq!(runs(&editor, ids[1]), vec![coded("b")]);
    assert!(editor.active_marks().unwrap().code);
}

// ---- Caret-only toggles ----

#[test]
fn a_toggle_with_just_a_caret_formats_the_next_typed_text() {
    let (mut editor, ids) = page(&[para("plain ")]);
    let id = ids[0];
    editor.set_caret(at(id, 6)).unwrap();
    editor.toggle_mark(MarkKind::Bold).unwrap();
    assert_eq!(runs(&editor, id), vec![plain("plain ")], "nothing written yet");
    assert!(editor.active_marks().unwrap().bold);
    editor.insert_text("lo").unwrap();
    editor.insert_text("ud").unwrap();
    assert_eq!(runs(&editor, id), vec![plain("plain "), bold("loud")]);
    // Toggling off carries on the same way.
    editor.toggle_mark(MarkKind::Bold).unwrap();
    editor.insert_text(" quiet").unwrap();
    assert_eq!(
        runs(&editor, id),
        vec![plain("plain "), bold("loud"), plain(" quiet")]
    );
}

#[test]
fn caret_toggles_combine_and_are_forgotten_when_the_caret_moves() {
    let (mut editor, ids) = page(&[para("ab")]);
    let id = ids[0];
    editor.set_caret(at(id, 1)).unwrap();
    editor.toggle_mark(MarkKind::Italic).unwrap();
    editor.toggle_mark(MarkKind::Strike).unwrap();
    editor.toggle_mark(MarkKind::Strike).unwrap();
    let marks = editor.active_marks().unwrap();
    assert!(marks.italic && !marks.strike, "strike toggled twice is off again");
    editor.move_caret(Motion::Right).unwrap();
    editor.move_caret(Motion::Left).unwrap();
    assert!(!editor.active_marks().unwrap().italic, "moving away drops it");
    editor.insert_text("x").unwrap();
    assert_eq!(runs(&editor, id), vec![plain("axb")]);
}

#[test]
fn inline_code_typed_after_a_toggle_keeps_going() {
    // Code doesn't grow by itself while typing; a pending toggle carries it along anyway.
    let (mut editor, ids) = page(&[para("")]);
    editor.set_caret(at(ids[0], 0)).unwrap();
    editor.toggle_mark(MarkKind::Code).unwrap();
    for ch in ["f", "n", "()"] {
        editor.insert_text(ch).unwrap();
    }
    assert_eq!(
        runs(&editor, ids[0]),
        vec![(
            "fn()".into(),
            Marks {
                code: true,
                ..Marks::default()
            }
        )]
    );
}

#[test]
fn typing_marks_follow_the_expand_rules() {
    let (mut editor, ids) = page(&[para("bold code end")]);
    let id = ids[0];
    mark(&mut editor, id, 0..4, Mark::Bold);
    mark(&mut editor, id, 5..9, Mark::Code);
    editor.set_caret(at(id, 4)).unwrap();
    assert!(editor.active_marks().unwrap().bold, "bold carries on after bold");
    editor.set_caret(at(id, 0)).unwrap();
    assert!(!editor.active_marks().unwrap().bold, "but not before it");
    editor.set_caret(at(id, 7)).unwrap();
    assert!(editor.active_marks().unwrap().code, "inside code");
    editor.set_caret(at(id, 9)).unwrap();
    assert!(!editor.active_marks().unwrap().code, "code doesn't carry on");
    assert_eq!(
        PageEditor::new(PageDoc::new()).active_marks().unwrap(),
        Marks::default()
    );
}

#[test]
fn caret_toggles_do_nothing_in_code_blocks_or_for_links() {
    let (mut editor, ids) = page(&[code("x"), para("y")]);
    editor.set_caret(at(ids[0], 1)).unwrap();
    editor.toggle_mark(MarkKind::Bold).unwrap();
    editor.insert_text("z").unwrap();
    assert_eq!(runs(&editor, ids[0]), vec![plain("xz")]);
    editor.set_caret(at(ids[1], 1)).unwrap();
    editor.toggle_mark(MarkKind::Link).unwrap();
    assert_eq!(editor.active_marks().unwrap(), Marks::default());
}

// ---- Links ----

#[test]
fn set_link_links_the_selection_and_edits_the_link_at_the_caret() {
    let (mut editor, ids) = page(&[para("see the docs now")]);
    let id = ids[0];
    editor.select(id, 8..12).unwrap();
    editor.set_link("  https://leaf.app/docs ").unwrap();
    assert_eq!(
        runs(&editor, id),
        vec![
            plain("see the "),
            linked("docs", "https://leaf.app/docs"),
            plain(" now")
        ]
    );
    // Caret inside the link: the popover shows it and edits all of it.
    editor.set_caret(at(id, 10)).unwrap();
    assert_eq!(
        editor.link_at_caret().unwrap(),
        Some((
            TextRange {
                block: id,
                range: 8..12
            },
            "https://leaf.app/docs".into()
        ))
    );
    editor.set_link("https://leaf.app/help").unwrap();
    assert_eq!(runs(&editor, id)[1], linked("docs", "https://leaf.app/help"));
    // Right after the link still counts as in it.
    editor.set_caret(at(id, 12)).unwrap();
    assert!(editor.link_at_caret().unwrap().is_some());
    editor.set_caret(at(id, 3)).unwrap();
    assert_eq!(editor.link_at_caret().unwrap(), None);
    editor.set_link("https://elsewhere").unwrap();
    assert_eq!(
        runs(&editor, id)[0],
        plain("see the "),
        "no selection, no link: nothing"
    );
}

#[test]
fn remove_link_from_the_caret_or_a_blank_address_or_a_toggle() {
    let (mut editor, ids) = page(&[para("a link here")]);
    let id = ids[0];
    let linked_up = |editor: &mut PageEditor| mark(editor, id, 2..6, Mark::Link("https://x.y".into()));
    linked_up(&mut editor);
    editor.set_caret(at(id, 4)).unwrap();
    editor.remove_link().unwrap();
    assert_eq!(runs(&editor, id), vec![plain("a link here")]);

    linked_up(&mut editor);
    editor.set_caret(at(id, 4)).unwrap();
    editor.set_link("   ").unwrap();
    assert_eq!(
        runs(&editor, id),
        vec![plain("a link here")],
        "blank address removes"
    );

    linked_up(&mut editor);
    editor.select(id, 0..4).unwrap();
    editor.toggle_mark(MarkKind::Link).unwrap();
    assert_eq!(runs(&editor, id).len(), 3, "not all linked: the toggle leaves it");
    editor.select(id, 2..6).unwrap();
    editor.toggle_mark(MarkKind::Link).unwrap();
    assert_eq!(
        runs(&editor, id),
        vec![plain("a link here")],
        "all linked: removed"
    );
    editor.set_caret(at(id, 1)).unwrap();
    editor.remove_link().unwrap();
    assert_eq!(editor.link_at_caret().unwrap(), None);
}

#[test]
fn neighbouring_links_with_different_addresses_are_separate() {
    let (mut editor, ids) = page(&[para("onetwo")]);
    let id = ids[0];
    mark(&mut editor, id, 0..3, Mark::Link("https://one".into()));
    mark(&mut editor, id, 3..6, Mark::Link("https://two".into()));
    editor.set_caret(at(id, 4)).unwrap();
    let (range, href) = editor.link_at_caret().unwrap().unwrap();
    assert_eq!((range.range, href.as_str()), (3..6, "https://two"));
    editor.select(id, 0..6).unwrap();
    assert_eq!(
        editor.active_marks().unwrap().link,
        None,
        "mixed addresses: no shared link"
    );
    assert_eq!(editor.link_at_caret().unwrap(), None, "text is selected");
}

#[test]
fn a_link_over_a_selection_is_one_undo_step() {
    let (mut editor, ids) = page(&[para("ab"), para("cd")]);
    editor.select_between(at(ids[0], 1), at(ids[1], 1)).unwrap();
    editor.set_link("https://leaf.app").unwrap();
    assert_eq!(runs(&editor, ids[1])[0], linked("c", "https://leaf.app"));
    editor.doc.undo().unwrap();
    assert_eq!(runs(&editor, ids[0]), vec![plain("ab")]);
    assert_eq!(runs(&editor, ids[1]), vec![plain("cd")]);
}
