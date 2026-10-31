//! Damaged saved bytes give an error, never a crash (E08-S11). Each input here is one the fuzzer found that made
//! Loro panic inside its import; the files hold a type byte, then a snapshot and updates separated by
//! `\0LEAF-UPDATE\0`, as the `doc_bytes` fuzz target reads them.

use leaf_model::{Database, PageDoc, Workspace, fold_saved};

const PART: &[u8] = b"\0LEAF-UPDATE\0";

/// The snapshot and updates in a fuzz input (its first byte, the document type, dropped).
fn parts(input: &[u8]) -> (Vec<u8>, Vec<Vec<u8>>) {
    let mut pieces: Vec<Vec<u8>> = Vec::new();
    let mut rest = &input[1..];
    while let Some(at) = rest.windows(PART.len()).position(|window| window == PART) {
        pieces.push(rest[..at].to_vec());
        rest = &rest[at + PART.len()..];
    }
    pieces.push(rest.to_vec());
    let snapshot = pieces.remove(0);
    (snapshot, pieces)
}

/// A good snapshot followed by an update whose block header Loro can't read: its import panicked on an
/// assertion (`block_meta_encode.rs`, `rest.is_empty()`) instead of returning an error.
#[test]
fn an_update_loro_panics_on_is_an_error_and_recovery_skips_it() {
    let (snapshot, updates) = parts(include_bytes!("damaged/import-batch-panic.bin"));
    assert!(PageDoc::from_saved(&snapshot, &updates).is_err());
    assert!(Workspace::from_saved(&snapshot, &updates).is_err());
    assert!(Database::from_saved(&snapshot, &updates).is_err());
    assert!(fold_saved(&snapshot, &updates).is_err());
    // Recovery opens what it can: the snapshot, skipping the damaged update.
    if let Ok((_, skipped)) = PageDoc::recover(&snapshot, &updates) {
        assert_ne!(skipped, [] as [usize; 0]);
    }
    // Merging the update into an open page fails the same way.
    let mut page = PageDoc::new();
    assert!(updates.iter().any(|update| page.import_changes(update).is_err()));
}
