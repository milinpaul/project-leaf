//! Tests for page links in the page model (E10-S02, ADR 0071).

use leaf_core::{BlockId, PageId};
use loro::{Container, ExpandType, LoroDoc, LoroText, StyleConfig, ValueOrContainer};

use super::super::PageDoc;
use super::super::tests::insert;
use crate::block_kind::keys;
use crate::text::marks;
use crate::{BlockKind, Mark, MarkKind, Marks, ModelError, PAGE_LINK, PageCommand, PageOutcome, TextSpan};

fn link(page: &mut PageDoc, id: BlockId, pos: usize, target: PageId) -> Result<PageOutcome, ModelError> {
    page.apply(PageCommand::InsertPageLink {
        id,
        pos,
        page: target,
    })
}

fn spans(page: &PageDoc, id: BlockId) -> Vec<TextSpan> {
    page.text(id).unwrap().unwrap()
}

fn plain(text: &str) -> TextSpan {
    TextSpan {
        text: text.into(),
        marks: Marks::default(),
    }
}

fn linked(target: PageId) -> TextSpan {
    TextSpan {
        text: PAGE_LINK.to_string(),
        marks: Marks {
            page: Some(target),
            ..Marks::default()
        },
    }
}

#[test]
fn a_link_is_one_placeholder_carrying_the_page() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, BlockKind::Paragraph, "See  for dates");
    let trip = PageId::new();
    assert_eq!(link(&mut page, id, 4, trip).unwrap(), PageOutcome::Changed);
    assert_eq!(
        spans(&page, id),
        vec![plain("See "), linked(trip), plain(" for dates")]
    );
    assert_eq!(page.page_links(), vec![(id, trip)]);
    assert_eq!(page.check_invariants(), Ok(()));
}

#[test]
fn typing_beside_a_link_never_joins_it() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, BlockKind::Paragraph, "ab");
    let trip = PageId::new();
    link(&mut page, id, 1, trip).unwrap();
    for pos in [2, 1] {
        page.apply(PageCommand::InsertText {
            id,
            pos,
            text: "x".into(),
        })
        .unwrap();
    }
    assert_eq!(spans(&page, id), vec![plain("ax"), linked(trip), plain("xb")]);
}

#[test]
fn inserting_a_link_is_one_undo_step() {
    let mut page = PageDoc::new();
    page.set_undo_merge_interval(0);
    let id = insert(&mut page, BlockKind::Paragraph, "ab");
    link(&mut page, id, 1, PageId::new()).unwrap();
    page.undo().unwrap();
    assert_eq!(spans(&page, id), vec![plain("ab")]);
    assert_eq!(page.page_links(), Vec::new());
}

#[test]
fn links_are_refused_where_marks_are() {
    let mut page = PageDoc::new();
    let code = insert(
        &mut page,
        BlockKind::Code {
            language: String::new(),
        },
        "x",
    );
    assert!(matches!(
        link(&mut page, code, 0, PageId::new()),
        Err(ModelError::MarksNotAllowed { .. })
    ));
    let divider = insert(&mut page, BlockKind::Divider, "");
    assert!(matches!(
        link(&mut page, divider, 0, PageId::new()),
        Err(ModelError::TextNotAllowed { .. })
    ));
    let text = insert(&mut page, BlockKind::Paragraph, "ab");
    assert!(matches!(
        link(&mut page, text, 3, PageId::new()),
        Err(ModelError::TextOutOfRange { .. })
    ));
}

#[test]
fn plain_text_cannot_bring_in_the_placeholder() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, BlockKind::Paragraph, "ab");
    let bare = format!("x{PAGE_LINK}");
    for command in [
        PageCommand::InsertText {
            id,
            pos: 0,
            text: bare.clone(),
        },
        PageCommand::ReplaceText {
            id,
            range: 0..1,
            text: bare.clone(),
        },
        PageCommand::InsertBlock {
            parent: None,
            index: 0,
            kind: BlockKind::Paragraph,
            text: bare.clone(),
        },
    ] {
        assert!(matches!(page.apply(command), Err(ModelError::PageLinkCharInText)));
    }
    assert_eq!(spans(&page, id), vec![plain("ab")]);
}

#[test]
fn replacing_a_link_with_words_leaves_no_link() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, BlockKind::Paragraph, "ab");
    link(&mut page, id, 1, PageId::new()).unwrap();
    page.apply(PageCommand::SetMark {
        id,
        range: 1..2,
        mark: Mark::Bold,
    })
    .unwrap();
    page.apply(PageCommand::ReplaceText {
        id,
        range: 1..2,
        text: "Trip".into(),
    })
    .unwrap();
    let bold = Marks {
        bold: true,
        ..Marks::default()
    };
    assert_eq!(
        spans(&page, id),
        vec![
            plain("a"),
            TextSpan {
                text: "Trip".into(),
                marks: bold
            },
            plain("b")
        ],
        "the words keep the link's other formatting, not the link"
    );
    assert_eq!(page.check_invariants(), Ok(()));
}

