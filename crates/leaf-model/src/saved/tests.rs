//! Tests for folding saved updates into a snapshot (E03-S05).

use loro::{ExportMode, LoroDoc};

use super::fold_saved;

/// A document saved as a snapshot, then as one update per edit, the way the store keeps it.
fn saved(edits: usize) -> (LoroDoc, Vec<u8>, Vec<Vec<u8>>) {
    let doc = LoroDoc::new();
    // A container no Leaf version knows: folding must carry it over untouched.
    doc.get_map("from-a-newer-leaf").insert("x", 1).unwrap();
    doc.commit();
    let snapshot = doc.export(ExportMode::Snapshot).unwrap();
    let mut updates = Vec::new();
    let text = doc.get_text("t");
    for n in 0..edits {
        let from = doc.oplog_vv();
        text.insert(0, &n.to_string()).unwrap();
        doc.commit();
        updates.push(doc.export(ExportMode::updates(&from)).unwrap());
    }
    (doc, snapshot, updates)
}

#[test]
fn folding_keeps_the_document_and_its_history() {
    let (doc, snapshot, updates) = saved(5);
    let folded = fold_saved(&snapshot, &updates).unwrap().unwrap();
    let back = LoroDoc::new();
    back.import(&folded).unwrap();
    assert_eq!(back.get_deep_value(), doc.get_deep_value());
    assert_eq!(
        back.oplog_vv(),
        doc.oplog_vv(),
        "history too, so merging still works"
    );

    let empty = fold_saved(&snapshot, &[]).unwrap().unwrap();
    let back = LoroDoc::new();
    back.import(&empty).unwrap();
    assert_eq!(
        back.get_map("from-a-newer-leaf")
            .get("x")
            .map(|v| v.into_value().ok()),
        Some(Some(1.into()))
    );
}

#[test]
fn an_update_waiting_for_a_lost_one_is_not_folded_and_damage_is_an_error() {
    let (_, snapshot, updates) = saved(3);
    assert_eq!(fold_saved(&snapshot, &updates[1..]).unwrap(), None);
    assert!(fold_saved(&snapshot, &[vec![0, 255]]).is_err());
    assert!(fold_saved(&[0, 255], &[]).is_err());
}
