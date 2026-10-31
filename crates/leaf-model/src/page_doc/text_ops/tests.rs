//! Tests for text and mark commands (E02-S03).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use leaf_core::BlockId;

use crate::{BlockKind, Mark, MarkKind, Marks, ModelError, PageCommand, PageDoc, PageOutcome};

/// An emoji with a skin-tone modifier (2 scalars), a space, a Devanagari word with a conjunct
/// (6 scalars) and "!": 10 Unicode scalar values in all.
const MIXED: &str = "👋🏽 नमस्ते!";

fn block(page: &mut PageDoc, kind: BlockKind, text: &str) -> BlockId {
    let index = page.root_blocks().len();
    match page
        .apply(PageCommand::InsertBlock {
            parent: None,
            index,
            kind,
            text: text.into(),
        })
        .unwrap()
    {
        PageOutcome::BlockCreated(id) => id,
        other => panic!("expected a new block, got {other:?}"),
    }
}

fn text_of(page: &PageDoc, id: BlockId) -> String {
    page.plain_text(id).unwrap().unwrap()
}

/// The text of each span that carries the given marks.
fn runs(page: &PageDoc, id: BlockId, pick: impl Fn(&Marks) -> bool) -> Vec<String> {
    page.text(id)
        .unwrap()
        .unwrap()
        .into_iter()
        .filter(|s| pick(&s.marks))
        .map(|s| s.text)
        .collect()
}

fn commit_counter(page: &PageDoc) -> (Arc<AtomicUsize>, loro::Subscription) {
    let count = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&count);
    let sub = page.doc.subscribe_local_update(Box::new(move |_| {
        seen.fetch_add(1, Ordering::SeqCst);
        true
    }));
    (count, sub)
}

#[test]
fn insert_and_delete_use_unicode_scalar_positions_with_emoji_and_indic_text() {
    assert_eq!(MIXED.chars().count(), 10);
    for pos in 0..=10 {
        let mut page = PageDoc::new();
        let id = block(&mut page, BlockKind::Paragraph, MIXED);
        page.apply(PageCommand::InsertText {
            id,
            pos,
            text: "|".into(),
        })
        .unwrap();
        let mut expected: Vec<char> = MIXED.chars().collect();
        expected.insert(pos, '|');
        assert_eq!(
            text_of(&page, id),
            expected.iter().collect::<String>(),
            "insert at {pos}"
        );
    }

    let mut page = PageDoc::new();
    let id = block(&mut page, BlockKind::Paragraph, MIXED);
    page.apply(PageCommand::DeleteText { id, range: 0..2 }).unwrap();
    assert_eq!(
        text_of(&page, id),
        " नमस्ते!",
        "the emoji and its skin tone are two scalars"
    );
    page.apply(PageCommand::DeleteText { id, range: 4..6 }).unwrap();
    assert_eq!(
        text_of(&page, id),
        " नमसे!",
        "removes the virama and ta (scalars 4..6), keeping the vowel sign"
    );
    let err = page
        .apply(PageCommand::InsertText {
            id,
            pos: 7,
            text: "x".into(),
        })
        .unwrap_err();
    assert!(
        matches!(err, ModelError::TextOutOfRange { end: 7, len: 6 }),
        "{err}"
    );
}

#[test]
fn marks_follow_the_expand_table_when_typing_at_their_edges() {
    let mut page = PageDoc::new();
    let id = block(&mut page, BlockKind::Paragraph, "ab cd ef gh");
    let set = |page: &mut PageDoc, range, mark| page.apply(PageCommand::SetMark { id, range, mark }).unwrap();
    set(&mut page, 0..2, Mark::Bold);
    set(&mut page, 3..5, Mark::Code);
    set(&mut page, 6..8, Mark::Link("https://leaf.app".into()));
    set(&mut page, 9..11, Mark::Italic);

    // Type right after each marked run (from the end backwards, so positions stay valid).
    for pos in [11, 8, 5, 2] {
        page.apply(PageCommand::InsertText {
            id,
            pos,
            text: "+".into(),
        })
        .unwrap();
    }
    assert_eq!(text_of(&page, id), "ab+ cd+ ef+ gh+");
    assert_eq!(runs(&page, id, |m| m.bold), vec!["ab+"], "bold grows");
    assert_eq!(runs(&page, id, |m| m.italic), vec!["gh+"], "italic grows");
    assert_eq!(
        runs(&page, id, |m| m.code),
        vec!["cd"],
        "inline code does not grow"
    );
    assert_eq!(
        runs(&page, id, |m| m.link.is_some()),
        vec!["ef"],
        "links do not grow"
    );
    let spans = page.text(id).unwrap().unwrap();
    let link = spans.iter().find(|s| s.marks.link.is_some()).unwrap();
    assert_eq!(
        link.marks.link.as_deref(),
        Some("https://leaf.app"),
        "the link keeps its address"
    );

    page.apply(PageCommand::SetMark {
        id,
        range: 12..15,
        mark: Mark::Strike,
    })
    .unwrap();
    assert_eq!(runs(&page, id, |m| m.strike), vec!["gh+"]);
}