#[test]
fn the_page_mark_only_repoints_links() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, BlockKind::Paragraph, "ab");
    let (trip, budget) = (PageId::new(), PageId::new());
    link(&mut page, id, 1, trip).unwrap();
    assert!(matches!(
        page.apply(PageCommand::SetMark {
            id,
            range: 0..2,
            mark: Mark::PageLink(budget)
        }),
        Err(ModelError::PageLinkOnText)
    ));
    assert!(matches!(
        page.apply(PageCommand::SetMark {
            id,
            range: 1..2,
            mark: Mark::PageLink(budget)
        }),
        Ok(PageOutcome::Changed)
    ));
    assert_eq!(page.page_links(), vec![(id, budget)]);
    assert!(matches!(
        page.apply(PageCommand::SetMark {
            id,
            range: 1..2,
            mark: Mark::PageLink(budget)
        }),
        Ok(PageOutcome::Unchanged)
    ));
}

#[test]
fn a_link_goes_by_deleting_it_not_by_removing_its_mark() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, BlockKind::Paragraph, "ab");
    link(&mut page, id, 1, PageId::new()).unwrap();
    assert!(matches!(
        page.apply(PageCommand::RemoveMark {
            id,
            range: 0..3,
            kind: MarkKind::PageLink
        }),
        Err(ModelError::PageLinkNotRemovable)
    ));
    assert!(
        matches!(
            page.apply(PageCommand::RemoveMark {
                id,
                range: 0..1,
                kind: MarkKind::PageLink
            }),
            Ok(PageOutcome::Unchanged)
        ),
        "no link in the range: nothing to refuse"
    );
    page.apply(PageCommand::DeleteText { id, range: 1..2 }).unwrap();
    assert_eq!(spans(&page, id), vec![plain("ab")]);
}

#[test]
fn split_merge_and_duplicate_keep_links_whole() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, BlockKind::Paragraph, "ab");
    let trip = PageId::new();
    link(&mut page, id, 1, trip).unwrap();
    let PageOutcome::BlockCreated(tail) = page.apply(PageCommand::SplitBlock { id, pos: 1 }).unwrap() else {
        panic!("split makes a block");
    };
    assert_eq!(spans(&page, tail), vec![linked(trip), plain("b")]);
    page.apply(PageCommand::MergeIntoPrevious { id: tail }).unwrap();
    assert_eq!(spans(&page, id), vec![plain("a"), linked(trip), plain("b")]);
    page.apply(PageCommand::DuplicateBlocks { ids: vec![id] })
        .unwrap();
    assert_eq!(page.page_links().len(), 2);
    assert!(page.page_links().iter().all(|(_, target)| *target == trip));
    assert_eq!(page.check_invariants(), Ok(()));
}

#[test]
fn turning_a_block_into_code_drops_its_links_and_undo_brings_them_back() {
    let mut page = PageDoc::new();
    page.set_undo_merge_interval(0);
    let id = insert(&mut page, BlockKind::Paragraph, "ab");
    let trip = PageId::new();
    link(&mut page, id, 1, trip).unwrap();
    page.apply(PageCommand::SetBlockKind {
        id,
        kind: BlockKind::Code {
            language: String::new(),
        },
    })
    .unwrap();
    assert_eq!(spans(&page, id), vec![plain("ab")]);
    assert_eq!(page.check_invariants(), Ok(()));
    page.undo().unwrap();
    assert_eq!(spans(&page, id), vec![plain("a"), linked(trip), plain("b")]);
}

#[test]
fn page_links_lists_every_link_in_reading_order() {
    let mut page = PageDoc::new();
    let first = insert(&mut page, BlockKind::Paragraph, "");
    let PageOutcome::BlockCreated(child) = page
        .apply(PageCommand::InsertBlock {
            parent: Some(first),
            index: 0,
            kind: BlockKind::BulletedList,
            text: String::new(),
        })
        .unwrap()
    else {
        panic!("a block");
    };
    let last = insert(&mut page, BlockKind::Quote, "");
    let pages: Vec<PageId> = (0..4).map(|_| PageId::new()).collect();
    link(&mut page, last, 0, pages[3]).unwrap();
    link(&mut page, child, 0, pages[2]).unwrap();
    link(&mut page, first, 0, pages[1]).unwrap();
    link(&mut page, first, 0, pages[0]).unwrap();
    assert_eq!(
        page.page_links(),
        vec![
            (first, pages[0]),
            (first, pages[1]),
            (child, pages[2]),
            (last, pages[3])
        ]
    );
}

