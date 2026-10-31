//! Tests for callouts in the page model (E21-S01): the tone, a tone from a newer version, and turning
//! blocks into and out of callouts.

use loro::{LoroValue, ValueOrContainer};

use super::PageDoc;
use super::tests::{insert, load_raw, raw_block};
use crate::block_kind::keys;
use crate::{BlockKind, BlockProp, CalloutTone, Mark, ModelError, PageCommand, PageOutcome};

fn callout(tone: CalloutTone) -> BlockKind {
    BlockKind::Callout { tone }
}

/// The block's stored tone, read straight from its meta.
fn stored_tone(page: &PageDoc, id: leaf_core::BlockId) -> Option<LoroValue> {
    let meta = page.tree.get_meta(page.node(id).unwrap()).unwrap();
    match meta.get(keys::TONE) {
        Some(ValueOrContainer::Value(value)) => Some(value),
        _ => None,
    }
}

#[test]
fn a_callout_holds_text_with_marks_and_its_tone() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, callout(CalloutTone::Warning), "Pack light");
    assert_eq!(page.kind(id).unwrap(), callout(CalloutTone::Warning));
    page.apply(PageCommand::SetMark {
        id,
        range: 0..4,
        mark: Mark::Bold,
    })
    .unwrap();
    assert!(
        page.text(id).unwrap().unwrap()[0].marks.bold,
        "callouts take marks"
    );
    assert_eq!(stored_tone(&page, id), Some(LoroValue::String("warning".into())));

    let child = page.apply(PageCommand::InsertBlock {
        parent: Some(id),
        index: 0,
        kind: BlockKind::Paragraph,
        text: String::new(),
    });
    assert!(child.is_err(), "nothing nests in a callout");
}

#[test]
fn set_prop_changes_the_tone_and_undo_brings_it_back() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, callout(CalloutTone::Info), "Heads up");
    page.set_undo_merge_interval(0);
    page.apply(PageCommand::SetProp {
        id,
        prop: BlockProp::CalloutTone(CalloutTone::Error),
    })
    .unwrap();
    assert_eq!(page.kind(id).unwrap(), callout(CalloutTone::Error));
    assert_eq!(
        page.apply(PageCommand::SetProp {
            id,
            prop: BlockProp::CalloutTone(CalloutTone::Error),
        })
        .unwrap(),
        PageOutcome::Unchanged
    );
    page.undo().unwrap();
    assert_eq!(page.kind(id).unwrap(), callout(CalloutTone::Info));

    let para = insert(&mut page, BlockKind::Paragraph, "plain");
    let err = page
        .apply(PageCommand::SetProp {
            id: para,
            prop: BlockProp::CalloutTone(CalloutTone::Note),
        })
        .unwrap_err();
    assert!(matches!(err, ModelError::PropNotForKind { .. }), "{err}");
}

#[test]
fn turning_into_and_out_of_a_callout_keeps_text_and_marks() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, BlockKind::Quote, "Remember the keys");
    page.apply(PageCommand::SetMark {
        id,
        range: 0..8,
        mark: Mark::Italic,
    })
    .unwrap();
    page.apply(PageCommand::SetBlockKind {
        id,
        kind: callout(CalloutTone::Success),
    })
    .unwrap();
    assert_eq!(page.kind(id).unwrap(), callout(CalloutTone::Success));
    page.apply(PageCommand::SetBlockKind {
        id,
        kind: BlockKind::Paragraph,
    })
    .unwrap();
    assert_eq!(page.kind(id).unwrap(), BlockKind::Paragraph);
    assert_eq!(stored_tone(&page, id), None, "the tone goes with the kind");
    let spans = page.text(id).unwrap().unwrap();
    assert_eq!(spans[0].text, "Remember");
    assert!(spans[0].marks.italic, "marks kept");
}

#[test]
fn a_tone_from_a_newer_version_is_kept_through_edits_and_saving() {
    let id = leaf_core::BlockId::new();
    let mut page = load_raw(|tree| {
        let meta = raw_block(tree, id, "callout");
        meta.insert(keys::TONE, "celebrate").unwrap();
        meta.ensure_mergeable_text(keys::TEXT)
            .unwrap()
            .insert(0, "Launch day")
            .unwrap();
    })
    .unwrap();
    assert_eq!(
        page.kind(id).unwrap(),
        callout(CalloutTone::Other("celebrate".into()))
    );
    page.apply(PageCommand::InsertText {
        id,
        pos: 10,
        text: "!".into(),
    })
    .unwrap();
    let reloaded = PageDoc::from_bytes(&page.to_bytes().unwrap()).unwrap();
    assert_eq!(
        reloaded.kind(id).unwrap(),
        callout(CalloutTone::Other("celebrate".into()))
    );
    assert_eq!(reloaded.plain_text(id).unwrap().as_deref(), Some("Launch day!"));
}

#[test]
fn a_callout_without_a_tone_reads_as_info_and_a_bad_one_is_an_error() {
    let id = leaf_core::BlockId::new();
    let page = load_raw(|tree| {
        raw_block(tree, id, "callout");
    })
    .unwrap();
    assert_eq!(page.kind(id).unwrap(), callout(CalloutTone::Info));

    let bad = load_raw(|tree| {
        raw_block(tree, id, "callout").insert(keys::TONE, 3).unwrap();
    })
    .unwrap();
    assert!(
        matches!(
            bad.kind(id),
            Err(ModelError::InvalidBlockData { field: "tone", .. })
        ),
        "a tone that isn't text is reported"
    );

    let mut page = PageDoc::new();
    let err = page
        .apply(PageCommand::InsertBlock {
            parent: None,
            index: 0,
            kind: callout(CalloutTone::Other(String::new())),
            text: String::new(),
        })
        .unwrap_err();
    assert!(matches!(err, ModelError::InvalidKind(_)), "{err}");
}
