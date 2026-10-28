//! Tests that need the store's private reading path.

use rusqlite::params;

use crate::{DocKey, Store};

#[test]
fn updates_are_read_in_the_order_they_were_saved() {
    let dir = std::env::temp_dir().join(format!("leaf-store-unit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let store = Store::open(&dir.join("w.sqlite")).unwrap();
    let id = DocKey::Workspace.to_string();
    store
        .conn
        .execute(
            "INSERT INTO docs (doc_id, snapshot, updated_at) VALUES (?1, x'00', 0)",
            [&id],
        )
        .unwrap();
    store
        .conn
        .execute(
            "INSERT INTO docs (doc_id, snapshot, updated_at) VALUES ('other', x'00', 0)",
            [],
        )
        .unwrap();
    for (doc, byte) in [
        (id.as_str(), 3_u8),
        ("other", 9),
        (id.as_str(), 1),
        (id.as_str(), 2),
    ] {
        store
            .conn
            .execute(
                "INSERT INTO doc_updates (doc_id, data, created_at) VALUES (?1, ?2, 0)",
                params![doc, vec![byte]],
            )
            .unwrap();
    }
    let saved = store.read(&id).unwrap().unwrap();
    assert_eq!(saved.snapshot, vec![0]);
    assert_eq!(
        saved.updates,
        vec![vec![3], vec![1], vec![2]],
        "by seq, not by content"
    );
    assert_eq!(store.read("missing").unwrap(), None);
    drop(store);
    let _ = std::fs::remove_dir_all(&dir);
}
