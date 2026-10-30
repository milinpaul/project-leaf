//! Tests for action items in the page model (E19-S01). A task's fields are props in its meta, changed
//! with `SetProp` like any kind's: Loro skips writing a value that didn't change, so two devices
//! changing different fields keep both (the test below guards that).

use leaf_core::BlockId;
use loro::{LoroValue, ValueOrContainer};

use super::PageDoc;
use super::tests::{insert, load_raw, raw_block};
use crate::block_kind::keys;
use crate::{BlockKind, BlockProp, DueDate, DueTime, ModelError, PageCommand, PageOutcome, Priority};

fn task() -> BlockKind {
    BlockKind::Task {
        done: false,
        due: None,
        time: None,
        priority: None,
    }
}

fn date(text: &str) -> DueDate {
    text.parse().unwrap()
}

fn time(text: &str) -> DueTime {
    text.parse().unwrap()
}

fn set(page: &mut PageDoc, id: BlockId, prop: BlockProp) -> Result<PageOutcome, ModelError> {
    page.apply(PageCommand::SetProp { id, prop })
}

#[test]
fn each_field_is_set_and_cleared_on_its_own() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, task(), "Send the invoice");
    set(&mut page, id, BlockProp::TaskDue(Some(date("2026-10-31")))).unwrap();
    set(&mut page, id, BlockProp::TaskTime(Some(time("14:30")))).unwrap();
    set(&mut page, id, BlockProp::TaskPriority(Some(Priority::High))).unwrap();
    set(&mut page, id, BlockProp::TaskDone(true)).unwrap();
    assert_eq!(
        page.kind(id).unwrap(),
        BlockKind::Task {
            done: true,
            due: Some(date("2026-10-31")),
            time: Some(time("14:30")),
            priority: Some(Priority::High),
        }
    );
    assert_eq!(page.plain_text(id).unwrap().as_deref(), Some("Send the invoice"));
    assert_eq!(
        set(&mut page, id, BlockProp::TaskDone(true)).unwrap(),
        PageOutcome::Unchanged,
        "setting the same value changes nothing"
    );

    // Clearing the date clears the time with it; priority and done stay.
    set(&mut page, id, BlockProp::TaskDue(None)).unwrap();
    assert_eq!(
        page.kind(id).unwrap(),
        BlockKind::Task {
            done: true,
            due: None,
            time: None,
            priority: Some(Priority::High),
        }
    );
    set(&mut page, id, BlockProp::TaskPriority(None)).unwrap();
    let saved = PageDoc::from_bytes(&page.to_bytes().unwrap()).unwrap();
    assert_eq!(
        saved.kind(id).unwrap(),
        BlockKind::Task {
            done: true,
            due: None,
            time: None,
            priority: None,
        },
        "survives a save and load"
    );
}

#[test]
fn a_time_needs_a_date_and_task_fields_belong_to_tasks() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, task(), "Call");
    assert!(matches!(
        set(&mut page, id, BlockProp::TaskTime(Some(time("09:00")))),
        Err(ModelError::TimeWithoutDate)
    ));
    assert!(matches!(
        page.apply(PageCommand::InsertBlock {
            parent: None,
            index: 0,
            kind: BlockKind::Task {
                done: false,
                due: None,
                time: Some(time("09:00")),
                priority: None,
            },
            text: String::new(),
        }),
        Err(ModelError::TimeWithoutDate)
    ));
    let paragraph = insert(&mut page, BlockKind::Paragraph, "x");
    assert!(matches!(
        set(&mut page, paragraph, BlockProp::TaskDone(true)),
        Err(ModelError::PropNotForKind { .. })
    ));
    assert!(matches!(
        set(&mut page, id, BlockProp::Checked(true)),
        Err(ModelError::PropNotForKind { .. })
    ));
}

#[test]
fn every_change_is_one_undo_step() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, task(), "Plan the trip");
    page.set_undo_merge_interval(0);
    set(&mut page, id, BlockProp::TaskDue(Some(date("2027-01-05")))).unwrap();
    set(&mut page, id, BlockProp::TaskTime(Some(time("18:00")))).unwrap();
    set(&mut page, id, BlockProp::TaskDone(true)).unwrap();
    page.undo().unwrap();
    assert!(matches!(
        page.kind(id).unwrap(),
        BlockKind::Task {
            done: false,
            time: Some(_),
            ..
        }
    ));
    page.undo().unwrap();
    assert!(matches!(
        page.kind(id).unwrap(),
        BlockKind::Task {
            time: None,
            due: Some(_),
            ..
        }
    ));
    page.redo().unwrap();
    page.redo().unwrap();
    assert!(matches!(
        page.kind(id).unwrap(),
        BlockKind::Task {
            done: true,
            time: Some(_),
            ..
        }
    ));
}

#[test]
fn two_devices_changing_different_fields_keep_both() {
    let mut a = PageDoc::new();
    let id = insert(&mut a, task(), "Renew passport");
    set(&mut a, id, BlockProp::TaskDue(Some(date("2026-11-15")))).unwrap();
    set(&mut a, id, BlockProp::TaskPriority(Some(Priority::Medium))).unwrap();
    let mut b = PageDoc::from_bytes(&a.to_bytes().unwrap()).unwrap();
    // At the same time: one ticks it off, the other gives it a date and a priority.
    set(&mut a, id, BlockProp::TaskDone(true)).unwrap();
    set(&mut b, id, BlockProp::TaskDue(Some(date("2026-12-01")))).unwrap();
    set(&mut b, id, BlockProp::TaskPriority(Some(Priority::Low))).unwrap();
    let (from_a, from_b) = (a.to_bytes().unwrap(), b.to_bytes().unwrap());
    a.import_changes(&from_b).unwrap();
    b.import_changes(&from_a).unwrap();
    let both = BlockKind::Task {
        done: true,
        due: Some(date("2026-12-01")),
        time: None,
        priority: Some(Priority::Low),
    };
    assert_eq!(a.kind(id).unwrap(), both);
    assert_eq!(b.kind(id).unwrap(), both);
}

#[test]
fn fields_from_a_newer_version_are_kept() {
    let id = BlockId::new();
    let mut page = load_raw(|tree| {
        let meta = raw_block(tree, id, "task");
        meta.insert(keys::DUE, "2026-11-02").unwrap();
        meta.insert("repeat", "weekly").unwrap(); // a field this version doesn't know
    })
    .unwrap();
    assert!(matches!(
        page.kind(id).unwrap(),
        BlockKind::Task { due: Some(_), .. }
    ));
    set(&mut page, id, BlockProp::TaskDone(true)).unwrap();
    set(&mut page, id, BlockProp::TaskDue(None)).unwrap();
    let saved = PageDoc::from_bytes(&page.to_bytes().unwrap()).unwrap();
    let meta = saved.tree.get_meta(saved.node(id).unwrap()).unwrap();
    assert!(matches!(
        meta.get("repeat"),
        Some(ValueOrContainer::Value(LoroValue::String(s))) if s.as_str() == "weekly"
    ));
}

#[test]
fn turning_a_task_into_another_kind_drops_its_fields() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, task(), "Water the plants");
    set(&mut page, id, BlockProp::TaskDue(Some(date("2026-11-03")))).unwrap();
    page.apply(PageCommand::SetBlockKind {
        id,
        kind: BlockKind::Todo { checked: false },
    })
    .unwrap();
    let meta = page.tree.get_meta(page.node(id).unwrap()).unwrap();
    assert!(meta.get(keys::DUE).is_none(), "no stale date left on the to-do");
    assert_eq!(page.plain_text(id).unwrap().as_deref(), Some("Water the plants"));
}
