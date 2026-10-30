//! The performance budgets in `PRODUCT_SPEC.md`, measured: `just bench`.
//!
//! - Open a 5,000-block page: < 100 ms (`open_page`).
//! - Keystroke to paint on a 10,000-block page: < 16 ms. Drawing happens in the window; here
//!   `keystroke` times one typed character, which includes the editor bringing its view model up to
//!   date. `view_update` times that update on its own, and `view_rebuild` a full rebuild, to show
//!   what the incremental update saves.
//! - Saving a keystroke's change (`save`), which autosave does in the background.
//!
//! Benches that change the page take the change outside the timing (`with_inputs`) and use one
//! iteration per sample, so each timed call sees exactly one new keystroke.

#![allow(clippy::unwrap_used, reason = "a bench whose setup fails should stop loudly")]

use std::cell::RefCell;

use divan::Bencher;
use leaf_bench::{OPEN_PAGE_BLOCKS, SAVED_AT, SEED, SavedPage, ViewedPage, caret_mid_page, typing_editor};
use leaf_fixtures::BIG_PAGE_BLOCKS;
use leaf_store::DocKey;

fn main() {
    divan::main();
}

/// Load a saved 5,000-block page, start an editor on it and build the view model the window draws.
#[divan::bench(sample_count = 50)]
fn open_page(bencher: Bencher) {
    let saved = SavedPage::new(SEED, OPEN_PAGE_BLOCKS).unwrap();
    bencher.bench_local(|| saved.open().unwrap());
}

/// Type one character in the middle of a 10,000-block page. Ten per sample, so the paragraph grows
/// by at most a thousand characters over the run, as real typing would.
#[divan::bench(sample_count = 100, sample_size = 10)]
fn keystroke(bencher: Bencher) {
    let editor = RefCell::new(typing_editor(SEED, BIG_PAGE_BLOCKS).unwrap());
    bencher.bench_local(|| editor.borrow_mut().insert_text("a").unwrap());
}

/// Bring a 10,000-block page's view model up to date after one typed character (typed through the
/// model, outside the timing). The editor does this inside every keystroke, so `keystroke` already
/// includes it; this shows its share.
#[divan::bench(sample_count = 200, sample_size = 1)]
fn view_update(bencher: Bencher) {
    let page = RefCell::new(ViewedPage::new(SEED, BIG_PAGE_BLOCKS).unwrap());
    bencher
        .with_inputs(|| page.borrow_mut().type_char().unwrap())
        .bench_local_values(|()| page.borrow_mut().refresh().unwrap());
}

/// Build a 10,000-block page's view model from scratch, for comparison with `view_update`.
#[divan::bench(sample_count = 20)]
fn view_rebuild(bencher: Bencher) {
    let editor = typing_editor(SEED, BIG_PAGE_BLOCKS).unwrap();
    bencher.bench_local(|| editor.rows().unwrap());
}

/// Save one keystroke's change to a 10,000-block page (typed outside the timing). Over 200 saves
/// this includes one compaction of the saved updates, as real autosaves would.
#[divan::bench(sample_count = 200, sample_size = 1)]
fn save(bencher: Bencher) {
    let mut saved = SavedPage::new(SEED, BIG_PAGE_BLOCKS).unwrap();
    let mut editor = saved.open().unwrap();
    caret_mid_page(&mut editor).unwrap();
    let key = DocKey::Page(saved.page);
    let editor = RefCell::new(editor);
    let store = RefCell::new(saved.folder.store_mut());
    bencher
        .with_inputs(|| editor.borrow_mut().insert_text("a").unwrap())
        .bench_local_values(|_| {
            store
                .borrow_mut()
                .save(key, editor.borrow().doc(), SAVED_AT)
                .unwrap()
        });
}