#[test]
fn remove_mark_clears_part_or_all_of_a_mark() {
    let mut page = PageDoc::new();
    let id = block(&mut page, BlockKind::Paragraph, "linked words");
    page.apply(PageCommand::SetMark {
        id,
        range: 0..12,
        mark: Mark::Bold,
    })
    .unwrap();
    page.apply(PageCommand::SetMark {
        id,
        range: 0..6,
        mark: Mark::Link("https://a.b".into()),
    })
    .unwrap();

    page.apply(PageCommand::RemoveMark {
        id,
        range: 6..12,
        kind: MarkKind::Bold,
    })
    .unwrap();
    assert_eq!(runs(&page, id, |m| m.bold), vec!["linked"]);
    page.apply(PageCommand::RemoveMark {
        id,
        range: 0..6,
        kind: MarkKind::Link,
    })
    .unwrap();
    assert_eq!(
        runs(&page, id, |m| m.link.is_some()),
        Vec::<String>::new(),
        "no address needed to remove a link"
    );
}

#[test]
fn code_blocks_take_text_but_no_marks() {
    let mut page = PageDoc::new();
    let id = block(
        &mut page,
        BlockKind::Code {
            language: "rust".into(),
        },
        "fn main() {}",
    );
    let err = page
        .apply(PageCommand::SetMark {
            id,
            range: 0..2,
            mark: Mark::Bold,
        })
        .unwrap_err();
    assert!(
        matches!(&err, ModelError::MarksNotAllowed { kind } if kind == "code"),
        "{err}"
    );
    page.apply(PageCommand::InsertText {
        id,
        pos: 12,
        text: "\n".into(),
    })
    .unwrap();
    assert_eq!(text_of(&page, id), "fn main() {}\n");
    assert_eq!(
        page.apply(PageCommand::RemoveMark {
            id,
            range: 0..2,
            kind: MarkKind::Bold
        })
        .unwrap(),
        PageOutcome::Unchanged,
        "there is nothing to remove"
    );
}

#[test]
fn text_commands_refuse_bad_input_and_write_nothing() {
    let mut page = PageDoc::new();
    let id = block(&mut page, BlockKind::Paragraph, "four");
    let divider = block(&mut page, BlockKind::Divider, "");
    let (commits, _sub) = commit_counter(&page);

    let attempt = |page: &mut PageDoc, command| page.apply(command);
    assert!(matches!(
        attempt(
            &mut page,
            PageCommand::InsertText {
                id: divider,
                pos: 0,
                text: "x".into()
            }
        ),
        Err(ModelError::TextNotAllowed { .. })
    ));
    assert!(matches!(
        attempt(&mut page, PageCommand::DeleteText { id, range: 2..9 }),
        Err(ModelError::TextOutOfRange { end: 9, len: 4 })
    ));
    #[expect(
        clippy::reversed_empty_ranges,
        reason = "a backwards range is the input under test"
    )]
    let backwards = 3..1;
    assert!(matches!(
        attempt(&mut page, PageCommand::DeleteText { id, range: backwards }),
        Err(ModelError::TextOutOfRange { .. })
    ));
    assert!(matches!(
        attempt(
            &mut page,
            PageCommand::SetMark {
                id,
                range: 0..2,
                mark: Mark::Link(String::new())
            }
        ),
        Err(ModelError::EmptyLink)
    ));
    assert!(matches!(
        attempt(
            &mut page,
            PageCommand::SetMark {
                id,
                range: 0..5,
                mark: Mark::Italic
            }
        ),
        Err(ModelError::TextOutOfRange { .. })
    ));
    let ghost = BlockId::new();
    assert!(matches!(
        attempt(
            &mut page,
            PageCommand::InsertText {
                id: ghost,
                pos: 0,
                text: "x".into()
            }
        ),
        Err(ModelError::BlockNotFound(_))
    ));

    for no_op in [
        PageCommand::InsertText {
            id,
            pos: 2,
            text: String::new(),
        },
        PageCommand::DeleteText { id, range: 2..2 },
        PageCommand::SetMark {
            id,
            range: 1..1,
            mark: Mark::Bold,
        },
        PageCommand::RemoveMark {
            id,
            range: 0..4,
            kind: MarkKind::Strike,
        },
    ] {
        assert_eq!(
            attempt(&mut page, no_op.clone()).unwrap(),
            PageOutcome::Unchanged,
            "{no_op:?}"
        );
    }
    assert_eq!(commits.load(Ordering::SeqCst), 0);
    assert_eq!(text_of(&page, id), "four");
}