/// The block's text container in a raw Loro doc, as an older version of Leaf would reach it.
fn raw_text(doc: &LoroDoc) -> LoroText {
    let tree = doc.get_tree("blocks");
    let node = tree.roots()[0];
    match tree.get_meta(node).unwrap().get(keys::TEXT) {
        Some(ValueOrContainer::Container(Container::Text(text))) => text,
        _ => panic!("a text block"),
    }
}

/// A copy of `page` opened by a version of Leaf that doesn't know the page mark.
fn older_copy(page: &PageDoc) -> LoroDoc {
    let doc = LoroDoc::new();
    doc.set_peer_id(99).unwrap();
    doc.config_default_text_style(Some(StyleConfig {
        expand: ExpandType::None,
    }));
    doc.import(&page.to_bytes().unwrap()).unwrap();
    doc
}

#[test]
fn an_older_version_breaking_a_link_is_repaired_on_merge() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, BlockKind::Paragraph, "ab");
    let (trip, budget) = (PageId::new(), PageId::new());
    link(&mut page, id, 1, trip).unwrap();
    link(&mut page, id, 3, budget).unwrap();

    // The older version types over the first link, carrying its unknown mark onto the words (as today's
    // ReplaceText does with marks it doesn't know), and strips the second link's mark.
    let old = older_copy(&page);
    let text = raw_text(&old);
    text.delete(1, 1).unwrap();
    text.insert(1, "Trip").unwrap();
    text.mark(1..5, marks::PAGE, trip.to_string()).unwrap();
    text.unmark(5..7, marks::PAGE).unwrap();
    old.commit();

    page.import_changes(&old.export(loro::ExportMode::all_updates()).unwrap())
        .unwrap();
    assert_eq!(page.check_invariants(), Ok(()));
    assert_eq!(spans(&page, id), vec![plain("aTripb")]);
    assert_eq!(page.page_links(), Vec::new());
}

#[test]
fn an_older_version_typing_beside_a_link_keeps_it() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, BlockKind::Paragraph, "ab");
    let trip = PageId::new();
    link(&mut page, id, 1, trip).unwrap();

    let old = older_copy(&page);
    let text = raw_text(&old);
    text.insert(2, "!").unwrap();
    text.insert(1, "?").unwrap();
    text.mark(0..text.len_unicode(), "italic", true).unwrap();
    old.commit();

    page.import_changes(&old.export(loro::ExportMode::all_updates()).unwrap())
        .unwrap();
    assert_eq!(page.check_invariants(), Ok(()));
    assert_eq!(page.page_links(), vec![(id, trip)]);
}

#[test]
fn an_unreadable_target_is_kept_but_not_listed() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, BlockKind::Paragraph, "");
    link(&mut page, id, 0, PageId::new()).unwrap();
    let old = older_copy(&page);
    raw_text(&old).mark(0..1, marks::PAGE, "not-a-page").unwrap();
    old.commit();
    page.import_changes(&old.export(loro::ExportMode::all_updates()).unwrap())
        .unwrap();
    assert_eq!(page.page_links(), Vec::new());
    assert_eq!(page.text(id).unwrap().unwrap()[0].text, PAGE_LINK.to_string());
    assert_eq!(page.check_invariants(), Ok(()));
}

#[test]
fn typing_between_neighbouring_links_never_joins_them() {
    // Loro gives text typed inside one mark's span that mark, so copies and re-pointing must mark each
    // placeholder on its own (found by the page property test).
    let mut page = PageDoc::new();
    let id = insert(&mut page, BlockKind::Paragraph, "");
    let (trip, budget) = (PageId::new(), PageId::new());
    link(&mut page, id, 0, trip).unwrap();
    link(&mut page, id, 0, trip).unwrap();
    page.apply(PageCommand::SetMark {
        id,
        range: 0..2,
        mark: Mark::PageLink(budget),
    })
    .unwrap();
    page.apply(PageCommand::DuplicateBlocks { ids: vec![id] })
        .unwrap();
    for block in page.root_blocks() {
        page.apply(PageCommand::InsertText {
            id: block,
            pos: 1,
            text: "x".into(),
        })
        .unwrap();
        assert_eq!(
            spans(&page, block),
            vec![linked(budget), plain("x"), linked(budget)]
        );
    }
    assert_eq!(page.check_invariants(), Ok(()));
}

#[test]
fn merging_a_block_into_code_leaves_its_links_behind() {
    // Found by the page property test: code keeps no marks, so it keeps no links either.
    let mut page = PageDoc::new();
    let code = insert(
        &mut page,
        BlockKind::Code {
            language: String::new(),
        },
        "x",
    );
    let id = insert(&mut page, BlockKind::Paragraph, "ab");
    link(&mut page, id, 1, PageId::new()).unwrap();
    page.apply(PageCommand::MergeIntoPrevious { id }).unwrap();
    assert_eq!(spans(&page, code), vec![plain("xab")]);
    assert_eq!(page.check_invariants(), Ok(()));
}