#[test]
fn unknown_blocks_are_never_edited_even_if_they_have_text() {
    // A future kind that happens to carry text: written raw, then loaded.
    let id = BlockId::new();
    let doc = loro::LoroDoc::new();
    let tree = doc.get_tree("blocks");
    tree.enable_fractional_index(0);
    let meta = tree.get_meta(tree.create(None).unwrap()).unwrap();
    meta.insert("id", id.to_string()).unwrap();
    meta.insert("type", "sticky_note").unwrap();
    meta.ensure_mergeable_text("text")
        .unwrap()
        .insert(0, "kept")
        .unwrap();
    doc.commit();
    let mut page = PageDoc::from_bytes(&doc.export(loro::ExportMode::Snapshot).unwrap()).unwrap();

    let err = page
        .apply(PageCommand::InsertText {
            id,
            pos: 0,
            text: "x".into(),
        })
        .unwrap_err();
    assert!(
        matches!(&err, ModelError::TextNotAllowed { kind } if kind == "sticky_note"),
        "{err}"
    );
}

#[test]
fn text_errors_explain_themselves() {
    assert_eq!(
        ModelError::TextOutOfRange { end: 9, len: 4 }.to_string(),
        "text position 9 is past the end of the text (4 characters)"
    );
    assert_eq!(
        ModelError::MarksNotAllowed { kind: "code".into() }.to_string(),
        "code blocks take no formatting"
    );
    assert_eq!(ModelError::EmptyLink.to_string(), "a link needs an address");
}

#[test]
fn mark_commands_that_change_nothing_write_nothing() {
    // Regression (E02-S11, found by the 10,000-case undo property test): Loro writes an op for
    // these even though the text looks the same, but records no undo step for it.
    let mut page = PageDoc::new();
    let id = block(&mut page, BlockKind::Paragraph, "leaf");
    page.apply(PageCommand::SetMark {
        id,
        range: 0..2,
        mark: Mark::Italic,
    })
    .unwrap();
    let unchanged = [
        PageCommand::RemoveMark {
            id,
            range: 2..3,
            kind: MarkKind::Italic,
        },
        PageCommand::SetMark {
            id,
            range: 0..1,
            mark: Mark::Italic,
        },
    ];
    for command in unchanged {
        let before = page.to_bytes().unwrap();
        assert_eq!(
            page.apply(command.clone()).unwrap(),
            PageOutcome::Unchanged,
            "{command:?}"
        );
        assert_eq!(page.to_bytes().unwrap(), before, "{command:?} wrote something");
    }

    // A different link target is a real change, and so is a range only partly marked.
    page.apply(PageCommand::SetMark {
        id,
        range: 0..4,
        mark: Mark::Link("https://a.example".into()),
    })
    .unwrap();
    for command in [
        PageCommand::SetMark {
            id,
            range: 0..4,
            mark: Mark::Link("https://b.example".into()),
        },
        PageCommand::SetMark {
            id,
            range: 1..3,
            mark: Mark::Italic,
        },
        PageCommand::RemoveMark {
            id,
            range: 1..2,
            kind: MarkKind::Italic,
        },
    ] {
        assert_eq!(
            page.apply(command.clone()).unwrap(),
            PageOutcome::Changed,
            "{command:?}"
        );
    }
}
